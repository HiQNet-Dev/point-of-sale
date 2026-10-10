package com.hiqnet.pos.catalog;

import static org.junit.jupiter.api.Assertions.assertDoesNotThrow;
import static org.junit.jupiter.api.Assertions.assertThrows;

import com.hiqnet.pos.catalog.application.CategoryService;
import com.hiqnet.pos.organization.application.OrganizationService;
import com.hiqnet.pos.organization.domain.Organization;
import com.hiqnet.pos.TestSecurityConfig;
import com.hiqnet.pos.TestcontainersConfiguration;
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
 * The database is the final authority on catalog integrity: the composite FK
 * keeps a product's category in the SAME organization, the partial unique index
 * keeps barcodes unique only when present, and the CHECK keeps the SKU in its
 * canonical uppercase form. None of these rely on Java validation.
 */
@ActiveProfiles("test")
@Import({TestcontainersConfiguration.class, TestSecurityConfig.class})
@SpringBootTest
class CatalogDbConstraintsTest {

    @Autowired
    private OrganizationService organizationService;

    @Autowired
    private CategoryService categoryService;

    @Autowired
    private JdbcTemplate jdbcTemplate;

    private static final String INSERT_PRODUCT =
            "INSERT INTO products (id, organization_id, category_id, sku, barcode, name, description, unit_price, "
                    + "status, version, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)";

    private static Object[] productRow(UUID orgId, UUID categoryId, String sku, String barcode) {
        Timestamp now = Timestamp.from(Instant.now());
        return new Object[] {
            UUID.randomUUID(), orgId, categoryId, sku, barcode, "Producto", null,
            new BigDecimal("5.0000"), "ACTIVE", 0, now, now
        };
    }

    @Test
    void productCannotReferenceCategoryOfAnotherOrganization() {
        Organization orgA = organizationService.create("Org A", "catalog-fk-a", "UTC", "MXN", "es-MX");
        Organization orgB = organizationService.create("Org B", "catalog-fk-b", "UTC", "MXN", "es-MX");
        UUID categoryA = categoryService.create(orgA.getId(), "bebidas", "Bebidas").getId();

        // Product belongs to B but references a category of A: composite FK rejects it.
        assertThrows(
                DataIntegrityViolationException.class,
                () -> jdbcTemplate.update(
                        INSERT_PRODUCT, productRow(orgB.getId(), categoryA, "AGUA", null)));
    }

    @Test
    void barcodeIsUniquePerOrganizationOnlyWhenPresent() {
        Organization org = organizationService.create("Org", "catalog-barcode", "UTC", "MXN", "es-MX");

        jdbcTemplate.update(INSERT_PRODUCT, productRow(org.getId(), null, "AGUA", "111"));

        // Second product with the same barcode in the same organization: rejected.
        assertThrows(
                DataIntegrityViolationException.class,
                () -> jdbcTemplate.update(INSERT_PRODUCT, productRow(org.getId(), null, "JUGO", "111")));

        // Two products without a barcode are allowed (partial index).
        assertDoesNotThrow(() -> jdbcTemplate.update(INSERT_PRODUCT, productRow(org.getId(), null, "TE", null)));
        assertDoesNotThrow(() -> jdbcTemplate.update(INSERT_PRODUCT, productRow(org.getId(), null, "CAFE", null)));
    }

    @Test
    void skuFormatCheckRejectsNonCanonicalSku() {
        Organization org = organizationService.create("Org", "catalog-sku", "UTC", "MXN", "es-MX");
        assertThrows(
                DataIntegrityViolationException.class,
                () -> jdbcTemplate.update(INSERT_PRODUCT, productRow(org.getId(), null, "agua", null)));
    }
}
