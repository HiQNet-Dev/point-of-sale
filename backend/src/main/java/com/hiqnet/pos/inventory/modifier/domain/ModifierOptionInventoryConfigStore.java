package com.hiqnet.pos.inventory.modifier.domain;

import java.util.UUID;
import org.springframework.jdbc.core.JdbcTemplate;
import org.springframework.stereotype.Repository;

@Repository
public class ModifierOptionInventoryConfigStore {
    private final JdbcTemplate jdbcTemplate;
    public ModifierOptionInventoryConfigStore(JdbcTemplate jdbcTemplate) { this.jdbcTemplate = jdbcTemplate; }

    public int ensure(UUID id, UUID organizationId, UUID modifierOptionId) {
        return jdbcTemplate.update("""
                insert into modifier_option_inventory_configs
                    (id, organization_id, modifier_option_id, version, created_at, updated_at)
                values (?, ?, ?, 0, now(), now())
                on conflict (organization_id, modifier_option_id) do nothing
                """, id, organizationId, modifierOptionId);
    }
}
