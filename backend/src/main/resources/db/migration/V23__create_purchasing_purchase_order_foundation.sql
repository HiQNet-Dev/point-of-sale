-- Fundación de órdenes de compra de Purchasing (v0.4.x, PR 2.1).
--
-- Una orden de compra pertenece a una organización y apunta a una sucursal de
-- destino obligatoria. En esta migración el agregado solo puede existir en
-- DRAFT: las transiciones ORDERED y CANCELLED se implementan en el PR 2.2, por
-- lo que el CHECK de estado ya las contempla pero ningún camino de código las
-- alcanza todavía.
--
-- No se crean movimientos de inventario ni tablas de recepción: la recepción
-- de mercancía es un incremento posterior. V1-V22 son inmutables.

CREATE TABLE purchase_orders (
    id                     UUID         NOT NULL PRIMARY KEY,
    organization_id        UUID         NOT NULL REFERENCES organizations (id),
    branch_id              UUID         NOT NULL,
    supplier_id            UUID         NOT NULL,
    code                   VARCHAR(64)  NOT NULL,
    status                 VARCHAR(16)  NOT NULL DEFAULT 'DRAFT'
        CHECK (status IN ('DRAFT', 'ORDERED', 'CANCELLED')),
    currency               VARCHAR(3)   NOT NULL,
    supplier_code_snapshot VARCHAR(64)  NOT NULL,
    supplier_name_snapshot VARCHAR(120) NOT NULL,
    created_by_user_id     UUID         NOT NULL,
    issued_by_user_id      UUID,
    cancelled_by_user_id   UUID,
    version                BIGINT       NOT NULL DEFAULT 0,
    created_at             TIMESTAMPTZ  NOT NULL,
    updated_at             TIMESTAMPTZ  NOT NULL,
    issued_at              TIMESTAMPTZ,
    cancelled_at           TIMESTAMPTZ,
    CONSTRAINT purchase_orders_organization_code_unique UNIQUE (organization_id, code),
    CONSTRAINT purchase_orders_id_organization_unique   UNIQUE (id, organization_id),
    CONSTRAINT purchase_orders_code_format
        CHECK (code ~ '^[a-z0-9]+(-[a-z0-9]+)*$'),
    CONSTRAINT purchase_orders_currency_format
        CHECK (currency ~ '^[A-Z]{3}$'),
    CONSTRAINT purchase_orders_supplier_name_snapshot_not_blank
        CHECK (length(trim(supplier_name_snapshot)) > 0),
    CONSTRAINT purchase_orders_branch_fk
        FOREIGN KEY (branch_id, organization_id) REFERENCES branches (id, organization_id),
    CONSTRAINT purchase_orders_supplier_fk
        FOREIGN KEY (supplier_id, organization_id) REFERENCES suppliers (id, organization_id),
    CONSTRAINT purchase_orders_created_by_fk
        FOREIGN KEY (created_by_user_id, organization_id) REFERENCES users (id, organization_id),
    CONSTRAINT purchase_orders_issued_by_fk
        FOREIGN KEY (issued_by_user_id, organization_id) REFERENCES users (id, organization_id),
    CONSTRAINT purchase_orders_cancelled_by_fk
        FOREIGN KEY (cancelled_by_user_id, organization_id) REFERENCES users (id, organization_id),
    -- Coherencia de estado: cada estado exige exactamente los actores y
    -- timestamps que le corresponden. DRAFT es el único alcanzable en V23.
    CONSTRAINT purchase_orders_state_consistency CHECK (
        (status = 'DRAFT'
            AND issued_at IS NULL AND issued_by_user_id IS NULL
            AND cancelled_at IS NULL AND cancelled_by_user_id IS NULL)
        OR (status = 'ORDERED'
            AND issued_at IS NOT NULL AND issued_by_user_id IS NOT NULL
            AND cancelled_at IS NULL AND cancelled_by_user_id IS NULL)
        OR (status = 'CANCELLED'
            AND cancelled_at IS NOT NULL AND cancelled_by_user_id IS NOT NULL)
    )
);

CREATE TABLE purchase_order_lines (
    id                 UUID          NOT NULL PRIMARY KEY,
    organization_id    UUID          NOT NULL,
    purchase_order_id  UUID          NOT NULL,
    inventory_item_id  UUID          NOT NULL,
    line_number        INTEGER       NOT NULL CHECK (line_number > 0),
    item_code_snapshot VARCHAR(64)   NOT NULL,
    item_name_snapshot VARCHAR(120)  NOT NULL CHECK (length(trim(item_name_snapshot)) > 0),
    unit_snapshot      VARCHAR(16)   NOT NULL CHECK (unit_snapshot IN ('UNIT', 'GRAM', 'MILLILITER')),
    quantity           NUMERIC(19,6) NOT NULL CHECK (quantity > 0),
    unit_cost          NUMERIC(19,4) CHECK (unit_cost IS NULL OR unit_cost >= 0),
    created_at         TIMESTAMPTZ   NOT NULL,
    updated_at         TIMESTAMPTZ   NOT NULL,
    CONSTRAINT purchase_order_lines_order_line_unique UNIQUE (purchase_order_id, line_number),
    CONSTRAINT purchase_order_lines_order_item_unique UNIQUE (purchase_order_id, inventory_item_id),
    CONSTRAINT purchase_order_lines_id_organization_unique UNIQUE (id, organization_id),
    CONSTRAINT purchase_order_lines_order_fk
        FOREIGN KEY (purchase_order_id, organization_id)
        REFERENCES purchase_orders (id, organization_id),
    CONSTRAINT purchase_order_lines_item_fk
        FOREIGN KEY (inventory_item_id, organization_id)
        REFERENCES inventory_items (id, organization_id)
);

CREATE INDEX idx_purchase_orders_scope_status_created
    ON purchase_orders (organization_id, branch_id, status, created_at DESC, id DESC);
CREATE INDEX idx_purchase_orders_supplier
    ON purchase_orders (organization_id, supplier_id);
CREATE INDEX idx_purchase_order_lines_order_line
    ON purchase_order_lines (purchase_order_id, line_number);

-- Las entradas de catálogo no alteran los grants de roles existentes. Un
-- bootstrap seguro nuevo recibe el catálogo completo; una instalación existente
-- debe concederlas de forma explícita.
INSERT INTO permissions (code, description) VALUES
    ('purchasing.order.read',   'List and read branch purchase orders'),
    ('purchasing.order.write',  'Create and edit draft purchase orders and their lines'),
    ('purchasing.order.issue',  'Issue a draft purchase order to its supplier'),
    ('purchasing.order.cancel', 'Cancel a draft or issued purchase order');

COMMENT ON TABLE purchase_orders IS
    'Orden de compra organization-owned con sucursal de destino obligatoria. En V23 solo se persiste DRAFT; ORDERED y CANCELLED se habilitan en un incremento posterior.';
COMMENT ON TABLE purchase_order_lines IS
    'Partidas de una orden de compra con snapshots inmutables de código, nombre y unidad del InventoryItem. Editables solo mientras la orden está en DRAFT.';
COMMENT ON COLUMN purchase_orders.currency IS
    'Moneda ISO 4217 copiada de la organización al crear la orden; el cliente no puede fijarla ni modificarla.';
COMMENT ON COLUMN purchase_order_lines.unit_cost IS
    'Precio unitario pactado opcional (NUMERIC(19,4), >= 0). No representa valoración de inventario ni cuentas por pagar.';
COMMENT ON COLUMN purchase_order_lines.unit_snapshot IS
    'Unidad base del InventoryItem en el momento de capturar la partida; preserva el historial aunque el catálogo cambie.';
