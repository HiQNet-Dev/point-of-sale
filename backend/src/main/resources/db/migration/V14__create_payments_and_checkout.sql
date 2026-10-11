-- Atomic checkout and immutable payment facts. V1-V13 are immutable.

ALTER TABLE sales ADD COLUMN completed_at TIMESTAMPTZ NULL;
ALTER TABLE sales ADD COLUMN completed_by_user_id UUID NULL;
ALTER TABLE sales ADD CONSTRAINT sales_completed_by_fk
    FOREIGN KEY (completed_by_user_id, organization_id) REFERENCES users (id, organization_id);
ALTER TABLE sales DROP CONSTRAINT sales_state_consistency;
-- V13 declared the original OPEN/CANCELLED enum check inline, so PostgreSQL
-- named it sales_status_check. Replace both lifecycle barriers in V14.
ALTER TABLE sales DROP CONSTRAINT sales_status_check;
ALTER TABLE sales ADD CONSTRAINT sales_status_check CHECK (status IN ('OPEN', 'CANCELLED', 'COMPLETED'));
ALTER TABLE sales ADD CONSTRAINT sales_state_consistency CHECK (
    (status = 'OPEN' AND cancelled_at IS NULL AND cancelled_by_user_id IS NULL AND completed_at IS NULL AND completed_by_user_id IS NULL)
    OR (status = 'CANCELLED' AND cancelled_at IS NOT NULL AND cancelled_by_user_id IS NOT NULL AND completed_at IS NULL AND completed_by_user_id IS NULL)
    OR (status = 'COMPLETED' AND cancelled_at IS NULL AND cancelled_by_user_id IS NULL AND completed_at IS NOT NULL AND completed_by_user_id IS NOT NULL)
);
ALTER TABLE sales ADD CONSTRAINT sales_id_scope_unique UNIQUE (id, organization_id, branch_id, cash_register_id, cash_shift_id);

CREATE TABLE payments (
    id UUID NOT NULL PRIMARY KEY,
    organization_id UUID NOT NULL,
    branch_id UUID NOT NULL,
    cash_register_id UUID NOT NULL,
    cash_shift_id UUID NOT NULL,
    sale_id UUID NOT NULL,
    position INTEGER NOT NULL CHECK (position >= 1),
    method VARCHAR(16) NOT NULL CHECK (method IN ('CASH', 'CARD')),
    amount NUMERIC(19,4) NOT NULL CHECK (amount > 0),
    tendered_amount NUMERIC(19,4) NULL,
    change_amount NUMERIC(19,4) NOT NULL CHECK (change_amount >= 0),
    created_by_user_id UUID NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    CONSTRAINT payments_sale_scope_fk FOREIGN KEY (sale_id, organization_id, branch_id, cash_register_id, cash_shift_id)
        REFERENCES sales (id, organization_id, branch_id, cash_register_id, cash_shift_id),
    CONSTRAINT payments_created_by_fk FOREIGN KEY (created_by_user_id, organization_id) REFERENCES users (id, organization_id),
    CONSTRAINT payments_sale_position_unique UNIQUE (sale_id, position),
    CONSTRAINT payments_method_consistency CHECK (
        (method = 'CASH' AND tendered_amount IS NOT NULL AND tendered_amount >= amount AND change_amount = tendered_amount - amount)
        OR (method = 'CARD' AND tendered_amount IS NULL AND change_amount = 0)
    )
);
CREATE INDEX idx_payments_sale_position ON payments (sale_id, position);

INSERT INTO permissions (code, description) VALUES
    ('sale.checkout', 'Complete an OPEN sale through atomic checkout'),
    ('payment.read', 'Read historical payments of a completed sale');

COMMENT ON TABLE payments IS 'Immutable payment facts created only by atomic Sale checkout. No independent payment write lifecycle exists.';
