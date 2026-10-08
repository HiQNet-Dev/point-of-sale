-- Initial migration.
-- Deliberately minimal: only a technical metadata table so the pipeline
-- (connection -> Flyway -> PostgreSQL -> Hibernate validate) is verifiable.
-- Domain modeling (organization, branches, sales, ...) happens in later tasks.
CREATE TABLE app_metadata (
    key        VARCHAR(64)  NOT NULL PRIMARY KEY,
    value      VARCHAR(255) NOT NULL,
    created_at TIMESTAMPTZ  NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ  NOT NULL DEFAULT now()
);

COMMENT ON TABLE app_metadata IS
    'Technical key/value metadata for operations. Not business data.';
