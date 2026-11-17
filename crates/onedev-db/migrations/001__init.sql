-- OneDev Database Initial Migration
-- This migration creates all core tables for OneDev

-- Enable required extensions
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS "pgcrypto";

-- Custom types
CREATE TYPE user_type AS ENUM ('ordinary', 'service', 'ai');
CREATE TYPE user_status AS ENUM ('active', 'disabled', 'pending_verification', 'locked');
CREATE TYPE project_visibility AS ENUM ('private', 'internal', 'public');
CREATE TYPE project_status AS ENUM ('active', 'archived', 'maintenance', 'deleted');
CREATE TYPE owner_type AS ENUM ('user', 'group');
CREATE TYPE group_visibility AS ENUM ('private', 'internal', 'public');
CREATE TYPE project_creation_level AS ENUM ('noone', 'maintainer', 'developer', 'owner');
CREATE TYPE subgroup_creation_level AS ENUM ('noone', 'owner');
CREATE TYPE issue_state AS ENUM ('open', 'closed');
CREATE TYPE pull_request_state AS ENUM ('open', 'merged', 'closed', 'abandoned');
CREATE TYPE merge_strategy AS ENUM ('merge_commit', 'squash', 'fast_forward', 'rebase');
CREATE TYPE ssh_key_type AS ENUM ('rsa', 'dsa', 'ecdsa', 'ed25519');
CREATE TYPE group_role AS ENUM ('guest', 'reporter', 'developer', 'maintainer', 'owner');
CREATE TYPE project_role AS ENUM ('guest', 'reporter', 'developer', 'maintainer', 'owner');
CREATE TYPE issue_permission AS ENUM ('view', 'edit', 'admin');
CREATE TYPE dashboard_owner_type AS ENUM ('user', 'group');
CREATE TYPE dashboard_permission AS ENUM ('view', 'edit');
CREATE TYPE watchable_type AS ENUM ('project', 'issue', 'pull_request', 'build', 'pack', 'workspace');
CREATE TYPE query_type AS ENUM ('issue', 'pull_request', 'build', 'pack', 'workspace', 'commit', 'code_comment');
CREATE TYPE workspace_status AS ENUM ('pending', 'provisioning', 'running', 'stopped', 'failed', 'terminated');
CREATE TYPE build_status AS ENUM ('pending', 'running', 'success', 'failed', 'cancelled', 'skipped');
CREATE TYPE build_trigger AS ENUM ('push', 'pull_request', 'tag', 'schedule', 'manual', 'api', 'webhook');
CREATE TYPE agent_status AS ENUM ('offline', 'online', 'busy', 'disabled');
CREATE TYPE executor_type AS ENUM ('local', 'docker', 'kubernetes', 'sandbox');
CREATE TYPE oauth_owner_type AS ENUM ('user', 'group');
CREATE TYPE pack_type AS ENUM ('maven', 'npm', 'pypi', 'nuget', 'cargo', 'generic');

-- Users table
CREATE TABLE users (
    id BIGSERIAL PRIMARY KEY,
    uuid UUID NOT NULL DEFAULT uuid_generate_v4() UNIQUE,
    name VARCHAR(100) NOT NULL UNIQUE,
    full_name VARCHAR(200),
    email VARCHAR(255),
    password_hash TEXT,
    user_type user_type NOT NULL DEFAULT 'ordinary',
    status user_status NOT NULL DEFAULT 'pending_verification',
    avatar_url TEXT,
    two_factor_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    two_factor_secret TEXT,
    backup_codes JSONB NOT NULL DEFAULT '[]',
    last_login_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ,
    notify_own_events BOOLEAN NOT NULL DEFAULT TRUE,
    keep_emails_private BOOLEAN NOT NULL DEFAULT FALSE,
    timezone VARCHAR(50) NOT NULL DEFAULT 'UTC',
    locale VARCHAR(10) NOT NULL DEFAULT 'en',
    ai_setting JSONB,
    two_factor_auth JSONB
);

CREATE INDEX idx_users_name ON users(name);
CREATE INDEX idx_users_email ON users(email);
CREATE INDEX idx_users_uuid ON users(uuid);
CREATE INDEX idx_users_status ON users(status);
CREATE INDEX idx_users_deleted_at ON users(deleted_at);

-- Special system users
INSERT INTO users (id, uuid, name, full_name, email, user_type, status, created_at, updated_at)
VALUES
    (-1, uuid_generate_v4(), 'OneDev', 'OneDev System', 'noreply@onedev.io', 'service', 'active', NOW(), NOW()),
    (-2, uuid_generate_v4(), 'unknown', 'Unknown User', NULL, 'ordinary', 'disabled', NOW(), NOW())
ON CONFLICT (id) DO NOTHING;

-- Root user (id=1) will be created on first run

-- Email addresses table
CREATE TABLE email_addresses (
    id BIGSERIAL PRIMARY KEY,
    uuid UUID NOT NULL DEFAULT uuid_generate_v4() UNIQUE,
    user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    value VARCHAR(255) NOT NULL,
    verified BOOLEAN NOT NULL DEFAULT FALSE,
    primary BOOLEAN NOT NULL DEFAULT FALSE,
    verification_code VARCHAR(100),
    verification_sent_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_email_addresses_user_id ON email_addresses(user_id);
CREATE INDEX idx_email_addresses_value ON email_addresses(value);
CREATE UNIQUE INDEX idx_email_addresses_user_primary ON email_addresses(user_id) WHERE primary = TRUE;

-- SSH keys table
CREATE TABLE ssh_keys (
    id BIGSERIAL PRIMARY KEY,
    uuid UUID NOT NULL DEFAULT uuid_generate_v4() UNIQUE,
    user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    title VARCHAR(200) NOT NULL,
    public_key TEXT NOT NULL,
    fingerprint_sha256 VARCHAR(64) NOT NULL,
    fingerprint_md5 VARCHAR(32) NOT NULL,
    key_type ssh_key_type NOT NULL,
    bits INTEGER NOT NULL,
    last_used_at TIMESTAMPTZ,
    expires_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_ssh_keys_user_id ON ssh_keys(user_id);
CREATE INDEX idx_ssh_keys_fingerprint_sha256 ON ssh_keys(fingerprint_sha256);

-- GPG keys table
CREATE TABLE gpg_keys (
    id BIGSERIAL PRIMARY KEY,
    uuid UUID NOT NULL DEFAULT uuid_generate_v4() UNIQUE,
    user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    key_id VARCHAR(32) NOT NULL,
    public_key TEXT NOT NULL,
    fingerprint VARCHAR(64) NOT NULL,
    email VARCHAR(255),
    expires_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_gpg_keys_user_id ON gpg_keys(user_id);
CREATE INDEX idx_gpg_keys_key_id ON gpg_keys(key_id);

-- Access tokens table
CREATE TABLE access_tokens (
    id BIGSERIAL PRIMARY KEY,
    uuid UUID NOT NULL DEFAULT uuid_generate_v4() UNIQUE,
    user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name VARCHAR(100) NOT NULL,
    token_hash VARCHAR(64) NOT NULL,
    scopes JSONB NOT NULL DEFAULT '[]',
    expires_at TIMESTAMPTZ,
    last_used_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    revoked_at TIMESTAMPTZ
);

CREATE INDEX idx_access_tokens_user_id ON access_tokens(user_id);
CREATE INDEX idx_access_tokens_token_hash ON access_tokens(token_hash);
CREATE INDEX idx_access_tokens_revoked_at ON access_tokens(revoked_at);

-- Groups table
CREATE TABLE groups (
    id BIGSERIAL PRIMARY KEY,
    uuid UUID NOT NULL DEFAULT uuid_generate_v4() UNIQUE,
    name VARCHAR(100) NOT NULL,
    path VARCHAR(100) NOT NULL,
    description TEXT,
    parent_id BIGINT REFERENCES groups(id) ON DELETE SET NULL,
    avatar_url TEXT,
    visibility group_visibility NOT NULL DEFAULT 'private',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ,
    require_two_factor BOOLEAN NOT NULL DEFAULT FALSE,
    two_factor_grace_period INTEGER NOT NULL DEFAULT 0,
    project_creation_level project_creation_level NOT NULL DEFAULT 'maintainer',
    subgroup_creation_level subgroup_creation_level NOT NULL DEFAULT 'owner',
    settings JSONB
);

CREATE INDEX idx_groups_path ON groups(path);
CREATE INDEX idx_groups_parent_id ON groups(parent_id);
CREATE INDEX idx_groups_uuid ON groups(uuid);
CREATE INDEX idx_groups_deleted_at ON groups(deleted_at);
CREATE UNIQUE INDEX idx_groups_parent_path ON groups(parent_id, path);

-- Memberships table
CREATE TABLE memberships (
    id BIGSERIAL PRIMARY KEY,
    uuid UUID NOT NULL DEFAULT uuid_generate_v4() UNIQUE,
    user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    group_id BIGINT NOT NULL REFERENCES groups(id) ON DELETE CASCADE,
    role group_role NOT NULL DEFAULT 'guest',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(user_id, group_id)
);

CREATE INDEX idx_memberships_user_id ON memberships(user_id);
CREATE INDEX idx_memberships_group_id ON memberships(group_id);

-- Projects table
CREATE TABLE projects (
    id BIGSERIAL PRIMARY KEY,
    uuid UUID NOT NULL DEFAULT uuid_generate_v4() UNIQUE,
    name VARCHAR(200) NOT NULL,
    path VARCHAR(100) NOT NULL,
    description TEXT,
    readme TEXT,
    visibility project_visibility NOT NULL DEFAULT 'private',
    status project_status NOT NULL DEFAULT 'active',
    owner_id BIGINT NOT NULL,
    owner_type owner_type NOT NULL DEFAULT 'user',
    parent_id BIGINT REFERENCES projects(id) ON DELETE SET NULL,
    forked_from_id BIGINT REFERENCES projects(id) ON DELETE SET NULL,
    default_branch VARCHAR(100) NOT NULL DEFAULT 'main',
    avatar_url TEXT,
    issues_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    merge_requests_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    builds_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    wiki_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    snippets_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    packs_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    pages_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    container_registry_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    only_allow_merge_if_pipeline_succeeds BOOLEAN NOT NULL DEFAULT FALSE,
    only_allow_merge_if_all_discussions_resolved BOOLEAN NOT NULL DEFAULT FALSE,
    remove_source_branch_after_merge BOOLEAN NOT NULL DEFAULT TRUE,
    enable_merge_commit BOOLEAN NOT NULL DEFAULT TRUE,
    enable_squash_merge BOOLEAN NOT NULL DEFAULT TRUE,
    enable_fast_forward_merge BOOLEAN NOT NULL DEFAULT TRUE,
    squash_merge_template TEXT,
    build_timeout INTEGER NOT NULL DEFAULT 60,
    build_coverage_regex VARCHAR(200),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ,
    last_activity_at TIMESTAMPTZ,
    issue_setting JSONB,
    pull_request_setting JSONB,
    build_setting JSONB,
    pack_setting JSONB,
    workspace_setting JSONB,
    code_indexing_setting JSONB,
    wiki_setting JSONB,
    webhooks JSONB,
    branch_protections JSONB,
    tag_protections JSONB
);

CREATE INDEX idx_projects_path ON projects(path);
CREATE INDEX idx_projects_parent_id ON projects(parent_id);
CREATE INDEX idx_projects_forked_from_id ON projects(forked_from_id);
CREATE INDEX idx_projects_owner_id ON projects(owner_id);
CREATE INDEX idx_projects_uuid ON projects(uuid);
CREATE INDEX idx_projects_deleted_at ON projects(deleted_at);
CREATE INDEX idx_projects_last_activity ON projects(last_activity_at);
CREATE UNIQUE INDEX idx_projects_parent_path ON projects(parent_id, path);

-- Project number counter
CREATE TABLE project_number_counters (
    project_id BIGINT PRIMARY KEY REFERENCES projects(id) ON DELETE CASCADE,
    issue_counter INTEGER NOT NULL DEFAULT 0,
    merge_request_counter INTEGER NOT NULL DEFAULT 0,
    build_counter INTEGER NOT NULL DEFAULT 0
);

-- Labels table
CREATE TABLE labels (
    id BIGSERIAL PRIMARY KEY,
    uuid UUID NOT NULL DEFAULT uuid_generate_v4() UNIQUE,
    project_id BIGINT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name VARCHAR(100) NOT NULL,
    color VARCHAR(7) NOT NULL DEFAULT '#428bca',
    description TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(project_id, name)
);

CREATE INDEX idx_labels_project_id ON labels(project_id);

-- Issues table
CREATE TABLE issues (
    id BIGSERIAL PRIMARY KEY,
    uuid UUID NOT NULL DEFAULT uuid_generate_v4() UNIQUE,
    project_id BIGINT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    number INTEGER NOT NULL,
    title VARCHAR(500) NOT NULL,
    description TEXT,
    state issue_state NOT NULL DEFAULT 'open',
    submitter_id BIGINT NOT NULL REFERENCES users(id),
    assignee_id BIGINT REFERENCES users(id) ON DELETE SET NULL,
    priority INTEGER NOT NULL DEFAULT 0,
    milestone_id BIGINT,
    estimated_duration INTEGER,
    remaining_duration INTEGER,
    spent_duration INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    closed_at TIMESTAMPTZ,
    due_date TIMESTAMPTZ,
    label_ids BIGINT[] NOT NULL DEFAULT '{}',
    watcher_ids BIGINT[] NOT NULL DEFAULT '{}',
    voter_ids BIGINT[] NOT NULL DEFAULT '{}'
);

CREATE INDEX idx_issues_project_id ON issues(project_id);
CREATE INDEX idx_issues_number ON issues(number);
CREATE INDEX idx_issues_submitter_id ON issues(submitter_id);
CREATE INDEX idx_issues_assignee_id ON issues(assignee_id);
CREATE INDEX idx_issues_state ON issues(state);
CREATE INDEX idx_issues_milestone_id ON issues(milestone_id);
CREATE UNIQUE INDEX idx_issues_project_number ON issues(project_id, number);

-- Pull requests table
CREATE TABLE pull_requests (
    id BIGSERIAL PRIMARY KEY,
    uuid UUID NOT NULL DEFAULT uuid_generate_v4() UNIQUE,
    project_id BIGINT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    number INTEGER NOT NULL,
    title VARCHAR(500) NOT NULL,
    description TEXT,
    state pull_request_state NOT NULL DEFAULT 'open',
    author_id BIGINT NOT NULL REFERENCES users(id),
    source_project_id BIGINT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    source_branch VARCHAR(255) NOT NULL,
    target_project_id BIGINT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    target_branch VARCHAR(255) NOT NULL,
    merge_strategy merge_strategy NOT NULL DEFAULT 'merge_commit',
    merge_commit_sha VARCHAR(64),
    head_commit_sha VARCHAR(64) NOT NULL,
    base_commit_sha VARCHAR(64) NOT NULL,
    merge_base_sha VARCHAR(64),
    mergeable BOOLEAN NOT NULL DEFAULT TRUE,
    merge_check_status JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    closed_at TIMESTAMPTZ,
    merged_at TIMESTAMPTZ,
    label_ids BIGINT[] NOT NULL DEFAULT '{}',
    reviewer_ids BIGINT[] NOT NULL DEFAULT '{}',
    watcher_ids BIGINT[] NOT NULL DEFAULT '{}',
    draft BOOLEAN NOT NULL DEFAULT FALSE
);

CREATE INDEX idx_pull_requests_project_id ON pull_requests(project_id);
CREATE INDEX idx_pull_requests_number ON pull_requests(number);
CREATE INDEX idx_pull_requests_author_id ON pull_requests(author_id);
CREATE INDEX idx_pull_requests_state ON pull_requests(state);
CREATE INDEX idx_pull_requests_source_project_id ON pull_requests(source_project_id);
CREATE INDEX idx_pull_requests_target_project_id ON pull_requests(target_project_id);
CREATE UNIQUE INDEX idx_pull_requests_project_number ON pull_requests(project_id, number);

-- User authorizations table
CREATE TABLE user_authorizations (
    id BIGSERIAL PRIMARY KEY,
    uuid UUID NOT NULL DEFAULT uuid_generate_v4() UNIQUE,
    user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    project_id BIGINT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    role project_role NOT NULL DEFAULT 'guest',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(user_id, project_id)
);

CREATE INDEX idx_user_authorizations_user_id ON user_authorizations(user_id);
CREATE INDEX idx_user_authorizations_project_id ON user_authorizations(project_id);

-- Issue authorizations table
CREATE TABLE issue_authorizations (
    id BIGSERIAL PRIMARY KEY,
    uuid UUID NOT NULL DEFAULT uuid_generate_v4() UNIQUE,
    user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    issue_id BIGINT NOT NULL REFERENCES issues(id) ON DELETE CASCADE,
    permission issue_permission NOT NULL DEFAULT 'view',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_issue_authorizations_user_id ON issue_authorizations(user_id);
CREATE INDEX idx_issue_authorizations_issue_id ON issue_authorizations(issue_id);
CREATE UNIQUE INDEX idx_issue_authorizations_user_issue ON issue_authorizations(user_id, issue_id);

-- Watches table
CREATE TABLE watches (
    id BIGSERIAL PRIMARY KEY,
    uuid UUID NOT NULL DEFAULT uuid_generate_v4() UNIQUE,
    user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    watchable_type watchable_type NOT NULL,
    watchable_id BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(user_id, watchable_type, watchable_id)
);

CREATE INDEX idx_watches_user_id ON watches(user_id);
CREATE INDEX idx_watches_watchable ON watches(watchable_type, watchable_id);

-- Query subscriptions table
CREATE TABLE query_subscriptions (
    id BIGSERIAL PRIMARY KEY,
    uuid UUID NOT NULL DEFAULT uuid_generate_v4() UNIQUE,
    user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    query_name VARCHAR(200) NOT NULL,
    query_type query_type NOT NULL,
    project_id BIGINT REFERENCES projects(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(user_id, query_name, query_type)
);

CREATE INDEX idx_query_subscriptions_user_id ON query_subscriptions(user_id);

-- Roles table
CREATE TABLE roles (
    id BIGSERIAL PRIMARY KEY,
    uuid UUID NOT NULL DEFAULT uuid_generate_v4() UNIQUE,
    name VARCHAR(100) NOT NULL UNIQUE,
    description TEXT,
    is_system BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Role permissions table
CREATE TABLE role_permissions (
    id BIGSERIAL PRIMARY KEY,
    uuid UUID NOT NULL DEFAULT uuid_generate_v4() UNIQUE,
    role_id BIGINT NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
    permission VARCHAR(200) NOT NULL,
    resource_type VARCHAR(50),
    resource_id BIGINT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(role_id, permission, resource_type, resource_id)
);

CREATE INDEX idx_role_permissions_role_id ON role_permissions(role_id);

-- System settings table
CREATE TABLE system_settings (
    id BIGSERIAL PRIMARY KEY,
    uuid UUID NOT NULL DEFAULT uuid_generate_v4() UNIQUE,
    key VARCHAR(200) NOT NULL UNIQUE,
    value JSONB NOT NULL,
    description TEXT,
    category VARCHAR(100) NOT NULL DEFAULT 'general',
    editable BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Project settings table
CREATE TABLE project_settings (
    id BIGSERIAL PRIMARY KEY,
    uuid UUID NOT NULL DEFAULT uuid_generate_v4() UNIQUE,
    project_id BIGINT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    key VARCHAR(200) NOT NULL,
    value JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(project_id, key)
);

CREATE INDEX idx_project_settings_project_id ON project_settings(project_id);

-- User settings table
CREATE TABLE user_settings (
    id BIGSERIAL PRIMARY KEY,
    uuid UUID NOT NULL DEFAULT uuid_generate_v4() UNIQUE,
    user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    key VARCHAR(200) NOT NULL,
    value JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(user_id, key)
);

CREATE INDEX idx_user_settings_user_id ON user_settings(user_id);

-- Webhooks table
CREATE TABLE webhooks (
    id BIGSERIAL PRIMARY KEY,
    uuid UUID NOT NULL DEFAULT uuid_generate_v4() UNIQUE,
    project_id BIGINT REFERENCES projects(id) ON DELETE CASCADE,
    url TEXT NOT NULL,
    secret VARCHAR(255),
    scopes JSONB NOT NULL DEFAULT '[]',
    branch_filter VARCHAR(255),
    tag_filter VARCHAR(255),
    ssl_verification BOOLEAN NOT NULL DEFAULT TRUE,
    active BOOLEAN NOT NULL DEFAULT TRUE,
    last_delivery_at TIMESTAMPTZ,
    last_delivery_status INTEGER,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_webhooks_project_id ON webhooks(project_id);

-- Dashboards table
CREATE TABLE dashboards (
    id BIGSERIAL PRIMARY KEY,
    uuid UUID NOT NULL DEFAULT uuid_generate_v4() UNIQUE,
    owner_id BIGINT NOT NULL,
    owner_type dashboard_owner_type NOT NULL DEFAULT 'user',
    name VARCHAR(200) NOT NULL,
    layout JSONB NOT NULL DEFAULT '{}',
    is_default BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_dashboards_owner ON dashboards(owner_id, owner_type);

-- Dashboard shares table
CREATE TABLE dashboard_shares (
    id BIGSERIAL PRIMARY KEY,
    uuid UUID NOT NULL DEFAULT uuid_generate_v4() UNIQUE,
    dashboard_id BIGINT NOT NULL REFERENCES dashboards(id) ON DELETE CASCADE,
    user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    permission dashboard_permission NOT NULL DEFAULT 'view',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(dashboard_id, user_id)
);

CREATE INDEX idx_dashboard_shares_dashboard_id ON dashboard_shares(dashboard_id);
CREATE INDEX idx_dashboard_shares_user_id ON dashboard_shares(user_id);

-- Dashboard visits table
CREATE TABLE dashboard_visits (
    id BIGSERIAL PRIMARY KEY,
    uuid UUID NOT NULL DEFAULT uuid_generate_v4() UNIQUE,
    user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    dashboard_id BIGINT NOT NULL REFERENCES dashboards(id) ON DELETE CASCADE,
    last_visited_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(user_id, dashboard_id)
);

-- Code comments table
CREATE TABLE code_comments (
    id BIGSERIAL PRIMARY KEY,
    uuid UUID NOT NULL DEFAULT uuid_generate_v4() UNIQUE,
    project_id BIGINT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    pull_request_id BIGINT REFERENCES pull_requests(id) ON DELETE CASCADE,
    commit_sha VARCHAR(64) NOT NULL,
    file_path VARCHAR(500) NOT NULL,
    old_path VARCHAR(500),
    line_number INTEGER,
    old_line_number INTEGER,
    author_id BIGINT NOT NULL REFERENCES users(id),
    content TEXT NOT NULL,
    parent_id BIGINT REFERENCES code_comments(id) ON DELETE CASCADE,
    resolved BOOLEAN NOT NULL DEFAULT FALSE,
    resolved_by_id BIGINT REFERENCES users(id) ON DELETE SET NULL,
    resolved_at TIMESTAMPTZ,
    outdated BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_code_comments_project_id ON code_comments(project_id);
CREATE INDEX idx_code_comments_pull_request_id ON code_comments(pull_request_id);
CREATE INDEX idx_code_comments_commit_sha ON code_comments(commit_sha);
CREATE INDEX idx_code_comments_file_path ON code_comments(file_path);
CREATE INDEX idx_code_comments_author_id ON code_comments(author_id);
CREATE INDEX idx_code_comments_parent_id ON code_comments(parent_id);

-- Code comment mentions table
CREATE TABLE code_comment_mentions (
    id BIGSERIAL PRIMARY KEY,
    uuid UUID NOT NULL DEFAULT uuid_generate_v4() UNIQUE,
    code_comment_id BIGINT NOT NULL REFERENCES code_comments(id) ON DELETE CASCADE,
    user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(code_comment_id, user_id)
);

-- Packs table
CREATE TABLE packs (
    id BIGSERIAL PRIMARY KEY,
    uuid UUID NOT NULL DEFAULT uuid_generate_v4() UNIQUE,
    project_id BIGINT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name VARCHAR(200) NOT NULL,
    version VARCHAR(100) NOT NULL,
    pack_type pack_type NOT NULL,
    description TEXT,
    readme TEXT,
    author_id BIGINT NOT NULL REFERENCES users(id),
    size BIGINT NOT NULL DEFAULT 0,
    file_count INTEGER NOT NULL DEFAULT 0,
    dependencies JSONB,
    metadata JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ,
    UNIQUE(project_id, name, version)
);

CREATE INDEX idx_packs_project_id ON packs(project_id);
CREATE INDEX idx_packs_name ON packs(name);
CREATE INDEX idx_packs_deleted_at ON packs(deleted_at);

-- Pack blob references table
CREATE TABLE pack_blob_refs (
    id BIGSERIAL PRIMARY KEY,
    uuid UUID NOT NULL DEFAULT uuid_generate_v4() UNIQUE,
    pack_id BIGINT NOT NULL REFERENCES packs(id) ON DELETE CASCADE,
    blob_sha256 VARCHAR(64) NOT NULL,
    path VARCHAR(500) NOT NULL,
    size BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_pack_blob_refs_pack_id ON pack_blob_refs(pack_id);
CREATE INDEX idx_pack_blob_refs_blob_sha256 ON pack_blob_refs(blob_sha256);

-- Pack labels table
CREATE TABLE pack_labels (
    id BIGSERIAL PRIMARY KEY,
    uuid UUID NOT NULL DEFAULT uuid_generate_v4() UNIQUE,
    pack_id BIGINT NOT NULL REFERENCES packs(id) ON DELETE CASCADE,
    name VARCHAR(100) NOT NULL,
    color VARCHAR(7) NOT NULL DEFAULT '#428bca',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(pack_id, name)
);

-- Workspaces table
CREATE TABLE workspaces (
    id BIGSERIAL PRIMARY KEY,
    uuid UUID NOT NULL DEFAULT uuid_generate_v4() UNIQUE,
    project_id BIGINT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name VARCHAR(200) NOT NULL,
    spec JSONB NOT NULL,
    status workspace_status NOT NULL DEFAULT 'pending',
    agent_id BIGINT,
    creator_id BIGINT NOT NULL REFERENCES users(id),
    commit_sha VARCHAR(64),
    branch VARCHAR(255),
    started_at TIMESTAMPTZ,
    finished_at TIMESTAMPTZ,
    error_message TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_workspaces_project_id ON workspaces(project_id);
CREATE INDEX idx_workspaces_creator_id ON workspaces(creator_id);
CREATE INDEX idx_workspaces_agent_id ON workspaces(agent_id);
CREATE INDEX idx_workspaces_status ON workspaces(status);

-- Builds table
CREATE TABLE builds (
    id BIGSERIAL PRIMARY KEY,
    uuid UUID NOT NULL DEFAULT uuid_generate_v4() UNIQUE,
    project_id BIGINT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    number INTEGER NOT NULL,
    ref_name VARCHAR(255) NOT NULL,
    commit_sha VARCHAR(64) NOT NULL,
    commit_message TEXT,
    author_id BIGINT NOT NULL REFERENCES users(id),
    status build_status NOT NULL DEFAULT 'pending',
    trigger build_trigger NOT NULL DEFAULT 'push',
    trigger_data JSONB,
    agent_id BIGINT,
    started_at TIMESTAMPTZ,
    finished_at TIMESTAMPTZ,
    duration INTEGER,
    error_message TEXT,
    spec JSONB NOT NULL,
    parameters JSONB,
    artifacts JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_builds_project_id ON builds(project_id);
CREATE INDEX idx_builds_number ON builds(number);
CREATE INDEX idx_builds_status ON builds(status);
CREATE INDEX idx_builds_author_id ON builds(author_id);
CREATE INDEX idx_builds_agent_id ON builds(agent_id);
CREATE UNIQUE INDEX idx_builds_project_number ON builds(project_id, number);

-- Build jobs table
CREATE TABLE build_jobs (
    id BIGSERIAL PRIMARY KEY,
    uuid UUID NOT NULL DEFAULT uuid_generate_v4() UNIQUE,
    build_id BIGINT NOT NULL REFERENCES builds(id) ON DELETE CASCADE,
    name VARCHAR(200) NOT NULL,
    status build_status NOT NULL DEFAULT 'pending',
    agent_id BIGINT,
    started_at TIMESTAMPTZ,
    finished_at TIMESTAMPTZ,
    duration INTEGER,
    error_message TEXT,
    spec JSONB NOT NULL,
    dependencies BIGINT[] NOT NULL DEFAULT '{}',
    order_num INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_build_jobs_build_id ON build_jobs(build_id);
CREATE INDEX idx_build_jobs_agent_id ON build_jobs(agent_id);
CREATE INDEX idx_build_jobs_status ON build_jobs(status);

-- Build steps table
CREATE TABLE build_steps (
    id BIGSERIAL PRIMARY KEY,
    uuid UUID NOT NULL DEFAULT uuid_generate_v4() UNIQUE,
    job_id BIGINT NOT NULL REFERENCES build_jobs(id) ON DELETE CASCADE,
    name VARCHAR(200) NOT NULL,
    status build_status NOT NULL DEFAULT 'pending',
    command TEXT,
    image VARCHAR(500),
    started_at TIMESTAMPTZ,
    finished_at TIMESTAMPTZ,
    duration INTEGER,
    error_message TEXT,
    logs TEXT,
    order_num INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_build_steps_job_id ON build_steps(job_id);
CREATE INDEX idx_build_steps_status ON build_steps(status);

-- Agents table
CREATE TABLE agents (
    id BIGSERIAL PRIMARY KEY,
    uuid UUID NOT NULL DEFAULT uuid_generate_v4() UNIQUE,
    name VARCHAR(200) NOT NULL,
    token_hash VARCHAR(64) NOT NULL UNIQUE,
    status agent_status NOT NULL DEFAULT 'offline',
    executor_type executor_type NOT NULL DEFAULT 'local',
    executor_config JSONB,
    labels JSONB NOT NULL DEFAULT '[]',
    max_concurrent_jobs INTEGER NOT NULL DEFAULT 1,
    current_jobs INTEGER NOT NULL DEFAULT 0,
    last_seen_at TIMESTAMPTZ,
    last_used_at TIMESTAMPTZ,
    version VARCHAR(50),
    os VARCHAR(50),
    arch VARCHAR(50),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ
);

CREATE INDEX idx_agents_token_hash ON agents(token_hash);
CREATE INDEX idx_agents_status ON agents(status);
CREATE INDEX idx_agents_deleted_at ON agents(deleted_at);

-- Pipelines table
CREATE TABLE pipelines (
    id BIGSERIAL PRIMARY KEY,
    uuid UUID NOT NULL DEFAULT uuid_generate_v4() UNIQUE,
    project_id BIGINT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name VARCHAR(200) NOT NULL,
    spec JSONB NOT NULL,
    is_default BOOLEAN NOT NULL DEFAULT FALSE,
    protected BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(project_id, name)
);

CREATE INDEX idx_pipelines_project_id ON pipelines(project_id);

-- Pipeline schedules table
CREATE TABLE pipeline_schedules (
    id BIGSERIAL PRIMARY KEY,
    uuid UUID NOT NULL DEFAULT uuid_generate_v4() UNIQUE,
    pipeline_id BIGINT NOT NULL REFERENCES pipelines(id) ON DELETE CASCADE,
    cron_expression VARCHAR(100) NOT NULL,
    timezone VARCHAR(50) NOT NULL DEFAULT 'UTC',
    ref_name VARCHAR(255) NOT NULL DEFAULT 'main',
    parameters JSONB,
    active BOOLEAN NOT NULL DEFAULT TRUE,
    next_run_at TIMESTAMPTZ,
    last_run_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_pipeline_schedules_pipeline_id ON pipeline_schedules(pipeline_id);
CREATE INDEX idx_pipeline_schedules_next_run ON pipeline_schedules(next_run_at);

-- OAuth applications table
CREATE TABLE oauth_applications (
    id BIGSERIAL PRIMARY KEY,
    uuid UUID NOT NULL DEFAULT uuid_generate_v4() UNIQUE,
    owner_id BIGINT NOT NULL,
    owner_type oauth_owner_type NOT NULL DEFAULT 'user',
    name VARCHAR(200) NOT NULL,
    client_id VARCHAR(100) NOT NULL UNIQUE,
    client_secret_hash VARCHAR(64) NOT NULL,
    redirect_uris JSONB NOT NULL DEFAULT '[]',
    scopes JSONB NOT NULL DEFAULT '[]',
    confidential BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_oauth_applications_owner ON oauth_applications(owner_id, owner_type);
CREATE INDEX idx_oauth_applications_client_id ON oauth_applications(client_id);

-- OAuth grants table
CREATE TABLE oauth_grants (
    id BIGSERIAL PRIMARY KEY,
    uuid UUID NOT NULL DEFAULT uuid_generate_v4() UNIQUE,
    application_id BIGINT NOT NULL REFERENCES oauth_applications(id) ON DELETE CASCADE,
    user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    scopes JSONB NOT NULL DEFAULT '[]',
    redirect_uri TEXT NOT NULL,
    code VARCHAR(100),
    code_hash VARCHAR(64),
    code_expires_at TIMESTAMPTZ,
    access_token_hash VARCHAR(64),
    refresh_token_hash VARCHAR(64),
    access_token_expires_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_oauth_grants_application_id ON oauth_grants(application_id);
CREATE INDEX idx_oauth_grants_user_id ON oauth_grants(user_id);
CREATE INDEX idx_oauth_grants_code_hash ON oauth_grants(code_hash);
CREATE INDEX idx_oauth_grants_access_token_hash ON oauth_grants(access_token_hash);
CREATE INDEX idx_oauth_grants_refresh_token_hash ON oauth_grants(refresh_token_hash);

-- SSO accounts table
CREATE TABLE sso_accounts (
    id BIGSERIAL PRIMARY KEY,
    uuid UUID NOT NULL DEFAULT uuid_generate_v4() UNIQUE,
    user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    provider VARCHAR(50) NOT NULL,
    provider_user_id VARCHAR(255) NOT NULL,
    provider_username VARCHAR(255),
    provider_email VARCHAR(255),
    access_token TEXT,
    refresh_token TEXT,
    token_expires_at TIMESTAMPTZ,
    raw_data JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(provider, provider_user_id)
);

CREATE INDEX idx_sso_accounts_user_id ON sso_accounts(user_id);
CREATE INDEX idx_sso_accounts_provider_user ON sso_accounts(provider, provider_user_id);

-- Audit logs table
CREATE TABLE audit_logs (
    id BIGSERIAL PRIMARY KEY,
    uuid UUID NOT NULL DEFAULT uuid_generate_v4() UNIQUE,
    user_id BIGINT REFERENCES users(id) ON DELETE SET NULL,
    action VARCHAR(100) NOT NULL,
    entity_type VARCHAR(100) NOT NULL,
    entity_id BIGINT,
    entity_ref VARCHAR(200),
    old_value JSONB,
    new_value JSONB,
    ip_address VARCHAR(45),
    user_agent TEXT,
    request_id UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_audit_logs_user_id ON audit_logs(user_id);
CREATE INDEX idx_audit_logs_entity ON audit_logs(entity_type, entity_id);
CREATE INDEX idx_audit_logs_action ON audit_logs(action);
CREATE INDEX idx_audit_logs_created_at ON audit_logs(created_at);

-- Updated at trigger function
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Apply updated_at triggers
CREATE TRIGGER update_users_updated_at BEFORE UPDATE ON users FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_email_addresses_updated_at BEFORE UPDATE ON email_addresses FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_ssh_keys_updated_at BEFORE UPDATE ON ssh_keys FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_gpg_keys_updated_at BEFORE UPDATE ON gpg_keys FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_access_tokens_updated_at BEFORE UPDATE ON access_tokens FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_groups_updated_at BEFORE UPDATE ON groups FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_memberships_updated_at BEFORE UPDATE ON memberships FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_projects_updated_at BEFORE UPDATE ON projects FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_labels_updated_at BEFORE UPDATE ON labels FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_issues_updated_at BEFORE UPDATE ON issues FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_pull_requests_updated_at BEFORE UPDATE ON pull_requests FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_user_authorizations_updated_at BEFORE UPDATE ON user_authorizations FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_roles_updated_at BEFORE UPDATE ON roles FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_role_permissions_updated_at BEFORE UPDATE ON role_permissions FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_system_settings_updated_at BEFORE UPDATE ON system_settings FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_project_settings_updated_at BEFORE UPDATE ON project_settings FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_user_settings_updated_at BEFORE UPDATE ON user_settings FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_webhooks_updated_at BEFORE UPDATE ON webhooks FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_dashboards_updated_at BEFORE UPDATE ON dashboards FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_dashboard_shares_updated_at BEFORE UPDATE ON dashboard_shares FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_code_comments_updated_at BEFORE UPDATE ON code_comments FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_packs_updated_at BEFORE UPDATE ON packs FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_workspaces_updated_at BEFORE UPDATE ON workspaces FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_builds_updated_at BEFORE UPDATE ON builds FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_build_jobs_updated_at BEFORE UPDATE ON build_jobs FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_build_steps_updated_at BEFORE UPDATE ON build_steps FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_agents_updated_at BEFORE UPDATE ON agents FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_pipelines_updated_at BEFORE UPDATE ON pipelines FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_pipeline_schedules_updated_at BEFORE UPDATE ON pipeline_schedules FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_oauth_applications_updated_at BEFORE UPDATE ON oauth_applications FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_oauth_grants_updated_at BEFORE UPDATE ON oauth_grants FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_sso_accounts_updated_at BEFORE UPDATE ON sso_accounts FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- Insert default system settings
INSERT INTO system_settings (key, value, description, category, editable) VALUES
    ('site.name', '"OneDev"', 'Site name', 'general', true),
    ('site.url', '"http://localhost:46315"', 'Site base URL', 'general', true),
    ('site.admin_email', '"admin@localhost"', 'Admin email', 'general', true),
    ('security.enforce_2fa', 'false', 'Enforce two-factor authentication', 'security', true),
    ('security.password_min_length', '8', 'Minimum password length', 'security', true),
    ('security.session_timeout', '86400', 'Session timeout in seconds', 'security', true),
    ('email.enabled', 'false', 'Enable email notifications', 'email', true),
    ('email.from', '"noreply@onedev.io"', 'From email address', 'email', true),
    ('email.smtp_host', '"localhost"', 'SMTP host', 'email', true),
    ('email.smtp_port', '25', 'SMTP port', 'email', true),
    ('email.smtp_username', '""', 'SMTP username', 'email', true),
    ('email.smtp_password', '""', 'SMTP password', 'email', true),
    ('email.smtp_tls', 'false', 'Use TLS for SMTP', 'email', true),
    ('git.http_enabled', 'true', 'Enable Git HTTP', 'git', true),
    ('git.ssh_enabled', 'true', 'Enable Git SSH', 'git', true),
    ('git.ssh_port', '22', 'SSH port', 'git', true),
    ('git.lfs_enabled', 'true', 'Enable Git LFS', 'git', true),
    ('build.max_concurrent', '10', 'Max concurrent builds', 'build', true),
    ('build.default_timeout', '3600', 'Default build timeout in seconds', 'build', true),
    ('pack.max_file_size', '1073741824', 'Max package file size in bytes', 'pack', true),
    ('storage.path', '"/var/onedev/data"', 'Storage path', 'storage', true)
ON CONFLICT (key) DO NOTHING;

-- Insert default roles
INSERT INTO roles (name, description, is_system) VALUES
    ('admin', 'Full system administrator', true),
    ('user', 'Regular user', true)
ON CONFLICT (name) DO NOTHING;