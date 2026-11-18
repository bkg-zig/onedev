-- Vanilla role model (1-to-1 port): a role is a free-form record whose
-- privilege flags live on the role itself (Role.java), and authorizations
-- reference roles by id. Replaces the fixed GitLab-style role enum.

-- Role privilege flags (Role.java fields)
ALTER TABLE roles ADD COLUMN manage_project BOOLEAN NOT NULL DEFAULT FALSE;
ALTER TABLE roles ADD COLUMN create_children BOOLEAN NOT NULL DEFAULT FALSE;
ALTER TABLE roles ADD COLUMN manage_pull_requests BOOLEAN NOT NULL DEFAULT FALSE;
ALTER TABLE roles ADD COLUMN manage_code_comments BOOLEAN NOT NULL DEFAULT FALSE;
ALTER TABLE roles ADD COLUMN code_privilege VARCHAR(10) NOT NULL DEFAULT 'none';
ALTER TABLE roles ADD COLUMN pack_privilege VARCHAR(10) NOT NULL DEFAULT 'none';
ALTER TABLE roles ADD COLUMN manage_issues BOOLEAN NOT NULL DEFAULT FALSE;
ALTER TABLE roles ADD COLUMN access_confidential_issues BOOLEAN NOT NULL DEFAULT FALSE;
ALTER TABLE roles ADD COLUMN access_time_tracking BOOLEAN NOT NULL DEFAULT TRUE;
ALTER TABLE roles ADD COLUMN schedule_issues BOOLEAN NOT NULL DEFAULT FALSE;
ALTER TABLE roles ADD COLUMN editable_issue_fields JSONB NOT NULL DEFAULT '{"type":"all"}';
ALTER TABLE roles ADD COLUMN can_edit_fields_of_other_issues BOOLEAN NOT NULL DEFAULT FALSE;
ALTER TABLE roles ADD COLUMN editable_issue_links JSONB NOT NULL DEFAULT '[]';
ALTER TABLE roles ADD COLUMN manage_builds BOOLEAN NOT NULL DEFAULT FALSE;
ALTER TABLE roles ADD COLUMN upload_cache BOOLEAN NOT NULL DEFAULT FALSE;
ALTER TABLE roles ADD COLUMN job_privileges JSONB NOT NULL DEFAULT '[]';
ALTER TABLE roles ADD COLUMN accessible_sites TEXT;
ALTER TABLE roles ADD COLUMN manage_workspaces BOOLEAN NOT NULL DEFAULT FALSE;
ALTER TABLE roles ADD COLUMN create_workspaces BOOLEAN NOT NULL DEFAULT FALSE;
ALTER TABLE roles ADD COLUMN maintenance BOOLEAN NOT NULL DEFAULT FALSE;

-- Privileges are role flags now, not rows.
DROP TABLE role_permissions;

-- The single built-in role (Role.OWNER_ID, created by DefaultDataService)
INSERT INTO roles (name, description, manage_project)
VALUES ('Project Owner', 'Built-in role with full project administration', TRUE)
ON CONFLICT (name) DO NOTHING;

-- Free-form roles replacing the legacy fixed names, so existing
-- user_authorizations rows keep their meaning.
INSERT INTO roles (name, code_privilege) VALUES ('Guest (legacy)', 'none') ON CONFLICT (name) DO NOTHING;
INSERT INTO roles (name, code_privilege) VALUES ('Reporter (legacy)', 'read') ON CONFLICT (name) DO NOTHING;
INSERT INTO roles (name, code_privilege) VALUES ('Developer (legacy)', 'write') ON CONFLICT (name) DO NOTHING;
INSERT INTO roles (name, code_privilege, manage_issues, manage_builds, manage_pull_requests)
VALUES ('Maintainer (legacy)', 'write', TRUE, TRUE, TRUE) ON CONFLICT (name) DO NOTHING;

-- user_authorizations: role_id instead of the fixed enum
ALTER TABLE user_authorizations ADD COLUMN role_id BIGINT REFERENCES roles(id) ON DELETE CASCADE;
UPDATE user_authorizations ua SET role_id = r.id
FROM roles r
WHERE r.name = CASE ua.role
    WHEN 'guest' THEN 'Guest (legacy)'
    WHEN 'reporter' THEN 'Reporter (legacy)'
    WHEN 'developer' THEN 'Developer (legacy)'
    WHEN 'maintainer' THEN 'Maintainer (legacy)'
    WHEN 'owner' THEN 'Project Owner'
END;
ALTER TABLE user_authorizations ALTER COLUMN role_id SET NOT NULL;
ALTER TABLE user_authorizations DROP COLUMN role;
DROP TYPE project_role;

-- GroupAuthorization (group + project + role)
CREATE TABLE group_authorizations (
    id BIGSERIAL PRIMARY KEY,
    uuid UUID NOT NULL DEFAULT gen_random_uuid() UNIQUE,
    group_id BIGINT NOT NULL REFERENCES groups(id) ON DELETE CASCADE,
    project_id BIGINT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    role_id BIGINT NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(group_id, project_id, role_id)
);
CREATE INDEX idx_group_authorizations_group_id ON group_authorizations(group_id);
CREATE INDEX idx_group_authorizations_project_id ON group_authorizations(project_id);

-- BaseAuthorization (project + role, applies to everyone)
CREATE TABLE base_authorizations (
    id BIGSERIAL PRIMARY KEY,
    uuid UUID NOT NULL DEFAULT gen_random_uuid() UNIQUE,
    project_id BIGINT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    role_id BIGINT NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(project_id, role_id)
);
CREATE INDEX idx_base_authorizations_project_id ON base_authorizations(project_id);

-- AccessTokenAuthorization (project + access token + role)
CREATE TABLE access_token_authorizations (
    id BIGSERIAL PRIMARY KEY,
    uuid UUID NOT NULL DEFAULT gen_random_uuid() UNIQUE,
    project_id BIGINT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    access_token_id BIGINT NOT NULL REFERENCES access_tokens(id) ON DELETE CASCADE,
    role_id BIGINT NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(access_token_id, project_id, role_id)
);
CREATE INDEX idx_access_token_authorizations_token_id ON access_token_authorizations(access_token_id);

-- LinkAuthorization (link name + role)
CREATE TABLE link_authorizations (
    id BIGSERIAL PRIMARY KEY,
    uuid UUID NOT NULL DEFAULT gen_random_uuid() UNIQUE,
    link_name VARCHAR(200) NOT NULL,
    role_id BIGINT NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(link_name, role_id)
);

-- IssueAuthorization in vanilla is only issue + user (no permission level)
ALTER TABLE issue_authorizations DROP COLUMN permission;
DROP TYPE issue_permission;
