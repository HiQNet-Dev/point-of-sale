-- Aprobada para v0.4.0 (ADR-0026). Inmutable: toda corrección posterior exige
-- una migración nueva.
-- Autorización de provisioning (opción B del ADR-0025).
--
-- Secuencia resuelta: el token se emite ANTES de existir la primera organización.
--   provisional_installation  -> existe sin organización y sin identidad canónica
--   bootstrap_authorization   -> depende solo de la preautorización
--   bootstrap                 -> en una sola UoW crea organización + identidad +
--                                `installation_identity`, y marca `claimed_at`
-- No se crea una organización ficticia ni se desactivan claves foráneas.
--
-- Solo se persiste el digest SHA-256 del token; nunca el valor en claro.

CREATE TABLE provisional_installation (
    id TEXT PRIMARY KEY NOT NULL CHECK (
        length(id)=36
        AND substr(id,9,1)='-' AND substr(id,14,1)='-' AND substr(id,19,1)='-'
        AND substr(id,24,1)='-'
        AND length(replace(id,'-',''))=32
        AND replace(id,'-','') NOT GLOB '*[^0-9a-f]*'
    ),
    -- Huella pública de la instalación; nunca material de clave privada.
    fingerprint TEXT NOT NULL CHECK (
        instr(fingerprint, char(0)) = 0
        AND length(fingerprint) BETWEEN 1 AND 128
    ),
    created_at INTEGER NOT NULL,
    claimed_at INTEGER,
    claimed_installation_id TEXT UNIQUE REFERENCES installation_identity(id),
    -- Una preautorización reclamada apunta a una identidad canónica y viceversa.
    CHECK ((claimed_at IS NULL) = (claimed_installation_id IS NULL)),
    CHECK (claimed_at IS NULL OR claimed_at >= created_at)
) STRICT;

CREATE TABLE bootstrap_authorization (
    id TEXT PRIMARY KEY NOT NULL CHECK (
        length(id)=36
        AND substr(id,9,1)='-' AND substr(id,14,1)='-' AND substr(id,19,1)='-'
        AND substr(id,24,1)='-'
        AND length(replace(id,'-',''))=32
        AND replace(id,'-','') NOT GLOB '*[^0-9a-f]*'
    ),
    installation_id TEXT NOT NULL REFERENCES provisional_installation(id),
    purpose TEXT NOT NULL CHECK (
        purpose IN ('BOOTSTRAP','ADMINISTRATIVE_RECOVERY')
    ),
    -- Digest SHA-256 hexadecimal minúsculo; nunca el token.
    token_digest TEXT NOT NULL UNIQUE CHECK (
        length(token_digest)=64 AND token_digest NOT GLOB '*[^0-9a-f]*'
    ),
    issued_at INTEGER NOT NULL,
    -- Ventana admitida por el dominio: 60 s a 3600 s.
    expires_at INTEGER NOT NULL CHECK (
        expires_at > issued_at AND expires_at - issued_at BETWEEN 60 AND 3600
    ),
    state TEXT NOT NULL DEFAULT 'PENDING' CHECK (
        state IN ('PENDING','CONSUMED','REVOKED','EXPIRED')
    ),
    -- Instante de cierre: consumo, revocación o caducidad materializada.
    closed_at INTEGER,
    contract_version INTEGER NOT NULL DEFAULT 1 CHECK (contract_version >= 1),
    updated_at INTEGER NOT NULL CHECK (updated_at >= issued_at),
    -- PENDING no tiene cierre; todo estado terminal sí, y nunca anterior.
    CHECK (
        (state = 'PENDING' AND closed_at IS NULL)
        OR (state <> 'PENDING' AND closed_at IS NOT NULL)
    ),
    CHECK (closed_at IS NULL OR closed_at >= issued_at),
    -- Una caducidad materializada no puede cerrarse antes de expirar.
    CHECK (state <> 'EXPIRED' OR closed_at >= expires_at)
) STRICT;

-- Como máximo un derecho vivo por instalación y finalidad. Al caducar o
-- revocarse, el índice deja de cubrirlo y permite reemitir.
CREATE UNIQUE INDEX bootstrap_authorization_live_unique
    ON bootstrap_authorization(installation_id, purpose)
    WHERE state = 'PENDING';

CREATE TABLE bootstrap_authorization_event (
    id TEXT PRIMARY KEY NOT NULL CHECK (
        length(id)=36
        AND substr(id,9,1)='-' AND substr(id,14,1)='-' AND substr(id,19,1)='-'
        AND substr(id,24,1)='-'
        AND length(replace(id,'-',''))=32
        AND replace(id,'-','') NOT GLOB '*[^0-9a-f]*'
    ),
    authorization_id TEXT NOT NULL REFERENCES bootstrap_authorization(id),
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
    )
) STRICT;

CREATE INDEX bootstrap_authorization_event_timeline
    ON bootstrap_authorization_event(authorization_id, occurred_at);

-- El rastro de autorización es append-only: no se reescribe ni se borra.
CREATE TRIGGER bootstrap_authorization_event_no_update
BEFORE UPDATE ON bootstrap_authorization_event
BEGIN
    SELECT RAISE(ABORT, 'bootstrap_authorization_event is append-only');
END;

CREATE TRIGGER bootstrap_authorization_event_no_delete
BEFORE DELETE ON bootstrap_authorization_event
BEGIN
    SELECT RAISE(ABORT, 'bootstrap_authorization_event is append-only');
END;
