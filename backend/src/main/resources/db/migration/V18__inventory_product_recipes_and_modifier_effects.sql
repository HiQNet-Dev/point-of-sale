-- Organization-owned Inventory configuration. Product and ModifierOption remain Catalog-owned;
-- these aggregates reference them without adding Inventory dependencies to Catalog.
-- Recipe component order is not operational; API reads use a stable InventoryItem id order.


CREATE TABLE product_inventory_recipes (
    id UUID NOT NULL PRIMARY KEY,
    organization_id UUID NOT NULL,
    product_id UUID NOT NULL,
    status VARCHAR(16) NOT NULL CHECK (status IN ('ACTIVE', 'INACTIVE')),
    version BIGINT NOT NULL DEFAULT 0 CHECK (version >= 0),
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    CONSTRAINT product_inventory_recipes_product_unique UNIQUE (organization_id, product_id),
    CONSTRAINT product_inventory_recipes_id_organization_unique UNIQUE (id, organization_id),
    CONSTRAINT product_inventory_recipes_product_fk
        FOREIGN KEY (product_id, organization_id) REFERENCES products (id, organization_id)
);

CREATE TABLE product_inventory_recipe_components (
    id UUID NOT NULL PRIMARY KEY,
    organization_id UUID NOT NULL,
    recipe_id UUID NOT NULL,
    inventory_item_id UUID NOT NULL,
    quantity_per_product NUMERIC(19,6) NOT NULL CHECK (quantity_per_product > 0),
    CONSTRAINT product_inventory_recipe_components_item_unique UNIQUE (recipe_id, inventory_item_id),
    CONSTRAINT product_inventory_recipe_components_recipe_fk
        FOREIGN KEY (recipe_id, organization_id)
        REFERENCES product_inventory_recipes (id, organization_id) ON DELETE CASCADE,
    CONSTRAINT product_inventory_recipe_components_item_fk
        FOREIGN KEY (inventory_item_id, organization_id)
        REFERENCES inventory_items (id, organization_id)
);
CREATE INDEX idx_product_inventory_recipe_components_recipe
    ON product_inventory_recipe_components (organization_id, recipe_id);

CREATE FUNCTION assert_active_recipe_has_components(target_recipe_id UUID) RETURNS void AS $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM product_inventory_recipes r
        WHERE r.id = target_recipe_id AND r.status = 'ACTIVE'
    ) AND NOT EXISTS (
        SELECT 1 FROM product_inventory_recipe_components c WHERE c.recipe_id = target_recipe_id
    ) THEN
        RAISE EXCEPTION 'ACTIVE product inventory recipe must contain at least one component'
            USING ERRCODE = 'check_violation';
    END IF;
END;
$$ LANGUAGE plpgsql;

CREATE FUNCTION product_inventory_recipe_requires_components() RETURNS trigger AS $$
DECLARE
    target_recipe_id UUID;
BEGIN
    IF TG_TABLE_NAME = 'product_inventory_recipes' THEN
        target_recipe_id := NEW.id;
        PERFORM assert_active_recipe_has_components(target_recipe_id);
    ELSIF TG_OP = 'DELETE' THEN
        PERFORM assert_active_recipe_has_components(OLD.recipe_id);
    ELSE
        PERFORM assert_active_recipe_has_components(OLD.recipe_id);
        IF NEW.recipe_id IS DISTINCT FROM OLD.recipe_id THEN
            PERFORM assert_active_recipe_has_components(NEW.recipe_id);
        END IF;
    END IF;
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

CREATE CONSTRAINT TRIGGER product_inventory_recipe_nonempty_on_parent
    AFTER INSERT OR UPDATE ON product_inventory_recipes
    DEFERRABLE INITIALLY DEFERRED
    FOR EACH ROW EXECUTE FUNCTION product_inventory_recipe_requires_components();
CREATE CONSTRAINT TRIGGER product_inventory_recipe_nonempty_on_component
    AFTER INSERT OR UPDATE OR DELETE ON product_inventory_recipe_components
    DEFERRABLE INITIALLY DEFERRED
    FOR EACH ROW EXECUTE FUNCTION product_inventory_recipe_requires_components();

CREATE TABLE modifier_option_inventory_configs (
    id UUID NOT NULL PRIMARY KEY,
    organization_id UUID NOT NULL,
    modifier_option_id UUID NOT NULL,
    version BIGINT NOT NULL DEFAULT 0 CHECK (version >= 0),
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    CONSTRAINT modifier_option_inventory_configs_option_unique UNIQUE (organization_id, modifier_option_id),
    CONSTRAINT modifier_option_inventory_configs_id_organization_unique UNIQUE (id, organization_id),
    CONSTRAINT modifier_option_inventory_configs_option_fk
        FOREIGN KEY (modifier_option_id, organization_id)
        REFERENCES modifier_options (id, organization_id)
);

CREATE TABLE modifier_option_inventory_effects (
    id UUID NOT NULL PRIMARY KEY,
    organization_id UUID NOT NULL,
    config_id UUID NOT NULL,
    inventory_item_id UUID NOT NULL,
    quantity_delta NUMERIC(19,6) NOT NULL CHECK (quantity_delta <> 0),
    CONSTRAINT modifier_option_inventory_effects_item_unique UNIQUE (config_id, inventory_item_id),
    CONSTRAINT modifier_option_inventory_effects_config_fk
        FOREIGN KEY (config_id, organization_id)
        REFERENCES modifier_option_inventory_configs (id, organization_id) ON DELETE CASCADE,
    CONSTRAINT modifier_option_inventory_effects_item_fk
        FOREIGN KEY (inventory_item_id, organization_id)
        REFERENCES inventory_items (id, organization_id)
);
CREATE INDEX idx_modifier_option_inventory_effects_config
    ON modifier_option_inventory_effects (organization_id, config_id);
CREATE INDEX idx_modifier_option_inventory_configs_option
    ON modifier_option_inventory_configs (organization_id, modifier_option_id);

INSERT INTO permissions (code, description) VALUES
    ('inventory.recipe.read', 'Read product Inventory recipes and modifier Inventory effects'),
    ('inventory.recipe.write', 'Create and replace product Inventory recipes and modifier effects');

COMMENT ON TABLE product_inventory_recipes IS
    'Organization-owned recipe configuration for a Catalog Product. No recipe or an INACTIVE recipe means untracked for future consumption resolution.';
COMMENT ON TABLE product_inventory_recipe_components IS
    'Positive exact quantity per saleable product unit; component ordering has no business meaning.';
COMMENT ON TABLE modifier_option_inventory_configs IS
    'Inventory-owned, independently versioned effects configuration for a Catalog ModifierOption. An empty effect set is valid.';
COMMENT ON TABLE modifier_option_inventory_effects IS
    'Signed non-zero exact quantity delta applied by a selected modifier option. Consumption resolution aggregates effects and rejects negative/discrete-invalid totals.';
