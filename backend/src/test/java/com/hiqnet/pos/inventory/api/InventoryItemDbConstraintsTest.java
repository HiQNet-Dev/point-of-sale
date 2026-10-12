package com.hiqnet.pos.inventory.api;

import static org.junit.jupiter.api.Assertions.assertThrows;

import com.hiqnet.pos.TestSecurityConfig;
import com.hiqnet.pos.TestcontainersConfiguration;
import com.hiqnet.pos.organization.application.OrganizationService;
import java.sql.Timestamp;
import java.time.Instant;
import java.util.UUID;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.boot.test.context.SpringBootTest;
import org.springframework.context.annotation.Import;
import org.springframework.dao.DataIntegrityViolationException;
import org.springframework.jdbc.core.JdbcTemplate;
import org.springframework.test.context.ActiveProfiles;

@ActiveProfiles("test")
@Import({TestcontainersConfiguration.class, TestSecurityConfig.class})
@SpringBootTest
class InventoryItemDbConstraintsTest {

    @Autowired
    private OrganizationService organizationService;

    @Autowired
    private JdbcTemplate jdbcTemplate;

    private static final String INSERT_ITEM = "INSERT INTO inventory_items "
            + "(id, organization_id, code, name, description, base_unit, fractional_unit_allowed, status, version, created_at, updated_at) "
            + "VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)";

    @Test
    void postgresEnforcesOrganizationForeignKeyAndItemChecks() {
        var organization = organizationService.create("Inventory DB Org", "inventory-db-org", "UTC", "MXN", "es-MX");
        Timestamp now = Timestamp.from(Instant.now());

        assertThrows(DataIntegrityViolationException.class, () -> jdbcTemplate.update(
                INSERT_ITEM, UUID.randomUUID(), UUID.randomUUID(), "valid-code", "Item", null,
                "GRAM", false, "ACTIVE", 0, now, now));
        assertThrows(DataIntegrityViolationException.class, () -> jdbcTemplate.update(
                INSERT_ITEM, UUID.randomUUID(), organization.getId(), "Invalid_Code", "Item", null,
                "GRAM", false, "ACTIVE", 0, now, now));
        assertThrows(DataIntegrityViolationException.class, () -> jdbcTemplate.update(
                INSERT_ITEM, UUID.randomUUID(), organization.getId(), "valid-code", "Item", null,
                "KILOGRAM", false, "ACTIVE", 0, now, now));
        assertThrows(DataIntegrityViolationException.class, () -> jdbcTemplate.update(
                INSERT_ITEM, UUID.randomUUID(), organization.getId(), "valid-code", "Item", null,
                "GRAM", false, "RETIRED", 0, now, now));
        assertThrows(DataIntegrityViolationException.class, () -> jdbcTemplate.update(
                INSERT_ITEM, UUID.randomUUID(), organization.getId(), "valid-code", "   ", null,
                "GRAM", false, "ACTIVE", 0, now, now));
        assertThrows(DataIntegrityViolationException.class, () -> jdbcTemplate.update(
                INSERT_ITEM, UUID.randomUUID(), organization.getId(), "valid-code", "Item", "   ",
                "GRAM", false, "ACTIVE", 0, now, now));

        jdbcTemplate.update(INSERT_ITEM, UUID.randomUUID(), organization.getId(), "valid-code", "Item", null,
                "GRAM", false, "ACTIVE", 0, now, now);
        assertThrows(DataIntegrityViolationException.class, () -> jdbcTemplate.update(
                INSERT_ITEM, UUID.randomUUID(), organization.getId(), "valid-code", "Duplicate", null,
                "UNIT", false, "ACTIVE", 0, now, now));
    }
}
