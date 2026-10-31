-- PROPUESTA 0017 — autorización de provisioning (opción B del ADR-0025).
-- CANDIDATA: no forma parte del migrador Local productivo (`migrations/local`),
-- que solo embebe 0001. No ejecutar sin aceptación formal del contrato físico.
--
-- Depende de 0002 (installation_identity). El token en claro nunca se persiste:
-- solo su digest SHA-256. Eventos append-only sin secretos.

CREATE TABLE bootstrap_authorization (
    id TEXT PRIMARY KEY NOT NULL CHECK (
        length(id)=36
        AND substr(id,9,1)='-' AND substr(id,14,1)='-' AND substr(id,19,1)='-'
        AND substr(id,24,1)='-'
        AND length(replace(id,'-',''))=32
        AND replace(id,'-','') NOT GLOB '*[^0-9a-f]*'
    ),
    installation_id TEXT NOT NULL CHECK (
        length(installation_id)=36
        AND substr(installation_id,9,1)='-' AND substr(installation_id,14,1)='-'
        AND substr(installation_id,19,1)='-' AND substr(installation_id,24,1)='-'
        AND length(replace(installation_id,'-',''))=32
        AND replace(installation_id,'-','') NOT GLOB '*[^0-9a-f]*'
    ),
    purpose TEXT NOT NULL CHECK (
        purpose IN ('BOOTSTRAP','ADMINISTRATIVE_RECOVERY')
    ),
    -- Digest SHA-256 hexadecimal minúsculo; nunca el token.
    token_digest TEXT NOT NULL CHECK (
        length(token_digest)=64 AND token_digest NOT GLOB '*[^0-9a-f]*'
    ),
    issued_at INTEGER NOT NULL,
    -- Ventana admitida por el dominio: 60 s a 3600 s.
    expires_at INTEGER NOT NULL CHECK (
        expires_at > issued_at AND expires_at - issued_at BETWEEN 60 AND 3600
    ),
    consumed_at INTEGER CHECK (consumed_at IS NULL OR consumed_at >= issued_at),
    revoked_at INTEGER CHECK (revoked_at IS NULL OR revoked_at >= issued_at),
    contract_version INTEGER NOT NULL DEFAULT 1 CHECK (contract_version >= 1),
    updated_at INTEGER NOT NULL CHECK (updated_at >= issued_at),
    -- Consumo y revocación son estados terminales incompatibles entre sí.
    CHECK (consumed_at IS NULL OR revoked_at IS NULL),
    FOREIGN KEY (installation_id) REFERENCES installation_identity(id)
) STRICT;

-- Un digest identifica una sola autorización.
CREATE UNIQUE INDEX bootstrap_authorization_digest_unique
    ON bootstrap_authorization(token_digest);

-- Como máximo un derecho vivo por instalación y finalidad.
CREATE UNIQUE INDEX bootstrap_authorization_live_unique
    ON bootstrap_authorization(installation_id, purpose)
    WHERE consumed_at IS NULL AND revoked_at IS NULL;

CREATE TABLE bootstrap_authorization_event (
    id TEXT PRIMARY KEY NOT NULL CHECK (
        length(id)=36
        AND substr(id,9,1)='-' AND substr(id,14,1)='-' AND substr(id,19,1)='-'
        AND substr(id,24,1)='-'
        AND length(replace(id,'-',''))=32
        AND replace(id,'-','') NOT GLOB '*[^0-9a-f]*'
    ),
    authorization_id TEXT NOT NULL,
    event_type TEXT NOT NULL CHECK (
        event_type IN ('ISSUED','REVOKED','CONSUMED','REJECTED','EXPIRED')
    ),
    occurred_at INTEGER NOT NULL,
    -- Contexto no sensible (motivo, código de error estable); nunca token ni digest.
    security_context TEXT CHECK (
        security_context IS NULL OR (
            length(security_context) <= 200
            AND instr(security_context, char(0)) = 0
        )
    ),
    FOREIGN KEY (authorization_id) REFERENCES bootstrap_authorization(id)
) STRICT;

CREATE INDEX bootstrap_authorization_event_timeline
    ON bootstrap_authorization_event(authorization_id, occurred_at);
