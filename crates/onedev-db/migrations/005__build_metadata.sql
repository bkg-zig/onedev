-- Build metadata set while a build runs (SetBuildVersionStep /
-- SetBuildDescriptionStep) and shown next to the build number.
ALTER TABLE builds
    ADD COLUMN IF NOT EXISTS version VARCHAR(255),
    ADD COLUMN IF NOT EXISTS description TEXT;

CREATE INDEX IF NOT EXISTS idx_builds_project_version
    ON builds (project_id, version)
    WHERE version IS NOT NULL;
