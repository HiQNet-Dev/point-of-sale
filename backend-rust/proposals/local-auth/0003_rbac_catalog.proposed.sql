-- PROPUESTA D-AUTH-06. No ejecutar sin aceptación formal.
-- Depende de 0002 (pos_user) y 0001 (organization).
-- Catálogo: 59 códigos exactos del backend Java (V5-V25), inventariados en
-- docs/05-development/rust-parity-baseline.md §4.3. No concede nada a roles.

CREATE TABLE permission (
    code TEXT PRIMARY KEY NOT NULL CHECK (
        length(code) BETWEEN 3 AND 64 AND code NOT GLOB '*[^a-z0-9._-]*'
    ),
    description TEXT NOT NULL CHECK (length(trim(description)) > 0)
) STRICT;

CREATE TABLE role (
    id TEXT PRIMARY KEY NOT NULL CHECK (
        instr(id,char(0))=0 AND length(id)=36 AND substr(id,9,1)='-' AND substr(id,14,1)='-'
        AND substr(id,19,1)='-' AND substr(id,24,1)='-'
        AND length(replace(id,'-',''))=32 AND replace(id,'-','') NOT GLOB '*[^0-9a-f]*'
    ),
    organization_id TEXT NOT NULL REFERENCES organization(id),
    code TEXT NOT NULL CHECK (
        length(code) BETWEEN 1 AND 64 AND code NOT GLOB '*[^a-z0-9-]*'
    ),
    name TEXT NOT NULL CHECK (length(trim(name)) > 0 AND length(name) <= 120),
    status TEXT NOT NULL DEFAULT 'ACTIVE' CHECK (status IN ('ACTIVE','INACTIVE')),
    version INTEGER NOT NULL DEFAULT 0 CHECK (version >= 0),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE (organization_id, code),
    UNIQUE (id, organization_id)
) STRICT;

CREATE TABLE role_permission (
    role_id TEXT NOT NULL REFERENCES role(id),
    permission_code TEXT NOT NULL REFERENCES permission(code),
    PRIMARY KEY (role_id, permission_code)
) STRICT;

CREATE TABLE user_organization_role (
    organization_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    role_id TEXT NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY (organization_id, user_id, role_id),
    FOREIGN KEY (user_id, organization_id) REFERENCES pos_user(id, organization_id),
    FOREIGN KEY (role_id, organization_id) REFERENCES role(id, organization_id)
) STRICT;

CREATE INDEX idx_role_organization ON role (organization_id, status);
CREATE INDEX idx_user_organization_role_user ON user_organization_role (organization_id, user_id);

-- Seed determinista e idempotente: catálogo de sistema, nunca modificado por el
-- cliente. No se auto-concede a roles existentes.
INSERT INTO permission (code, description) VALUES
    ('organization.read', 'Read organization details'),
    ('organization.update', 'Update organization details'),
    ('branch.read', 'List and read branches'),
    ('branch.create', 'Create branches'),
    ('branch.update', 'Update branches'),
    ('branch.membership.read', 'Read branch memberships'),
    ('branch.membership.create', 'Create branch memberships'),
    ('branch.membership.update', 'Update branch memberships'),
    ('branch.membership.roles.manage', 'Replace a membership''s branch role set'),
    ('user.read', 'List and read users'),
    ('user.create', 'Create users'),
    ('user.update', 'Update users'),
    ('permission.read', 'List the permission catalog'),
    ('role.read', 'List and read roles and role assignments'),
    ('role.create', 'Create roles'),
    ('role.update', 'Update roles'),
    ('role.permissions.manage', 'Replace a role''s permissions'),
    ('role.assign', 'Replace a user''s organization role assignments'),
    ('audit.read', 'Read the organization audit log'),
    ('category.read', 'List and read categories'),
    ('category.create', 'Create categories'),
    ('category.update', 'Update categories'),
    ('product.read', 'List and read products'),
    ('product.create', 'Create products'),
    ('product.update', 'Update products'),
    ('product.modifiers.manage', 'Replace the modifier groups assigned to a product'),
    ('modifier.group.read', 'List and read modifier groups'),
    ('modifier.group.create', 'Create modifier groups'),
    ('modifier.group.update', 'Update modifier groups'),
    ('modifier.option.read', 'List and read modifier options'),
    ('modifier.option.create', 'Create modifier options'),
    ('modifier.option.update', 'Update modifier options'),
    ('cash-register.read', 'List and read cash registers of a branch'),
    ('cash-register.create', 'Create cash registers in a branch'),
    ('cash-register.update', 'Update cash registers (name, status)'),
    ('cash-shift.read', 'List and read cash shifts of a cash register'),
    ('cash-shift.open', 'Open a cash shift on a cash register'),
    ('cash-shift.close', 'Close an open cash shift'),
    ('sale.read', 'List and read sales of a cash shift'),
    ('sale.create', 'Create an OPEN sale on a cash shift'),
    ('sale.update', 'Add, update and remove items of an OPEN sale'),
    ('sale.cancel', 'Cancel an OPEN sale for operational recovery'),
    ('sale.checkout', 'Complete an OPEN sale through atomic checkout'),
    ('payment.read', 'Read historical payments of a completed sale'),
    ('pos.catalog.read', 'Discover organization catalog for POS operations in an authorized branch'),
    ('inventory.item.read', 'List and read organization inventory item definitions'),
    ('inventory.item.write', 'Create and update organization inventory item definitions'),
    ('inventory.stock.read', 'Read branch inventory balances and movement history'),
    ('inventory.stock.adjust', 'Open stock and make manual branch inventory adjustments'),
    ('inventory.recipe.read', 'Read product Inventory recipes and modifier Inventory effects'),
    ('inventory.recipe.write', 'Create and replace product Inventory recipes and modifier effects'),
    ('purchasing.supplier.read', 'List and read organization suppliers'),
    ('purchasing.supplier.write', 'Create and update organization suppliers'),
    ('purchasing.order.read', 'List and read branch purchase orders'),
    ('purchasing.order.write', 'Create and edit draft purchase orders and their lines'),
    ('purchasing.order.issue', 'Issue a draft purchase order to its supplier'),
    ('purchasing.order.cancel', 'Cancel a draft or issued purchase order'),
    ('purchasing.receipt.read', 'List and read branch purchase receipts'),
    ('purchasing.receipt.write', 'Post purchase receipts that increase branch stock')
ON CONFLICT (code) DO NOTHING;
