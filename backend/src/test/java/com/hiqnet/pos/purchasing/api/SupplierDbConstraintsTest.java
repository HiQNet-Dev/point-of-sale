package com.hiqnet.pos.purchasing.api;

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
class SupplierDbConstraintsTest {

    @Autowired
    private OrganizationService organizationService;

    @Autowired
    private JdbcTemplate jdbcTemplate;

    private static final String INSERT_SUPPLIER = "INSERT INTO suppliers "
            + "(id, organization_id, code, name, legal_name, tax_id, contact_name, email, phone, address, status, version, created_at, updated_at) "
            + "VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)";

    @Test
    void postgresEnforcesSupplierOwnershipCodeAndStatusConstraints() {
        var organization = organizationService.create("Supplier DB Organization", "supplier-db-org", "UTC", "MXN", "es-MX");
        Timestamp now = Timestamp.from(Instant.now());

        assertThrows(DataIntegrityViolationException.class, () -> jdbcTemplate.update(
                INSERT_SUPPLIER, UUID.randomUUID(), UUID.randomUUID(), "supplier", "Supplier", null, null, null, null,
                null, null, "ACTIVE", 0, now, now));
        assertThrows(DataIntegrityViolationException.class, () -> jdbcTemplate.update(
                INSERT_SUPPLIER, UUID.randomUUID(), organization.getId(), "Supplier_Invalid", "Supplier", null, null,
                null, null, null, null, "ACTIVE", 0, now, now));
        assertThrows(DataIntegrityViolationException.class, () -> jdbcTemplate.update(
                INSERT_SUPPLIER, UUID.randomUUID(), organization.getId(), "supplier", "   ", null, null, null, null,
                null, null, "ACTIVE", 0, now, now));
        assertThrows(DataIntegrityViolationException.class, () -> jdbcTemplate.update(
                INSERT_SUPPLIER, UUID.randomUUID(), organization.getId(), "supplier", "Supplier", "   ", null, null,
                null, null, null, "ACTIVE", 0, now, now));
        assertThrows(DataIntegrityViolationException.class, () -> jdbcTemplate.update(
                INSERT_SUPPLIER, UUID.randomUUID(), organization.getId(), "supplier", "Supplier", null, null, null,
                null, null, null, "RETIRED", 0, now, now));

        jdbcTemplate.update(
                INSERT_SUPPLIER, UUID.randomUUID(), organization.getId(), "supplier", "Supplier", null, null, null,
                null, null, null, "ACTIVE", 0, now, now);
        assertThrows(DataIntegrityViolationException.class, () -> jdbcTemplate.update(
                INSERT_SUPPLIER, UUID.randomUUID(), organization.getId(), "supplier", "Duplicate", null, null, null,
                null, null, null, "ACTIVE", 0, now, now));
    }
}
