package com.hiqnet.pos.catalog;

import static org.junit.jupiter.api.Assertions.assertDoesNotThrow;
import static org.junit.jupiter.api.Assertions.assertThrows;

import com.hiqnet.pos.TestSecurityConfig;
import com.hiqnet.pos.TestcontainersConfiguration;
import com.hiqnet.pos.organization.application.OrganizationService;
import com.hiqnet.pos.organization.domain.Organization;
import java.math.BigDecimal;
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

/**
 * The database is the final authority on modifier integrity: composite FKs keep
 * every option and product assignment in the SAME organization as its parent,
 * unique indexes keep codes unique per scope, CHECK constraints enforce the
 * selection bounds / slug format, and {@code price_delta} is a signed NUMERIC.
 */
@ActiveProfiles("test")
@Import({TestcontainersConfiguration.class, TestSecurityConfig.class})
@SpringBootTest
class ModifierDbConstraintsTest {

    @Autowired
    private OrganizationService organizationService;

    @Autowired
    private JdbcTemplate jdbcTemplate;

    private static final String INSERT_GROUP =
            "INSERT INTO modifier_groups (id, organization_id, code, name, min_selections, max_selections, "
                    + "status, version, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)";

    private static final String INSERT_OPTION =
            "INSERT INTO modifier_options (id, organization_id, modifier_group_id, code, name, price_delta, "
                    + "display_order, status, version, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)";

    private static final String INSERT_PRODUCT =
            "INSERT INTO products (id, organization_id, category_id, sku, barcode, name, description, unit_price, "
                    + "status, version, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)";

    private static final String INSERT_PMU =
            "INSERT INTO product_modifier_groups (organization_id, product_id, modifier_group_id, display_order, "
                    + "created_at) VALUES (?, ?, ?, ?, ?)";

    private static Object[] groupRow(UUID orgId, String code, int min, int max) {
        Timestamp now = Timestamp.from(Instant.now());
        return new Object[] {UUID.randomUUID(), orgId, code, "Group", min, max, "ACTIVE", 0, now, now};
    }

    private static Object[] optionRow(UUID orgId, UUID groupId, String code, BigDecimal priceDelta) {
        Timestamp now = Timestamp.from(Instant.now());
        return new Object[] {
            UUID.randomUUID(), orgId, groupId, code, "Option", priceDelta, 0, "ACTIVE", 0, now, now
        };
    }

    private static Object[] productRow(UUID orgId, String sku) {
        Timestamp now = Timestamp.from(Instant.now());
        return new Object[] {
            UUID.randomUUID(), orgId, null, sku, null, "Producto", null,
            new BigDecimal("5.0000"), "ACTIVE", 0, now, now
        };
    }

    private UUID insertGroup(UUID orgId, String code, int min, int max) {
        Object[] row = groupRow(orgId, code, min, max);
        jdbcTemplate.update(INSERT_GROUP, row);
        return (UUID) row[0];
    }

    private UUID insertProduct(UUID orgId, String sku) {
        Object[] row = productRow(orgId, sku);
        jdbcTemplate.update(INSERT_PRODUCT, row);
        return (UUID) row[0];
    }

    @Test
    void optionCannotReferenceGroupOfAnotherOrganization() {
        Organization orgA = organizationService.create("Org A", "mod-fk-a", "UTC", "MXN", "es-MX");
        Organization orgB = organizationService.create("Org B", "mod-fk-b", "UTC", "MXN", "es-MX");
        UUID groupA = insertGroup(orgA.getId(), "milk", 0, 1);

        assertThrows(
                DataIntegrityViolationException.class,
                () -> jdbcTemplate.update(
                        INSERT_OPTION, optionRow(orgB.getId(), groupA, "oat", BigDecimal.ZERO)));
    }

    @Test
    void groupCodeIsUniquePerOrganization() {
        Organization orgA = organizationService.create("Org A", "mod-code-a", "UTC", "MXN", "es-MX");
        Organization orgB = organizationService.create("Org B", "mod-code-b", "UTC", "MXN", "es-MX");
        insertGroup(orgA.getId(), "milk", 0, 1);

        assertThrows(
                DataIntegrityViolationException.class,
                () -> jdbcTemplate.update(INSERT_GROUP, groupRow(orgA.getId(), "milk", 0, 1)));

        assertDoesNotThrow(() -> jdbcTemplate.update(INSERT_GROUP, groupRow(orgB.getId(), "milk", 0, 1)));
    }

    @Test
    void groupSelectionBoundsAreEnforcedByCheck() {
        Organization org = organizationService.create("Org", "mod-bounds", "UTC", "MXN", "es-MX");

        assertThrows(
                DataIntegrityViolationException.class,
                () -> jdbcTemplate.update(INSERT_GROUP, groupRow(org.getId(), "zero-max", 0, 0)));
        assertThrows(
                DataIntegrityViolationException.class,
                () -> jdbcTemplate.update(INSERT_GROUP, groupRow(org.getId(), "min-gt-max", 2, 1)));
    }

    @Test
    void groupCodeFormatCheckRejectsNonCanonicalCode() {
        Organization org = organizationService.create("Org", "mod-slug", "UTC", "MXN", "es-MX");
        assertThrows(
                DataIntegrityViolationException.class,
                () -> jdbcTemplate.update(INSERT_GROUP, groupRow(org.getId(), "Milk", 0, 1)));
    }

    @Test
    void optionPriceDeltaAcceptsZeroAndNegative() {
        Organization org = organizationService.create("Org", "mod-delta", "UTC", "MXN", "es-MX");
        UUID groupId = insertGroup(org.getId(), "milk", 0, 1);

        assertDoesNotThrow(() -> jdbcTemplate.update(
                INSERT_OPTION, optionRow(org.getId(), groupId, "no-charge", new BigDecimal("0.0000"))));
        assertDoesNotThrow(() -> jdbcTemplate.update(
                INSERT_OPTION, optionRow(org.getId(), groupId, "discount", new BigDecimal("-1.5000"))));
    }

    @Test
    void optionCodeIsUniqueWithinGroup() {
        Organization org = organizationService.create("Org", "mod-opt-code", "UTC", "MXN", "es-MX");
        UUID groupId = insertGroup(org.getId(), "milk", 0, 1);
        UUID otherGroupId = insertGroup(org.getId(), "size", 0, 1);
        jdbcTemplate.update(INSERT_OPTION, optionRow(org.getId(), groupId, "oat", BigDecimal.ZERO));

        assertThrows(
                DataIntegrityViolationException.class,
                () -> jdbcTemplate.update(
                        INSERT_OPTION, optionRow(org.getId(), groupId, "oat", BigDecimal.ZERO)));
        assertDoesNotThrow(() -> jdbcTemplate.update(
                INSERT_OPTION, optionRow(org.getId(), otherGroupId, "oat", BigDecimal.ZERO)));
    }

    @Test
    void productModifierGroupCannotCrossOrganization() {
        Organization orgA = organizationService.create("Org A", "mod-pmu-a", "UTC", "MXN", "es-MX");
        Organization orgB = organizationService.create("Org B", "mod-pmu-b", "UTC", "MXN", "es-MX");
        UUID productA = insertProduct(orgA.getId(), "AGUA");
        UUID groupB = insertGroup(orgB.getId(), "milk", 0, 1);

        assertThrows(
                DataIntegrityViolationException.class,
                () -> jdbcTemplate.update(
                        INSERT_PMU, orgA.getId(), productA, groupB, 0, Timestamp.from(Instant.now())));
    }

    @Test
    void productModifierGroupIsUniquePerProductAndGroup() {
        Organization org = organizationService.create("Org", "mod-pmu-uniq", "UTC", "MXN", "es-MX");
        UUID product = insertProduct(org.getId(), "AGUA");
        UUID group = insertGroup(org.getId(), "milk", 0, 1);
        Timestamp now = Timestamp.from(Instant.now());
        jdbcTemplate.update(INSERT_PMU, org.getId(), product, group, 0, now);

        assertThrows(
                DataIntegrityViolationException.class,
                () -> jdbcTemplate.update(INSERT_PMU, org.getId(), product, group, 1, now));
    }
}
