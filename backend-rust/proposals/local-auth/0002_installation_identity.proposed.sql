-- PROPUESTA D-INST-01/02. No ejecutar sin aceptación formal.
-- Depende de 0001 (organization). Debe aplicarse ANTES de 0007 bootstrap_record.
--
-- Transcripción de F1-10 §16.3. No se añaden campos que la ficha no declare.
-- La clave privada Ed25519 nunca se persiste en SQLite: permanece en el
-- almacenamiento seguro del sistema operativo definido por la arquitectura.
--
-- Decisiones pendientes (no resueltas aquí):
--   D-INST-01: catálogo completo de `status`. Solo `ACTIVE` está confirmado.
--   D-INST-02: ámbito de unicidad de `public_key_fingerprint` (global o por
--              organización) y formato exacto de la huella.

CREATE TABLE installation_identity (
    id TEXT PRIMARY KEY NOT NULL CHECK (
        instr(id,char(0))=0 AND length(id)=36 AND substr(id,9,1)='-' AND substr(id,14,1)='-'
        AND substr(id,19,1)='-' AND substr(id,24,1)='-'
        AND length(replace(id,'-',''))=32 AND replace(id,'-','') NOT GLOB '*[^0-9a-f]*'
    ),
    organization_id TEXT NOT NULL REFERENCES organization(id),
    generation INTEGER NOT NULL DEFAULT 1 CHECK (generation >= 1),
    public_key_fingerprint TEXT NOT NULL CHECK (
        instr(public_key_fingerprint,char(0))=0
        AND length(public_key_fingerprint) BETWEEN 1 AND 128
    ),
    status TEXT NOT NULL DEFAULT 'ACTIVE' CHECK (status IN ('ACTIVE')),
    initialized_at TEXT NOT NULL CHECK (
        initialized_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]Z'
    ),
    updated_at TEXT NOT NULL CHECK (
        updated_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]Z'
    ),
    -- Superclave para la FK compuesta de ownership en 0007.
    UNIQUE (id, organization_id)
) STRICT;
