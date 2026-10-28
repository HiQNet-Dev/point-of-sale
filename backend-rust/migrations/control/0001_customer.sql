-- F1–10 §16.28. Base separada; ninguna FK alcanza Local ni POS Central.
-- Subconjunto de laboratorio: no implementa billing, licencias ni identidad administrativa.
CREATE TABLE customer (
    id UUID PRIMARY KEY,
    status TEXT NOT NULL DEFAULT 'ACTIVE' CHECK (status IN ('ACTIVE','CLOSED')),
    identity_verified_at TIMESTAMPTZ,
    version INTEGER NOT NULL DEFAULT 0 CHECK (version >= 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
