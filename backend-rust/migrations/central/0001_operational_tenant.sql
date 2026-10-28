-- F1–10 §16.25. Subconjunto de laboratorio, no activa Central ni sincronización.
-- UUID emitido por el llamador; no se regenera la identidad importada.
CREATE TABLE operational_tenant (
    id UUID PRIMARY KEY,
    external_organization_id UUID NOT NULL UNIQUE,
    status TEXT NOT NULL DEFAULT 'ACTIVE'
        CHECK (status IN ('ACTIVE','SUSPENDED','DECOMMISSIONED')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
