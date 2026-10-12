package com.hiqnet.pos.inventory.recipe.domain;

import java.util.UUID;
import org.springframework.jdbc.core.JdbcTemplate;
import org.springframework.stereotype.Repository;

@Repository
public class ProductInventoryRecipeStore {
    private final JdbcTemplate jdbcTemplate;
    public ProductInventoryRecipeStore(JdbcTemplate jdbcTemplate) { this.jdbcTemplate = jdbcTemplate; }

    public int ensure(UUID id, UUID organizationId, UUID productId) {
        return jdbcTemplate.update("""
                insert into product_inventory_recipes
                    (id, organization_id, product_id, status, version, created_at, updated_at)
                values (?, ?, ?, 'ACTIVE', 0, now(), now())
                on conflict (organization_id, product_id) do nothing
                """, id, organizationId, productId);
    }
}
