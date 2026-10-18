-- Recepción de mercancía de órdenes de compra con actualización atómica de
-- existencias (v0.4.x).
--
-- Una recepción pertenece a una orden emitida y a su sucursal de destino,
-- contiene una o más líneas con cantidades positivas expresadas en la unidad
-- base congelada en la partida, y crea exactamente un movimiento positivo de
-- inventario por línea. La identidad idempotente se impone en PostgreSQL
-- (organization_id, purchase_order_id, idempotency_key). Además PostgreSQL
-- garantiza que el movimiento comparta organización, sucursal y artículo con su
-- línea de recepción, que la línea pertenezca a la misma sucursal que la cabecera,
-- y que cabecera y líneas sean inmutables (append-only, como el ledger).
-- V1-V24 son inmutables.

-- 1. Superclaves compuestas requeridas por las FKs multisucursal de recepción.
ALTER TABLE purchase_orders
    ADD CONSTRAINT purchase_orders_id_organization_branch_unique
        UNIQUE (id, organization_id, branch_id);

ALTER TABLE purchase_order_lines
    ADD CONSTRAINT purchase_order_lines_id_organization_order_unique
        UNIQUE (id, organization_id, purchase_order_id);

-- 2. Enlace del ledger a la línea de recepción que originó el movimiento.
ALTER TABLE inventory_movements
    ADD COLUMN purchase_receipt_line_id UUID;

-- 3. Cabeceras y líneas de recepción.
CREATE TABLE purchase_receipts (
    id                  UUID         NOT NULL PRIMARY KEY,
    organization_id     UUID         NOT NULL,
    branch_id           UUID         NOT NULL,
    purchase_order_id   UUID         NOT NULL,
    idempotency_key     VARCHAR(128) NOT NULL CHECK (length(trim(idempotency_key)) > 0),
    payload_fingerprint VARCHAR(64)  NOT NULL CHECK (length(trim(payload_fingerprint)) > 0),
    received_by_user_id UUID         NOT NULL,
    received_at         TIMESTAMPTZ  NOT NULL,
    created_at          TIMESTAMPTZ  NOT NULL,
    CONSTRAINT purchase_receipts_idempotency_unique
        UNIQUE (organization_id, purchase_order_id, idempotency_key),
    CONSTRAINT purchase_receipts_id_org_branch_unique
        UNIQUE (id, organization_id, branch_id),
    CONSTRAINT purchase_receipts_id_org_order_unique
        UNIQUE (id, organization_id, purchase_order_id),
    CONSTRAINT purchase_receipts_order_fk
        FOREIGN KEY (purchase_order_id, organization_id, branch_id)
        REFERENCES purchase_orders (id, organization_id, branch_id),
    CONSTRAINT purchase_receipts_branch_fk
        FOREIGN KEY (branch_id, organization_id) REFERENCES branches (id, organization_id),
    CONSTRAINT purchase_receipts_received_by_fk
        FOREIGN KEY (received_by_user_id, organization_id) REFERENCES users (id, organization_id)
);

CREATE TABLE purchase_receipt_lines (
    id                     UUID          NOT NULL PRIMARY KEY,
    organization_id        UUID          NOT NULL,
    branch_id              UUID          NOT NULL,
    purchase_receipt_id    UUID          NOT NULL,
    purchase_order_id      UUID          NOT NULL,
    purchase_order_line_id UUID          NOT NULL,
    inventory_item_id      UUID          NOT NULL,
    quantity_received      NUMERIC(19,6) NOT NULL CHECK (quantity_received > 0),
    created_at             TIMESTAMPTZ   NOT NULL,
    CONSTRAINT purchase_receipt_lines_receipt_line_unique
        UNIQUE (purchase_receipt_id, purchase_order_line_id),
    CONSTRAINT purchase_receipt_lines_id_org_branch_item_unique
        UNIQUE (id, organization_id, branch_id, inventory_item_id),
    CONSTRAINT purchase_receipt_lines_receipt_fk
        FOREIGN KEY (purchase_receipt_id, organization_id, purchase_order_id)
        REFERENCES purchase_receipts (id, organization_id, purchase_order_id),
    CONSTRAINT purchase_receipt_lines_receipt_branch_fk
        FOREIGN KEY (purchase_receipt_id, organization_id, branch_id)
        REFERENCES purchase_receipts (id, organization_id, branch_id),
    CONSTRAINT purchase_receipt_lines_order_line_fk
        FOREIGN KEY (purchase_order_line_id, organization_id, purchase_order_id)
        REFERENCES purchase_order_lines (id, organization_id, purchase_order_id),
    CONSTRAINT purchase_receipt_lines_item_fk
        FOREIGN KEY (inventory_item_id, organization_id) REFERENCES inventory_items (id, organization_id)
);

-- 4. Ledger: nuevo tipo PURCHASE_RECEIPT, siempre positivo y ligado a una línea.
ALTER TABLE inventory_movements
    DROP CONSTRAINT inventory_movements_type_check;
ALTER TABLE inventory_movements
    ADD CONSTRAINT inventory_movements_type_check
        CHECK (type IN ('OPENING', 'ADJUSTMENT', 'SALE_CONSUMPTION', 'PURCHASE_RECEIPT'));

ALTER TABLE inventory_movements
    ADD CONSTRAINT inventory_movements_purchase_receipt_line_fk
        FOREIGN KEY (purchase_receipt_line_id, organization_id, branch_id, inventory_item_id)
        REFERENCES purchase_receipt_lines (id, organization_id, branch_id, inventory_item_id);

ALTER TABLE inventory_movements
    DROP CONSTRAINT inventory_movements_type_values;
ALTER TABLE inventory_movements
    ADD CONSTRAINT inventory_movements_type_values CHECK (
        (type = 'OPENING' AND quantity_delta >= 0
            AND sale_id IS NULL AND purchase_receipt_line_id IS NULL)
        OR (type = 'ADJUSTMENT' AND quantity_delta <> 0 AND reason IS NOT NULL
            AND length(trim(reason)) > 0 AND sale_id IS NULL AND purchase_receipt_line_id IS NULL)
        OR (type = 'SALE_CONSUMPTION' AND quantity_delta < 0
            AND sale_id IS NOT NULL AND purchase_receipt_line_id IS NULL)
        OR (type = 'PURCHASE_RECEIPT' AND quantity_delta > 0
            AND sale_id IS NULL AND purchase_receipt_line_id IS NOT NULL)
    );

-- 5. Inmutabilidad de la recepción: es un hecho histórico, nunca se modifica ni
-- se elimina por SQL. Mismo mecanismo (BEFORE UPDATE OR DELETE + ERRCODE) que el
-- ledger `inventory_movements` de V17. Los INSERT (cabecera y líneas) siguen
-- permitidos dentro de la misma transacción.
CREATE OR REPLACE FUNCTION purchase_receipts_reject_mutation() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'purchase_receipts is immutable: % is not allowed', TG_OP USING ERRCODE = 'check_violation';
END;
$$ LANGUAGE plpgsql;
CREATE TRIGGER purchase_receipts_immutable BEFORE UPDATE OR DELETE ON purchase_receipts
    FOR EACH ROW EXECUTE FUNCTION purchase_receipts_reject_mutation();

CREATE OR REPLACE FUNCTION purchase_receipt_lines_reject_mutation() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'purchase_receipt_lines is immutable: % is not allowed', TG_OP USING ERRCODE = 'check_violation';
END;
$$ LANGUAGE plpgsql;
CREATE TRIGGER purchase_receipt_lines_immutable BEFORE UPDATE OR DELETE ON purchase_receipt_lines
    FOR EACH ROW EXECUTE FUNCTION purchase_receipt_lines_reject_mutation();

-- 6. Un movimiento por línea de recepción: impide duplicar el hecho en el ledger.
CREATE UNIQUE INDEX inventory_movements_one_receipt_per_line
    ON inventory_movements (purchase_receipt_line_id) WHERE type = 'PURCHASE_RECEIPT';
CREATE INDEX idx_inventory_movements_purchase_receipt_line
    ON inventory_movements (purchase_receipt_line_id) WHERE purchase_receipt_line_id IS NOT NULL;

-- 7. Estados de la orden: PARTIALLY_RECEIVED y RECEIVED conservan la emisión.
-- V23 fijó status VARCHAR(16); PARTIALLY_RECEIVED (18) exige ampliar la columna.
ALTER TABLE purchase_orders
    ALTER COLUMN status TYPE VARCHAR(24);

ALTER TABLE purchase_orders
    DROP CONSTRAINT purchase_orders_status_check;
ALTER TABLE purchase_orders
    ADD CONSTRAINT purchase_orders_status_check
        CHECK (status IN ('DRAFT', 'ORDERED', 'PARTIALLY_RECEIVED', 'RECEIVED', 'CANCELLED'));

ALTER TABLE purchase_orders
    DROP CONSTRAINT purchase_orders_state_consistency;
ALTER TABLE purchase_orders
    ADD CONSTRAINT purchase_orders_state_consistency CHECK (
        (status = 'DRAFT'
            AND issued_at IS NULL AND issued_by_user_id IS NULL
            AND cancelled_at IS NULL AND cancelled_by_user_id IS NULL)
        OR (status IN ('ORDERED', 'PARTIALLY_RECEIVED', 'RECEIVED')
            AND issued_at IS NOT NULL AND issued_by_user_id IS NOT NULL
            AND cancelled_at IS NULL AND cancelled_by_user_id IS NULL)
        OR (status = 'CANCELLED'
            AND cancelled_at IS NOT NULL AND cancelled_by_user_id IS NOT NULL)
    );

-- 8. Índices de consulta por orden y por partida.
CREATE INDEX idx_purchase_receipts_order
    ON purchase_receipts (organization_id, purchase_order_id, received_at, id);
CREATE INDEX idx_purchase_receipt_lines_receipt
    ON purchase_receipt_lines (purchase_receipt_id);
CREATE INDEX idx_purchase_receipt_lines_order_line
    ON purchase_receipt_lines (organization_id, purchase_order_id, purchase_order_line_id);

-- 9. Permisos: lectura separada de escritura; no alteran grants existentes.
INSERT INTO permissions (code, description) VALUES
    ('purchasing.receipt.read',  'List and read branch purchase receipts'),
    ('purchasing.receipt.write', 'Post purchase receipts that increase branch stock');

COMMENT ON TABLE purchase_receipts IS
    'Recepción confirmada e inmutable de una orden de compra emitida. Identidad idempotente por (organization_id, purchase_order_id, idempotency_key). Inmutable: INSERT permitido, UPDATE/DELETE rechazados por trigger.';
COMMENT ON TABLE purchase_receipt_lines IS
    'Línea de recepción con la cantidad recibida en la unidad base congelada de la partida; origina un movimiento PURCHASE_RECEIPT. Pertenece a la misma organización y sucursal que su cabecera. Inmutable: INSERT permitido, UPDATE/DELETE rechazados por trigger.';
COMMENT ON COLUMN purchase_receipts.payload_fingerprint IS
    'Huella SHA-256 canónica de las líneas solicitadas; distingue un reintento idéntico de un conflicto de clave.';
COMMENT ON COLUMN inventory_movements.purchase_receipt_line_id IS
    'Línea de recepción que originó el movimiento PURCHASE_RECEIPT; NULL para OPENING/ADJUSTMENT/SALE_CONSUMPTION. La FK compuesta (línea, organización, sucursal, artículo) obliga a que el movimiento comparta artículo y alcance con su línea.';
COMMENT ON TABLE inventory_movements IS
    'Immutable append-only stock facts: OPENING and ADJUSTMENT (manual), SALE_CONSUMPTION (checkout), PURCHASE_RECEIPT (inbound receiving).';
