package com.hiqnet.pos.inventory.recipe.domain;

import com.hiqnet.pos.inventory.domain.InventoryBaseUnit;
import com.hiqnet.pos.inventory.domain.InventoryItemStatus;
import java.util.List;
import java.util.UUID;
import org.springframework.jdbc.core.JdbcTemplate;
import org.springframework.stereotype.Repository;

@Repository
public class InventoryRecipeComponentRepository {
    private final JdbcTemplate jdbcTemplate;
    public InventoryRecipeComponentRepository(JdbcTemplate jdbcTemplate) { this.jdbcTemplate = jdbcTemplate; }

    public List<RecipeComponentData> findByOrganizationIdAndRecipeId(UUID organizationId, UUID recipeId) {
        return jdbcTemplate.query("""
                select c.inventory_item_id, i.code, i.name, i.base_unit, i.fractional_unit_allowed,
                       i.status, c.quantity_per_product
                from product_inventory_recipe_components c
                join inventory_items i on i.id = c.inventory_item_id and i.organization_id = c.organization_id
                where c.organization_id = ? and c.recipe_id = ?
                order by c.inventory_item_id asc
                """, (rs, row) -> new RecipeComponentData(
                    rs.getObject("inventory_item_id", UUID.class), rs.getString("code"), rs.getString("name"),
                    InventoryBaseUnit.valueOf(rs.getString("base_unit")), rs.getBoolean("fractional_unit_allowed"),
                    InventoryItemStatus.valueOf(rs.getString("status")), rs.getBigDecimal("quantity_per_product")),
                organizationId, recipeId);
    }

    public void replace(UUID organizationId, UUID recipeId, List<RecipeComponentData> components) {
        jdbcTemplate.update("delete from product_inventory_recipe_components where organization_id = ? and recipe_id = ?",
                organizationId, recipeId);
        if (components.isEmpty()) return;
        jdbcTemplate.batchUpdate("""
                insert into product_inventory_recipe_components
                    (id, organization_id, recipe_id, inventory_item_id, quantity_per_product)
                values (?, ?, ?, ?, ?)
                """, components, components.size(), (ps, component) -> {
                    ps.setObject(1, UUID.randomUUID()); ps.setObject(2, organizationId); ps.setObject(3, recipeId);
                    ps.setObject(4, component.inventoryItemId()); ps.setBigDecimal(5, component.quantityPerProduct());
                });
    }
}
