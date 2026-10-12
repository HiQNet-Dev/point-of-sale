package com.hiqnet.pos.sales.domain;

import java.util.Collection;
import java.util.List;
import java.util.UUID;
import org.springframework.jdbc.core.JdbcTemplate;
import org.springframework.stereotype.Repository;

@Repository
public class SaleItemInventoryComponentRepository {
    private final JdbcTemplate jdbcTemplate;

    public SaleItemInventoryComponentRepository(JdbcTemplate jdbcTemplate) {
        this.jdbcTemplate = jdbcTemplate;
    }

    public void replace(UUID organizationId, UUID saleItemId, List<SaleItemInventoryComponent> components) {
        jdbcTemplate.update("delete from sale_item_inventory_components where organization_id = ? and sale_item_id = ?",
                organizationId, saleItemId);
        if (components.isEmpty()) return;
        jdbcTemplate.batchUpdate("""
                insert into sale_item_inventory_components
                    (id, organization_id, sale_item_id, inventory_item_id,
                     inventory_item_code_snapshot, inventory_item_name_snapshot, base_unit_snapshot,
                     quantity_per_unit, quantity_total, created_at)
                values (?, ?, ?, ?, ?, ?, ?, ?, ?, now())
                """, components, components.size(), (statement, component) -> {
                    statement.setObject(1, UUID.randomUUID());
                    statement.setObject(2, organizationId);
                    statement.setObject(3, saleItemId);
                    statement.setObject(4, component.inventoryItemId());
                    statement.setString(5, component.inventoryItemCodeSnapshot());
                    statement.setString(6, component.inventoryItemNameSnapshot());
                    statement.setString(7, component.baseUnitSnapshot());
                    statement.setBigDecimal(8, component.quantityPerUnit());
                    statement.setBigDecimal(9, component.quantityTotal());
                });
    }

    /** Bounded bulk read for the future checkout consumer; this slice does not deduct stock. */
    public List<SaleItemInventoryComponent> findBySaleItemIds(Collection<UUID> saleItemIds) {
        if (saleItemIds.isEmpty()) return List.of();
        String placeholders = String.join(",", java.util.Collections.nCopies(saleItemIds.size(), "?"));
        return jdbcTemplate.query("""
                select sale_item_id, inventory_item_id, inventory_item_code_snapshot,
                       inventory_item_name_snapshot, base_unit_snapshot, quantity_per_unit, quantity_total
                from sale_item_inventory_components
                where sale_item_id in (
                """ + placeholders + ") order by sale_item_id, inventory_item_id",
                (result, row) -> new SaleItemInventoryComponent(
                        result.getObject("inventory_item_id", UUID.class),
                        result.getString("inventory_item_code_snapshot"),
                        result.getString("inventory_item_name_snapshot"),
                        result.getString("base_unit_snapshot"),
                        result.getBigDecimal("quantity_per_unit"),
                        result.getBigDecimal("quantity_total")),
                saleItemIds.toArray());
    }
}
