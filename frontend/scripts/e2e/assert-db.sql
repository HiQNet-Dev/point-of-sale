\set ON_ERROR_STOP on

SELECT set_config(
  'e2e.organization_id',
  (SELECT id::text FROM organizations WHERE code = :'org_code'),
  false
);

DO $$
DECLARE
  e2e_org_id UUID := current_setting('e2e.organization_id')::UUID;
BEGIN

  IF e2e_org_id IS NULL THEN
    RAISE EXCEPTION 'E2E organization was not provisioned';
  END IF;

  IF NOT EXISTS (
    SELECT 1 FROM sales
    WHERE organization_id = e2e_org_id AND status = 'COMPLETED'
      AND completed_at IS NOT NULL AND completed_by_user_id IS NOT NULL
  ) THEN
    RAISE EXCEPTION 'No completed Sale persisted for E2E organization';
  END IF;

  IF NOT EXISTS (
    SELECT 1 FROM payments
    WHERE organization_id = e2e_org_id AND method = 'CASH'
      AND amount = 10.1000 AND tendered_amount = 20.0000 AND change_amount = 9.9000
  ) THEN
    RAISE EXCEPTION 'Expected exact CASH payment/tender/change was not persisted';
  END IF;

  IF NOT EXISTS (
    SELECT 1 FROM payments
    WHERE organization_id = e2e_org_id AND method = 'CARD' AND amount = 0.2000
      AND tendered_amount IS NULL AND change_amount = 0
  ) THEN
    RAISE EXCEPTION 'Expected CARD payment without tender/change was not persisted';
  END IF;

  IF NOT EXISTS (
    SELECT 1 FROM sale_items
    WHERE organization_id = e2e_org_id AND product_name_snapshot = 'E2E Simple Product'
      AND product_sku_snapshot = 'E2E-SIMPLE'
      AND base_unit_price = 10.1000 AND effective_unit_price = 10.1000 AND line_total = 10.1000
  ) THEN
    RAISE EXCEPTION 'Expected exact immutable SaleItem snapshot was not persisted';
  END IF;

  IF NOT EXISTS (
    SELECT 1 FROM sales s
    WHERE s.organization_id = e2e_org_id AND s.status = 'COMPLETED' AND s.total_amount = 0
      AND NOT EXISTS (SELECT 1 FROM payments p WHERE p.sale_id = s.id)
  ) THEN
    RAISE EXCEPTION 'Expected zero-total Sale without a fake Payment was not persisted';
  END IF;

  IF NOT EXISTS (
    SELECT 1 FROM payments card
    JOIN payments cash ON cash.sale_id = card.sale_id
    WHERE card.organization_id = e2e_org_id AND card.method = 'CARD' AND card.amount = 4.1000
      AND cash.method = 'CASH' AND cash.amount = 6.0000
      AND cash.tendered_amount = 10.0000 AND cash.change_amount = 4.0000
  ) THEN
    RAISE EXCEPTION 'Expected split-tender payment records were not persisted';
  END IF;

  IF NOT EXISTS (
    SELECT sale_id FROM payments
    WHERE organization_id = e2e_org_id AND method = 'CARD'
    GROUP BY sale_id HAVING count(*) = 2 AND sum(amount) = 10.1000
  ) THEN
    RAISE EXCEPTION 'Expected multiple CARD payment records were not persisted';
  END IF;

  IF NOT EXISTS (
    SELECT 1 FROM sale_item_modifiers
    WHERE organization_id = e2e_org_id AND option_name_snapshot = 'Required option'
      AND price_delta_snapshot = 0.2000
  ) THEN
    RAISE EXCEPTION 'Expected modifier snapshot was not persisted';
  END IF;

  IF NOT EXISTS (SELECT 1 FROM audit_events WHERE organization_id = e2e_org_id AND action = 'cash-shift.opened') THEN
    RAISE EXCEPTION 'Shift-open audit event is missing';
  END IF;
  IF NOT EXISTS (SELECT 1 FROM audit_events WHERE organization_id = e2e_org_id AND action = 'sale.created') THEN
    RAISE EXCEPTION 'Sale-create audit event is missing';
  END IF;
  IF NOT EXISTS (SELECT 1 FROM audit_events WHERE organization_id = e2e_org_id AND action = 'sale.item.added') THEN
    RAISE EXCEPTION 'Sale-item audit event is missing';
  END IF;
  IF NOT EXISTS (SELECT 1 FROM audit_events WHERE organization_id = e2e_org_id AND action = 'sale.completed') THEN
    RAISE EXCEPTION 'Sale-completed audit event is missing';
  END IF;
  IF NOT EXISTS (SELECT 1 FROM audit_events WHERE organization_id = e2e_org_id AND action = 'cash-shift.closed') THEN
    RAISE EXCEPTION 'Shift-close audit event is missing';
  END IF;
END
$$;

SELECT 'E2E database assertions passed' AS result;
