-- Users administration hardening.
--
-- V1/V2/V3 are published and immutable; this is an additive constraint only.
-- The domain already normalizes emails (trim + lowercase) and rejects malformed
-- values, but PostgreSQL is the final authority: enforce the normalization
-- invariant at the database level as defense in depth. The existing
-- UNIQUE (organization_id, email) — declared in V3 — is kept as is.
--
-- No new tables/columns: the only gap after V3 was the absence of a CHECK
-- guaranteeing that the persisted email equals its trimmed, lowercased form.

ALTER TABLE users
    ADD CONSTRAINT users_email_normalized_check
    CHECK (email = lower(email) AND email = trim(email) AND length(email) > 0);
