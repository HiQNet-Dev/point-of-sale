-- Upgrade sintético: debe preservar filas e identidad, no prueba paridad de negocio.
ALTER TABLE lab_parent ADD COLUMN note TEXT NOT NULL DEFAULT 'upgraded';
CREATE INDEX lab_child_parent_idx ON lab_child(parent_id);
