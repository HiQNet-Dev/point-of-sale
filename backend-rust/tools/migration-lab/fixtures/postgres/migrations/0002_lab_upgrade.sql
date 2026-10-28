-- N-1 -> N sintético: preserva UUID, tenant, decimales exactos y payload.
ALTER TABLE lab_record ADD COLUMN note TEXT NOT NULL DEFAULT 'upgraded';
CREATE INDEX lab_record_tenant_idx ON lab_record (tenant);
