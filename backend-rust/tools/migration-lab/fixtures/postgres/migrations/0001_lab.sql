-- Solo fixture técnico, no esquema de dominio F1-10/F1-13.
-- Se ejecuta en search_path=lab, con tracking lab._sqlx_migrations.
CREATE TABLE lab_record (
    id UUID PRIMARY KEY,
    tenant TEXT NOT NULL CHECK (tenant IN ('alpha', 'beta')),
    amount NUMERIC(19,4) NOT NULL,
    quantity NUMERIC(19,6) NOT NULL,
    payload TEXT NOT NULL
);
ALTER TABLE lab_record ENABLE ROW LEVEL SECURITY;
CREATE POLICY tenant_boundary ON lab_record
    USING (tenant = current_setting('lab.tenant', true))
    WITH CHECK (tenant = current_setting('lab.tenant', true));
