-- Aprobada para v0.4.0 (ADR-0026). Inmutable: toda corrección posterior exige
-- una migración nueva.
-- Turno histórico de una caja. Depende de 0003 (pos_user) y 0009 (cash_register).
-- Los importes son TEXT decimal canónico escala 4 para conservar NUMERIC(19,4)
-- sin REAL ni el límite de i64 de SQLite. Son declaraciones del operador, no
-- conciliaciones ni movimientos de efectivo.

CREATE TABLE cash_shift (
    id TEXT PRIMARY KEY NOT NULL CHECK (
        instr(id,char(0))=0 AND length(id)=36 AND substr(id,9,1)='-' AND substr(id,14,1)='-'
        AND substr(id,19,1)='-' AND substr(id,24,1)='-'
        AND length(replace(id,'-',''))=32 AND replace(id,'-','') NOT GLOB '*[^0-9a-f]*'
    ),
    organization_id TEXT NOT NULL REFERENCES organization(id),
    branch_id TEXT NOT NULL,
    cash_register_id TEXT NOT NULL,
    opened_by_user_id TEXT NOT NULL,
    closed_by_user_id TEXT,
    status TEXT NOT NULL CHECK (status IN ('OPEN','CLOSED')),
    opening_amount TEXT NOT NULL CHECK (
        instr(opening_amount,char(0))=0
        AND opening_amount NOT GLOB '*[^0-9.]*'
        AND substr(opening_amount,1,1) GLOB '[0-9]'
        AND length(opening_amount) BETWEEN 6 AND 20
        AND length(opening_amount) - length(replace(opening_amount,'.','')) = 1
        AND substr(opening_amount,-5,1) = '.'
        AND (substr(opening_amount,1,1) <> '0' OR length(opening_amount) = 6)
    ),
    closing_declared_amount TEXT CHECK (
        closing_declared_amount IS NULL OR (
            instr(closing_declared_amount,char(0))=0
            AND closing_declared_amount NOT GLOB '*[^0-9.]*'
            AND substr(closing_declared_amount,1,1) GLOB '[0-9]'
            AND length(closing_declared_amount) BETWEEN 6 AND 20
            AND length(closing_declared_amount) - length(replace(closing_declared_amount,'.','')) = 1
            AND substr(closing_declared_amount,-5,1) = '.'
            AND (substr(closing_declared_amount,1,1) <> '0' OR length(closing_declared_amount) = 6)
        )
    ),
    opened_at TEXT NOT NULL CHECK (
        opened_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]Z'
    ),
    closed_at TEXT CHECK (
        closed_at IS NULL OR closed_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]Z'
    ),
    version INTEGER NOT NULL DEFAULT 0 CHECK (version >= 0),
    created_at TEXT NOT NULL CHECK (
        created_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]Z'
    ),
    updated_at TEXT NOT NULL CHECK (
        updated_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]Z'
    ),
    CONSTRAINT cash_shift_register_fk
        FOREIGN KEY (cash_register_id, organization_id, branch_id)
        REFERENCES cash_register (id, organization_id, branch_id),
    CONSTRAINT cash_shift_opened_by_fk
        FOREIGN KEY (opened_by_user_id, organization_id) REFERENCES pos_user (id, organization_id),
    CONSTRAINT cash_shift_closed_by_fk
        FOREIGN KEY (closed_by_user_id, organization_id) REFERENCES pos_user (id, organization_id),
    CONSTRAINT cash_shift_state_consistency CHECK (
        (status = 'OPEN' AND closed_at IS NULL AND closed_by_user_id IS NULL AND closing_declared_amount IS NULL)
        OR
        (status = 'CLOSED' AND closed_at IS NOT NULL AND closed_by_user_id IS NOT NULL AND closing_declared_amount IS NOT NULL)
    ),
    -- Superclave que el próximo candidato de ventas reutilizará para ligar la
    -- venta con turno, caja, sucursal y organización en una sola FK.
    CONSTRAINT cash_shift_id_scope_unique UNIQUE (id, organization_id, branch_id, cash_register_id)
) STRICT;

-- Invariante de autoridad: el SELECT de experiencia nunca sustituye este índice.
CREATE UNIQUE INDEX cash_shift_one_open_per_register
    ON cash_shift (cash_register_id)
    WHERE status = 'OPEN';

CREATE INDEX idx_cash_shift_register_opened
    ON cash_shift (cash_register_id, opened_at DESC, id DESC);
