package com.hiqnet.pos.inventory.modifier.domain;

import com.hiqnet.pos.inventory.domain.InventoryBaseUnit;
import com.hiqnet.pos.inventory.domain.InventoryItemStatus;
import java.util.Collection;
import java.util.List;
import java.util.UUID;
import org.springframework.jdbc.core.JdbcTemplate;
import org.springframework.stereotype.Repository;

@Repository
public class ModifierOptionInventoryEffectRepository {
    private final JdbcTemplate jdbcTemplate;
    public ModifierOptionInventoryEffectRepository(JdbcTemplate jdbcTemplate) { this.jdbcTemplate = jdbcTemplate; }

    public List<ModifierInventoryEffectData> findByOrganizationIdAndModifierOptionId(UUID organizationId, UUID optionId) {
        return query("where c.organization_id = ? and c.modifier_option_id = ? order by e.inventory_item_id asc", organizationId, optionId);
    }

    public List<ModifierInventoryEffectData> findByOrganizationIdAndModifierOptionIds(UUID organizationId, Collection<UUID> optionIds) {
        if (optionIds.isEmpty()) return List.of();
        String placeholders = String.join(",", java.util.Collections.nCopies(optionIds.size(), "?"));
        Object[] args = new Object[optionIds.size() + 1]; args[0] = organizationId;
        int index = 1; for (UUID optionId : optionIds) args[index++] = optionId;
        return query("where c.organization_id = ? and c.modifier_option_id in (" + placeholders + ") order by e.inventory_item_id asc", args);
    }

    private List<ModifierInventoryEffectData> query(String clause, Object... args) {
        return jdbcTemplate.query("""
                select c.modifier_option_id, e.inventory_item_id, i.code, i.name, i.base_unit,
                       i.fractional_unit_allowed, i.status, e.quantity_delta
                from modifier_option_inventory_effects e
                join modifier_option_inventory_configs c on c.id = e.config_id and c.organization_id = e.organization_id
                join inventory_items i on i.id = e.inventory_item_id and i.organization_id = e.organization_id
                """ + clause, (rs, row) -> new ModifierInventoryEffectData(
                    rs.getObject("modifier_option_id", UUID.class), rs.getObject("inventory_item_id", UUID.class),
                    rs.getString("code"), rs.getString("name"), InventoryBaseUnit.valueOf(rs.getString("base_unit")),
                    rs.getBoolean("fractional_unit_allowed"), InventoryItemStatus.valueOf(rs.getString("status")),
                    rs.getBigDecimal("quantity_delta")), args);
    }

    public void replace(UUID organizationId, UUID configId, List<ModifierInventoryEffectData> effects) {
        jdbcTemplate.update("delete from modifier_option_inventory_effects where organization_id = ? and config_id = ?",
                organizationId, configId);
        if (effects.isEmpty()) return;
        jdbcTemplate.batchUpdate("""
                insert into modifier_option_inventory_effects
                    (id, organization_id, config_id, inventory_item_id, quantity_delta)
                values (?, ?, ?, ?, ?)
                """, effects, effects.size(), (ps, effect) -> {
                    ps.setObject(1, UUID.randomUUID()); ps.setObject(2, organizationId); ps.setObject(3, configId);
                    ps.setObject(4, effect.inventoryItemId()); ps.setBigDecimal(5, effect.quantityDelta());
                });
    }
}
