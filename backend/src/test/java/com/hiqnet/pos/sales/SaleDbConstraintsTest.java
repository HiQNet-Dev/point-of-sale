package com.hiqnet.pos.sales;

import static org.junit.jupiter.api.Assertions.assertDoesNotThrow;
import static org.junit.jupiter.api.Assertions.assertThrows;

import com.hiqnet.pos.AbstractRbacIntegrationTest;
import com.hiqnet.pos.branch.domain.Branch;
import com.hiqnet.pos.branch.domain.BranchRepository;
import com.hiqnet.pos.cashregister.domain.CashRegister;
import com.hiqnet.pos.cashregister.domain.CashRegisterRepository;
import com.hiqnet.pos.cashshift.domain.CashShift;
import com.hiqnet.pos.cashshift.domain.CashShiftRepository;
import com.hiqnet.pos.organization.application.OrganizationService;
import com.hiqnet.pos.organization.domain.Organization;
import java.math.BigDecimal;
import java.sql.Timestamp;
import java.time.Instant;
import java.util.UUID;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.dao.DataIntegrityViolationException;
import org.springframework.jdbc.core.JdbcTemplate;

/** The V13 composite foreign keys and lifecycle checks are the last integrity barrier. */
class SaleDbConstraintsTest extends AbstractRbacIntegrationTest {

    private static final String INSERT_SALE = "INSERT INTO sales (id, organization_id, branch_id, cash_register_id, cash_shift_id, "
            + "created_by_user_id, cancelled_by_user_id, status, total_amount, version, created_at, updated_at, cancelled_at) "
            + "VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)";

    private static final String INSERT_ITEM = "INSERT INTO sale_items (id, organization_id, sale_id, product_id, product_sku_snapshot, "
            + "product_name_snapshot, base_unit_price, modifier_unit_delta, effective_unit_price, quantity, line_total, status, position, "
            + "created_at, updated_at, removed_at, removed_by_user_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)";

    @Autowired
    private OrganizationService organizationService;

    @Autowired
    private BranchRepository branchRepository;

    @Autowired
    private CashRegisterRepository cashRegisterRepository;

    @Autowired
    private CashShiftRepository cashShiftRepository;

    @Autowired
    private JdbcTemplate jdbcTemplate;

    @Test
    void saleCannotReferenceShiftFromAnotherRegister() {
        Scope scope = scope("sale-db-register");
        CashRegister otherRegister = cashRegisterRepository.saveAndFlush(
                CashRegister.create(scope.organization(), scope.branch(), "other-register", "Other register"));

        assertThrows(DataIntegrityViolationException.class, () -> insertSale(
                scope.organization().getId(), scope.branch().getId(), otherRegister.getId(), scope.shift().getId(),
                scope.userId(), "OPEN", BigDecimal.ZERO, null, null));
    }

    @Test
    void saleLifecycleCheckRejectsPartialCancellationAndNegativeTotal() {
        Scope scope = scope("sale-db-lifecycle");
        Instant now = Instant.now();

        assertThrows(DataIntegrityViolationException.class, () -> insertSale(
                scope.organization().getId(), scope.branch().getId(), scope.register().getId(), scope.shift().getId(),
                scope.userId(), "OPEN", BigDecimal.ZERO, scope.userId(), now));
        assertThrows(DataIntegrityViolationException.class, () -> insertSale(
                scope.organization().getId(), scope.branch().getId(), scope.register().getId(), scope.shift().getId(),
                scope.userId(), "OPEN", new BigDecimal("-0.0001"), null, null));
    }

    @Test
    void saleItemCannotReferenceProductFromAnotherOrganization() {
        Scope scope = scope("sale-db-item-org-a");
        Organization otherOrganization = organizationService.create("Other organization", "sale-db-item-org-b", "UTC", "MXN", "es-MX");
        UUID saleId = insertSale(scope.organization().getId(), scope.branch().getId(), scope.register().getId(), scope.shift().getId(),
                scope.userId(), "OPEN", BigDecimal.ZERO, null, null);
        UUID otherProductId = insertProduct(otherOrganization.getId(), "OTHER-PRODUCT");

        assertThrows(DataIntegrityViolationException.class, () -> insertItem(
                scope.organization().getId(), saleId, otherProductId, "ACTIVE", null, null));
    }

    @Test
    void saleItemModifierCannotReferenceAnOptionFromAnotherOrganization() {
        Scope scope = scope("sale-db-modifier-org-a");
        UUID saleId = insertSale(scope.organization().getId(), scope.branch().getId(), scope.register().getId(), scope.shift().getId(),
                scope.userId(), "OPEN", BigDecimal.ZERO, null, null);
        UUID productId = insertProduct(scope.organization().getId(), "LOCAL-MODIFIER-PRODUCT");
        UUID itemId = insertItem(scope.organization().getId(), saleId, productId, "ACTIVE", null, null);
        Organization otherOrganization = organizationService.create("Other modifier organization", "sale-db-modifier-org-b", "UTC", "MXN", "es-MX");
        UUID groupId = insertModifierGroup(otherOrganization.getId(), "other-group");
        UUID optionId = insertModifierOption(otherOrganization.getId(), groupId, "other-option");
        Timestamp now = Timestamp.from(Instant.now());

        assertThrows(DataIntegrityViolationException.class, () -> jdbcTemplate.update(
                "INSERT INTO sale_item_modifiers (id, organization_id, sale_item_id, modifier_group_id, modifier_option_id, "
                        + "group_code_snapshot, group_name_snapshot, option_code_snapshot, option_name_snapshot, price_delta_snapshot, display_order, created_at) "
                        + "VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                UUID.randomUUID(), scope.organization().getId(), itemId, groupId, optionId,
                "other-group", "Other group", "other-option", "Other option", BigDecimal.ZERO, 0, now));
    }

    @Test
    void saleItemLifecycleCheckRejectsPartialRemoval() {
        Scope scope = scope("sale-db-item-lifecycle");
        UUID saleId = insertSale(scope.organization().getId(), scope.branch().getId(), scope.register().getId(), scope.shift().getId(),
                scope.userId(), "OPEN", BigDecimal.ZERO, null, null);
        UUID productId = insertProduct(scope.organization().getId(), "LOCAL-PRODUCT");

        assertThrows(DataIntegrityViolationException.class, () -> insertItem(
                scope.organization().getId(), saleId, productId, "ACTIVE", scope.userId(), Instant.now()));
        assertDoesNotThrow(() -> insertItem(scope.organization().getId(), saleId, productId, "REMOVED", scope.userId(), Instant.now()));
    }

    @Test
    void saleItemQuantityCheckRejectsZeroAndNegativeValues() {
        Scope scope = scope("sale-db-item-quantity");
        UUID saleId = insertSale(scope.organization().getId(), scope.branch().getId(), scope.register().getId(), scope.shift().getId(),
                scope.userId(), "OPEN", BigDecimal.ZERO, null, null);
        UUID productId = insertProduct(scope.organization().getId(), "QUANTITY-PRODUCT");

        assertThrows(DataIntegrityViolationException.class, () -> insertItem(
                scope.organization().getId(), saleId, productId, "ACTIVE", null, null, 0));
        assertThrows(DataIntegrityViolationException.class, () -> insertItem(
                scope.organization().getId(), saleId, productId, "ACTIVE", null, null, -1));
    }

    private Scope scope(String organizationCode) {
        RbacFixture fixture = fixture();
        Organization organization = organizationRepository.findById(fixture.orgId()).orElseThrow();
        Branch branch = branchRepository.saveAndFlush(Branch.create(organization, "Branch", organizationCode, null));
        CashRegister register = cashRegisterRepository.saveAndFlush(
                CashRegister.create(organization, branch, "register", "Register"));
        CashShift shift = cashShiftRepository.saveAndFlush(CashShift.open(
                organization.getId(), branch.getId(), register.getId(), fixture.userId(), BigDecimal.ZERO, Instant.now()));
        return new Scope(organization, branch, register, shift, fixture.userId());
    }

    private UUID insertSale(UUID organizationId, UUID branchId, UUID registerId, UUID shiftId, UUID userId,
            String status, BigDecimal total, UUID cancelledBy, Instant cancelledAt) {
        UUID id = UUID.randomUUID();
        Timestamp now = Timestamp.from(Instant.now());
        jdbcTemplate.update(INSERT_SALE, id, organizationId, branchId, registerId, shiftId, userId, cancelledBy, status,
                total, 0L, now, now, cancelledAt == null ? null : Timestamp.from(cancelledAt));
        return id;
    }

    private UUID insertProduct(UUID organizationId, String sku) {
        UUID id = UUID.randomUUID();
        Timestamp now = Timestamp.from(Instant.now());
        jdbcTemplate.update("INSERT INTO products (id, organization_id, category_id, sku, barcode, name, description, unit_price, "
                        + "status, version, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                id, organizationId, null, sku, null, "Product", null, new BigDecimal("1.0000"), "ACTIVE", 0L, now, now);
        return id;
    }

    private UUID insertItem(UUID organizationId, UUID saleId, UUID productId, String status, UUID removedBy, Instant removedAt) {
        return insertItem(organizationId, saleId, productId, status, removedBy, removedAt, 1);
    }

    private UUID insertItem(UUID organizationId, UUID saleId, UUID productId, String status, UUID removedBy, Instant removedAt, int quantity) {
        UUID id = UUID.randomUUID();
        Timestamp now = Timestamp.from(Instant.now());
        jdbcTemplate.update(INSERT_ITEM, id, organizationId, saleId, productId, "SKU", "Product",
                new BigDecimal("1.0000"), BigDecimal.ZERO, new BigDecimal("1.0000"), quantity, new BigDecimal("1.0000"),
                status, 0, now, now, removedAt == null ? null : Timestamp.from(removedAt), removedBy);
        return id;
    }

    private UUID insertModifierGroup(UUID organizationId, String code) {
        UUID id = UUID.randomUUID();
        Timestamp now = Timestamp.from(Instant.now());
        jdbcTemplate.update("INSERT INTO modifier_groups (id, organization_id, code, name, min_selections, max_selections, status, version, created_at, updated_at) "
                        + "VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                id, organizationId, code, "Other group", 0, 1, "ACTIVE", 0L, now, now);
        return id;
    }

    private UUID insertModifierOption(UUID organizationId, UUID groupId, String code) {
        UUID id = UUID.randomUUID();
        Timestamp now = Timestamp.from(Instant.now());
        jdbcTemplate.update("INSERT INTO modifier_options (id, organization_id, modifier_group_id, code, name, price_delta, display_order, status, version, created_at, updated_at) "
                        + "VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                id, organizationId, groupId, code, "Other option", BigDecimal.ZERO, 0, "ACTIVE", 0L, now, now);
        return id;
    }

    private record Scope(Organization organization, Branch branch, CashRegister register, CashShift shift, UUID userId) {}
}
