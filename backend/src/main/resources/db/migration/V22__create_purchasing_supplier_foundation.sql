-- Fundación de proveedores de Purchasing (v0.4.x).
-- Los proveedores son datos maestros de la organización. Nunca se eliminan:
-- INACTIVE conserva el historial futuro de órdenes y recepciones.

CREATE TABLE suppliers (
    id              UUID         NOT NULL PRIMARY KEY,
    organization_id UUID         NOT NULL REFERENCES organizations (id),
    code            VARCHAR(64)  NOT NULL,
    name            VARCHAR(120) NOT NULL CHECK (length(trim(name)) > 0),
    legal_name      VARCHAR(160),
    tax_id          VARCHAR(64),
    contact_name    VARCHAR(120),
    email           VARCHAR(254),
    phone           VARCHAR(40),
    address         VARCHAR(500),
    status          VARCHAR(16)  NOT NULL DEFAULT 'ACTIVE' CHECK (status IN ('ACTIVE', 'INACTIVE')),
    version         BIGINT       NOT NULL DEFAULT 0,
    created_at      TIMESTAMPTZ  NOT NULL,
    updated_at      TIMESTAMPTZ  NOT NULL,
    CONSTRAINT suppliers_organization_code_unique UNIQUE (organization_id, code),
    CONSTRAINT suppliers_code_format CHECK (code ~ '^[a-z0-9]+(-[a-z0-9]+)*$'),
    CONSTRAINT suppliers_legal_name_not_blank CHECK (legal_name IS NULL OR length(trim(legal_name)) > 0),
    CONSTRAINT suppliers_tax_id_not_blank CHECK (tax_id IS NULL OR length(trim(tax_id)) > 0),
    CONSTRAINT suppliers_contact_name_not_blank CHECK (contact_name IS NULL OR length(trim(contact_name)) > 0),
    CONSTRAINT suppliers_email_not_blank CHECK (email IS NULL OR length(trim(email)) > 0),
    CONSTRAINT suppliers_phone_not_blank CHECK (phone IS NULL OR length(trim(phone)) > 0),
    CONSTRAINT suppliers_address_not_blank CHECK (address IS NULL OR length(trim(address)) > 0),
    CONSTRAINT suppliers_id_organization_unique UNIQUE (id, organization_id)
);

CREATE INDEX idx_suppliers_organization_name ON suppliers (organization_id, name, id);

-- Las entradas de catálogo no alteran los grants de roles existentes. Un
-- bootstrap seguro nuevo recibe el catálogo completo; una instalación existente
-- debe concederlas de forma explícita.
INSERT INTO permissions (code, description) VALUES
    ('purchasing.supplier.read', 'List and read organization suppliers'),
    ('purchasing.supplier.write', 'Create and update organization suppliers');

COMMENT ON TABLE suppliers IS
    'Datos maestros de proveedores de Purchasing, propiedad de la organización. Se retiran mediante estado INACTIVE, sin eliminación física.';
COMMENT ON COLUMN suppliers.code IS
    'Slug inmutable en minúsculas, único dentro de la organización.';
