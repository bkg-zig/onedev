-- Server background services: webhook delivery queue, full-text search indexes,
-- build logs / artifacts, git maintenance bookkeeping and queue notifications.

-- ---------------------------------------------------------------------------
-- Webhook deliveries (outbox processed by the webhook dispatcher)
-- ---------------------------------------------------------------------------
CREATE TYPE webhook_delivery_status AS ENUM ('pending', 'sending', 'delivered', 'abandoned');

ALTER TABLE webhooks
    ADD COLUMN IF NOT EXISTS headers JSONB NOT NULL DEFAULT '{}'::jsonb;

CREATE TABLE webhook_deliveries (
    id BIGSERIAL PRIMARY KEY,
    uuid UUID NOT NULL DEFAULT uuid_generate_v4() UNIQUE,
    webhook_id BIGINT NOT NULL REFERENCES webhooks(id) ON DELETE CASCADE,
    event VARCHAR(100) NOT NULL,
    payload JSONB NOT NULL,
    url TEXT NOT NULL,
    status webhook_delivery_status NOT NULL DEFAULT 'pending',
    attempt INTEGER NOT NULL DEFAULT 0,
    last_attempt_at TIMESTAMPTZ,
    next_retry_at TIMESTAMPTZ,
    response_status INTEGER,
    response_body TEXT,
    error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    delivered_at TIMESTAMPTZ
);

CREATE INDEX idx_webhook_deliveries_due
    ON webhook_deliveries (next_retry_at NULLS FIRST, id)
    WHERE status = 'pending';
CREATE INDEX idx_webhook_deliveries_sending
    ON webhook_deliveries (last_attempt_at)
    WHERE status = 'sending';
CREATE INDEX idx_webhook_deliveries_webhook
    ON webhook_deliveries (webhook_id, created_at DESC);
CREATE INDEX idx_webhook_deliveries_created
    ON webhook_deliveries (created_at);

-- ---------------------------------------------------------------------------
-- Full-text search (issues, pull requests, projects, code comments)
-- ---------------------------------------------------------------------------
CREATE TABLE search_index (
    entity_type VARCHAR(32) NOT NULL,
    entity_id BIGINT NOT NULL,
    project_id BIGINT REFERENCES projects(id) ON DELETE CASCADE,
    title TEXT NOT NULL DEFAULT '',
    content TEXT NOT NULL DEFAULT '',
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    tsv TSVECTOR GENERATED ALWAYS AS (
        setweight(to_tsvector('simple', left(title, 1000)), 'A') ||
        setweight(to_tsvector('simple', left(content, 400000)), 'B')
    ) STORED,
    indexed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (entity_type, entity_id)
);

CREATE INDEX idx_search_index_tsv ON search_index USING GIN (tsv);
CREATE INDEX idx_search_index_project ON search_index (project_id);

-- Code search: one row per file of the indexed branch head
CREATE TABLE code_search_index (
    project_id BIGINT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    path TEXT NOT NULL,
    blob_sha VARCHAR(64) NOT NULL,
    size BIGINT NOT NULL,
    content TEXT NOT NULL,
    tsv TSVECTOR GENERATED ALWAYS AS (
        setweight(to_tsvector('simple', path), 'A') ||
        setweight(to_tsvector('simple', left(content, 400000)), 'B')
    ) STORED,
    indexed_commit VARCHAR(64) NOT NULL,
    indexed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (project_id, path)
);

CREATE INDEX idx_code_search_index_tsv ON code_search_index USING GIN (tsv);

CREATE TABLE code_index_state (
    project_id BIGINT PRIMARY KEY REFERENCES projects(id) ON DELETE CASCADE,
    branch VARCHAR(255) NOT NULL,
    indexed_commit VARCHAR(64) NOT NULL,
    indexed_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Commit search: commit messages of the indexed branch
CREATE TABLE commit_search_index (
    project_id BIGINT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    commit_sha VARCHAR(64) NOT NULL,
    author_name TEXT NOT NULL DEFAULT '',
    author_email TEXT NOT NULL DEFAULT '',
    message TEXT NOT NULL DEFAULT '',
    committed_at TIMESTAMPTZ,
    tsv TSVECTOR GENERATED ALWAYS AS (
        setweight(to_tsvector('simple', left(message, 20000)), 'A') ||
        setweight(to_tsvector('simple', author_name || ' ' || author_email), 'C')
    ) STORED,
    indexed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (project_id, commit_sha)
);

CREATE INDEX idx_commit_search_index_tsv ON commit_search_index USING GIN (tsv);

-- ---------------------------------------------------------------------------
-- Build output
-- ---------------------------------------------------------------------------
CREATE TABLE build_logs (
    id BIGSERIAL PRIMARY KEY,
    build_id BIGINT NOT NULL REFERENCES builds(id) ON DELETE CASCADE,
    seq BIGINT NOT NULL,
    job_name VARCHAR(200) NOT NULL,
    step_name VARCHAR(200) NOT NULL DEFAULT '',
    stream VARCHAR(6) NOT NULL DEFAULT 'stdout' CHECK (stream IN ('stdout', 'stderr')),
    line TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (build_id, seq)
);

CREATE INDEX idx_build_logs_created ON build_logs (created_at);

CREATE TABLE build_artifacts (
    id BIGSERIAL PRIMARY KEY,
    build_id BIGINT NOT NULL REFERENCES builds(id) ON DELETE CASCADE,
    job_name VARCHAR(200) NOT NULL,
    name TEXT NOT NULL,
    storage_path TEXT NOT NULL,
    size BIGINT NOT NULL,
    sha256 VARCHAR(64),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (build_id, job_name, name)
);

CREATE INDEX idx_build_artifacts_created ON build_artifacts (created_at);

-- Scheduler claim / stale-detection access paths
CREATE INDEX idx_builds_pending ON builds (created_at, id) WHERE status = 'pending';
CREATE INDEX idx_builds_running ON builds (updated_at) WHERE status = 'running';

-- ---------------------------------------------------------------------------
-- Git maintenance bookkeeping
-- ---------------------------------------------------------------------------
ALTER TABLE projects
    ADD COLUMN IF NOT EXISTS gc_last_run TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS gc_last_stats JSONB;

-- ---------------------------------------------------------------------------
-- Queue wake-ups (LISTEN/NOTIFY). Workers also poll, notifications only
-- reduce latency, so a lost notification is harmless.
-- ---------------------------------------------------------------------------
CREATE OR REPLACE FUNCTION notify_build_queued() RETURNS TRIGGER AS $$
BEGIN
    PERFORM pg_notify('onedev_build_queued', NEW.id::text);
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER builds_notify_queued
    AFTER INSERT ON builds
    FOR EACH ROW WHEN (NEW.status = 'pending')
    EXECUTE FUNCTION notify_build_queued();

CREATE OR REPLACE FUNCTION notify_webhook_queued() RETURNS TRIGGER AS $$
BEGIN
    PERFORM pg_notify('onedev_webhook_queued', NEW.id::text);
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER webhook_deliveries_notify_queued
    AFTER INSERT ON webhook_deliveries
    FOR EACH ROW WHEN (NEW.status = 'pending')
    EXECUTE FUNCTION notify_webhook_queued();
