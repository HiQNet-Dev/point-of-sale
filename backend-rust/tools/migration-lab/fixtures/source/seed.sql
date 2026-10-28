-- Synthetic SOURCE data only: run after unmodified Flyway V1--V25.
-- No DDL, entity generation, target schema or real credentials. All clocks/IDs fixed.
-- UUID suffixes are identities, not codes regenerated during export.
DO $$
DECLARE
    t timestamptz := '2025-01-01T00:00:00Z';
    o uuid := '00000000-0000-0000-0000-000000000001';
    o2 uuid := '00000000-0000-0000-0000-000000000002';
    b uuid := '00000000-0000-0000-0000-000000000003';
    b2 uuid := '00000000-0000-0000-0000-000000000004';
    b3 uuid := '00000000-0000-0000-0000-000000000005';
    u uuid := '00000000-0000-0000-0000-000000000006';
    u2 uuid := '00000000-0000-0000-0000-000000000007';
    u3 uuid := '00000000-0000-0000-0000-000000000008';
    r uuid := '00000000-0000-0000-0000-000000000009';
    p uuid := '00000000-0000-0000-0000-000000000010';
    c uuid := '00000000-0000-0000-0000-000000000011';
    g uuid := '00000000-0000-0000-0000-000000000012';
    opt uuid := '00000000-0000-0000-0000-000000000013';
    reg uuid := '00000000-0000-0000-0000-000000000014';
    shift uuid := '00000000-0000-0000-0000-000000000015';
    sale uuid := '00000000-0000-0000-0000-000000000016';
    line uuid := '00000000-0000-0000-0000-000000000017';
    item uuid := '00000000-0000-0000-0000-000000000018';
    recipe uuid := '00000000-0000-0000-0000-000000000019';
    config uuid := '00000000-0000-0000-0000-000000000020';
    supplier uuid := '00000000-0000-0000-0000-000000000021';
    ordered uuid := '00000000-0000-0000-0000-000000000022';
    received uuid := '00000000-0000-0000-0000-000000000023';
    ordered_line uuid := '00000000-0000-0000-0000-000000000024';
    received_line uuid := '00000000-0000-0000-0000-000000000025';
    receipt uuid := '00000000-0000-0000-0000-000000000026';
    receipt_line uuid := '00000000-0000-0000-0000-000000000027';
    membership uuid := '00000000-0000-0000-0000-000000000028';
    r2 uuid := '00000000-0000-0000-0000-000000000029';
    -- Fixed synthetic encoded fixture, not a deployment password. Never log/export in summaries.
    encoded text := '{bcrypt}$2a$10$N9qo8uLOickgx2ZMRZoMyeIjZAgcfl7p92ldGxad68LJZdL17lhWy';
BEGIN
    INSERT INTO app_metadata (key,value,created_at,updated_at) VALUES ('source-lab','synthetic-reference',t,t);
    INSERT INTO organizations (id,name,code,timezone,currency,locale,created_at,updated_at) VALUES
        (o,'Source café A','source-a','UTC','MXN','es-MX',t,t),
        (o2,'Source B','source-b','America/Mexico_City','USD','en-US',t,t);
    INSERT INTO branches (id,organization_id,name,code,timezone,created_at,updated_at) VALUES
        (b,o,'A central','central',NULL,t,t), (b2,o,'A norte','north','UTC',t,t),
        (b3,o2,'B central','central',NULL,t,t);
    INSERT INTO users (id,organization_id,email,password_hash,status,locked_until,last_login_at,created_at,updated_at) VALUES
        (u,o,'admin@source.invalid',encoded,'ACTIVE',NULL,t,t,t),
        (u2,o,'cashier@source.invalid',encoded,'INACTIVE',NULL,NULL,t,t),
        (u3,o2,'admin-b@source.invalid',encoded,'ACTIVE',NULL,NULL,t,t);
    INSERT INTO refresh_sessions (id,user_id,token_hash,family_id,family_expires_at,created_at,expires_at) VALUES
        ('00000000-0000-0000-0000-000000000030',u,repeat('a',64),
         '00000000-0000-0000-0000-000000000031',t+interval '30 days',t,t+interval '1 day');
    INSERT INTO roles (id,organization_id,code,name,created_at,updated_at) VALUES
        (r,o,'administrator','Source administrator',t,t), (r2,o2,'reader','Source B reader',t,t);
    -- Reuse the real migration permission catalog (59 codes); do not invent grants in export.
    INSERT INTO role_permissions (role_id,permission_code) SELECT r,code FROM permissions;
    INSERT INTO role_permissions (role_id,permission_code) VALUES (r2,'organization.read');
    INSERT INTO user_organization_roles (organization_id,user_id,role_id,created_at) VALUES (o,u,r,t),(o2,u3,r2,t);
    INSERT INTO branch_memberships (id,organization_id,branch_id,user_id,created_at,updated_at)
        VALUES (membership,o,b,u2,t,t);
    INSERT INTO branch_membership_roles (organization_id,membership_id,role_id,created_at) VALUES (o,membership,r,t);
    INSERT INTO audit_events (id,organization_id,branch_id,actor_type,actor_user_id,action,entity_type,entity_id,metadata,occurred_at)
        VALUES ('00000000-0000-0000-0000-000000000032',o,b,'USER',u,'source.fixture','ORGANIZATION',o,
                '{"synthetic":true,"exact":"0.000001","null":null}'::jsonb,t);
    UPDATE installation_bootstrap SET completed_at=t,organization_id=o,administrator_user_id=u,administrator_role_id=r WHERE id=1;
    INSERT INTO categories (id,organization_id,code,name,created_at,updated_at) VALUES (c,o,'drinks','Bebidas',t,t);
    INSERT INTO products (id,organization_id,category_id,sku,barcode,name,description,unit_price,created_at,updated_at)
        VALUES (p,o,c,'COFFEE','000123456789','Café actual',NULL,900719925474.1234,t,t),
               ('00000000-0000-0000-0000-000000000050',o,NULL,'FREE',NULL,'Free sample',NULL,0.0000,t,t);
    INSERT INTO modifier_groups (id,organization_id,code,name,min_selections,max_selections,created_at,updated_at)
        VALUES (g,o,'milk','Leche',0,1,t,t);
    INSERT INTO modifier_options (id,organization_id,modifier_group_id,code,name,price_delta,created_at,updated_at)
        VALUES (opt,o,g,'less-milk','Menos leche',-0.0001,t,t);
    INSERT INTO product_modifier_groups (organization_id,product_id,modifier_group_id,created_at) VALUES (o,p,g,t);
    INSERT INTO cash_registers (id,organization_id,branch_id,code,name,created_at,updated_at) VALUES (reg,o,b,'pos','Source POS',t,t);
    INSERT INTO cash_shifts (id,organization_id,branch_id,cash_register_id,opened_by_user_id,closed_by_user_id,status,
                            opening_amount,closing_declared_amount,opened_at,closed_at,created_at,updated_at)
        VALUES (shift,o,b,reg,u,u,'CLOSED',0.0000,0.0000,t,t+interval '3 hours',t,t+interval '3 hours');
    INSERT INTO sales (id,organization_id,branch_id,cash_register_id,cash_shift_id,created_by_user_id,status,total_amount,
                       created_at,updated_at,completed_at,completed_by_user_id)
        VALUES (sale,o,b,reg,shift,u,'COMPLETED',12.3456,t,t+interval '2 hours',t+interval '2 hours',u);
    -- Historical quote predates the current product price/name; never regenerate snapshots from catalog.
    INSERT INTO sale_items (id,organization_id,sale_id,product_id,product_sku_snapshot,product_name_snapshot,base_unit_price,
                            modifier_unit_delta,effective_unit_price,quantity,line_total,status,position,created_at,updated_at)
        VALUES (line,o,sale,p,'COFFEE','Café histórico',12.3457,-0.0001,12.3456,1,12.3456,'ACTIVE',0,t,t);
    INSERT INTO sale_item_modifiers (id,organization_id,sale_item_id,modifier_group_id,modifier_option_id,group_code_snapshot,
                                    group_name_snapshot,option_code_snapshot,option_name_snapshot,price_delta_snapshot,display_order,created_at)
        VALUES ('00000000-0000-0000-0000-000000000033',o,line,g,opt,'milk','Leche histórica','less-milk','Menos leche histórico',-0.0001,0,t);
    INSERT INTO payments (id,organization_id,branch_id,cash_register_id,cash_shift_id,sale_id,position,method,amount,
                          tendered_amount,change_amount,created_by_user_id,created_at)
        VALUES ('00000000-0000-0000-0000-000000000034',o,b,reg,shift,sale,1,'CARD',12.3456,NULL,0.0000,u,t+interval '2 hours');
    INSERT INTO inventory_items (id,organization_id,code,name,base_unit,fractional_unit_allowed,unit_locked,created_at,updated_at)
        VALUES (item,o,'milk','Leche','MILLILITER',true,true,t,t);
    INSERT INTO branch_inventory (id,organization_id,branch_id,inventory_item_id,quantity_on_hand,created_at,updated_at)
        VALUES ('00000000-0000-0000-0000-000000000035',o,b,item,11.000000,t,t+interval '2 hours');
    INSERT INTO product_inventory_recipes (id,organization_id,product_id,status,created_at,updated_at) VALUES (recipe,o,p,'ACTIVE',t,t);
    INSERT INTO product_inventory_recipe_components (id,organization_id,recipe_id,inventory_item_id,quantity_per_product)
        VALUES ('00000000-0000-0000-0000-000000000036',o,recipe,item,1.500001);
    INSERT INTO modifier_option_inventory_configs (id,organization_id,modifier_option_id,created_at,updated_at) VALUES (config,o,opt,t,t);
    INSERT INTO modifier_option_inventory_effects (id,organization_id,config_id,inventory_item_id,quantity_delta)
        VALUES ('00000000-0000-0000-0000-000000000037',o,config,item,-0.500000);
    INSERT INTO sale_item_inventory_components (id,organization_id,sale_item_id,inventory_item_id,inventory_item_code_snapshot,
                                              inventory_item_name_snapshot,base_unit_snapshot,quantity_per_unit,quantity_total,created_at)
        VALUES ('00000000-0000-0000-0000-000000000038',o,line,item,'milk','Leche histórica','MILLILITER',1.000001,1.000001,t);
    INSERT INTO suppliers (id,organization_id,code,name,email,created_at,updated_at) VALUES (supplier,o,'supplier','Source supplier','supplier@source.invalid',t,t);
    INSERT INTO purchase_orders (id,organization_id,branch_id,supplier_id,code,status,currency,supplier_code_snapshot,supplier_name_snapshot,
                                 created_by_user_id,issued_by_user_id,created_at,updated_at,issued_at) VALUES
        (ordered,o,b,supplier,'ordered-null-cost','ORDERED','MXN','supplier','Source supplier histórico',u,u,t,t,t),
        (received,o,b,supplier,'received','RECEIVED','MXN','supplier','Source supplier histórico',u,u,t,t+interval '1 hour',t);
    INSERT INTO purchase_order_lines (id,organization_id,purchase_order_id,inventory_item_id,line_number,item_code_snapshot,item_name_snapshot,
                                      unit_snapshot,quantity,unit_cost,created_at,updated_at) VALUES
        (ordered_line,o,ordered,item,1,'milk','Leche histórica','MILLILITER',1.000001,NULL,t,t),
        (received_line,o,received,item,1,'milk','Leche histórica','MILLILITER',2.000001,0.0000,t,t);
    INSERT INTO purchase_receipts (id,organization_id,branch_id,purchase_order_id,idempotency_key,payload_fingerprint,received_by_user_id,received_at,created_at)
        VALUES (receipt,o,b,received,'synthetic-receipt-1',repeat('b',64),u,t+interval '1 hour',t+interval '1 hour');
    INSERT INTO purchase_receipt_lines (id,organization_id,branch_id,purchase_receipt_id,purchase_order_id,purchase_order_line_id,inventory_item_id,quantity_received,created_at)
        VALUES (receipt_line,o,b,receipt,received,received_line,item,2.000001,t+interval '1 hour');
    INSERT INTO inventory_movements (id,organization_id,branch_id,inventory_item_id,type,quantity_delta,resulting_quantity,
                                     actor_user_id,occurred_at,sale_id,purchase_receipt_line_id) VALUES
        ('00000000-0000-0000-0000-000000000039',o,b,item,'OPENING',10.000000,10.000000,u,t,NULL,NULL),
        ('00000000-0000-0000-0000-000000000040',o,b,item,'PURCHASE_RECEIPT',2.000001,12.000001,u,t+interval '1 hour',NULL,receipt_line),
        ('00000000-0000-0000-0000-000000000041',o,b,item,'SALE_CONSUMPTION',-1.000001,11.000000,u,t+interval '2 hours',sale,NULL);
END
$$;
