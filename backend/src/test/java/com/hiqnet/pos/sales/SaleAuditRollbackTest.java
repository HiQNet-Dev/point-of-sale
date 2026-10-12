package com.hiqnet.pos.sales;

import static org.assertj.core.api.Assertions.assertThat;
import static org.junit.jupiter.api.Assertions.assertThrows;

import com.hiqnet.pos.AbstractRbacIntegrationTest;
import com.hiqnet.pos.branch.domain.Branch;
import com.hiqnet.pos.branch.domain.BranchRepository;
import com.hiqnet.pos.cashregister.domain.CashRegister;
import com.hiqnet.pos.cashregister.domain.CashRegisterRepository;
import com.hiqnet.pos.cashshift.domain.CashShift;
import com.hiqnet.pos.cashshift.domain.CashShiftRepository;
import com.hiqnet.pos.catalog.domain.Product;
import com.hiqnet.pos.catalog.domain.ProductRepository;
import com.hiqnet.pos.inventory.domain.InventoryBaseUnit;
import com.hiqnet.pos.inventory.domain.InventoryItem;
import com.hiqnet.pos.inventory.domain.InventoryItemRepository;
import com.hiqnet.pos.organization.domain.Organization;
import com.hiqnet.pos.sales.domain.Sale;
import com.hiqnet.pos.sales.domain.SaleRepository;
import com.hiqnet.pos.sales.application.SaleService;
import com.hiqnet.pos.shared.audit.AuditException;
import com.hiqnet.pos.shared.audit.AuditPort;
import com.hiqnet.pos.shared.audit.AuditRecord;
import java.math.BigDecimal;
import java.time.Instant;
import java.util.UUID;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.boot.test.context.TestConfiguration;
import org.springframework.context.annotation.Bean;
import org.springframework.context.annotation.Import;
import org.springframework.context.annotation.Primary;
import org.springframework.jdbc.core.JdbcTemplate;

/** Sales commands and their audit records commit atomically. */
@Import(SaleAuditRollbackTest.FailingAuditConfig.class)
class SaleAuditRollbackTest extends AbstractRbacIntegrationTest {

    @Autowired
    private BranchRepository branchRepository;

    @Autowired
    private CashRegisterRepository cashRegisterRepository;

    @Autowired
    private CashShiftRepository cashShiftRepository;

    @Autowired
    private SaleService saleService;

    @Autowired
    private SaleRepository saleRepository;

    @Autowired
    private ProductRepository productRepository;

    @Autowired
    private JdbcTemplate jdbcTemplate;

    @Autowired
    private InventoryItemRepository inventoryItemRepository;

    @Test
    void auditFailureRollsBackSaleCreation() {
        RbacFixture fixture = fixture();
        Organization organization = organizationRepository.findById(fixture.orgId()).orElseThrow();
        Branch branch = branchRepository.saveAndFlush(Branch.create(organization, "Branch", "audit-sales", null));
        CashRegister register = cashRegisterRepository.saveAndFlush(CashRegister.create(organization, branch, "register", "Register"));
        CashShift shift = cashShiftRepository.saveAndFlush(
                CashShift.open(fixture.orgId(), branch.getId(), register.getId(), fixture.userId(), BigDecimal.ZERO, Instant.now()));

        assertThrows(AuditException.class, () -> saleService.create(
                fixture.orgId(), branch.getId(), register.getId(), shift.getId(), fixture.userId()));

        Long sales = jdbcTemplate.queryForObject(
                "select count(*) from sales where cash_shift_id = ?", Long.class, shift.getId());
        assertThat(sales).isZero();
    }

    @Test
    void auditFailureRollsBackItemAdditionWithoutChangingTheAggregate() {
        RbacFixture fixture = fixture();
        OperationalScope scope = operationalScope(fixture, "audit-add-item");
        Product product = productRepository.saveAndFlush(
                Product.create(scope.organization(), "AUDIT-ITEM", "Audit item", new BigDecimal("3.2500")));
        InventoryItem inventoryItem = inventoryItemRepository.saveAndFlush(InventoryItem.create(
                scope.organization(), "audit-flour", "Audit flour", null, InventoryBaseUnit.GRAM, false));
        UUID recipeId = UUID.randomUUID();
        jdbcTemplate.update("insert into product_inventory_recipes (id,organization_id,product_id,status,version,created_at,updated_at) values (?,?,?,'INACTIVE',0,now(),now())",
                recipeId, fixture.orgId(), product.getId());
        jdbcTemplate.update("insert into product_inventory_recipe_components (id,organization_id,recipe_id,inventory_item_id,quantity_per_product) values (?,?,?,?,?)",
                UUID.randomUUID(), fixture.orgId(), recipeId, inventoryItem.getId(), new BigDecimal("7.5"));
        jdbcTemplate.update("update product_inventory_recipes set status='ACTIVE' where id=?", recipeId);
        Sale sale = saleRepository.saveAndFlush(Sale.open(
                fixture.orgId(), scope.branch().getId(), scope.register().getId(), scope.shift().getId(), fixture.userId()));
        long auditsBefore = auditCount(fixture.orgId());

        assertThrows(AuditException.class, () -> saleService.addItem(
                fixture.orgId(), scope.branch().getId(), scope.register().getId(), scope.shift().getId(), sale.getId(),
                0, product.getId(), 2, java.util.List.of(), fixture.userId()));

        assertThat(jdbcTemplate.queryForObject("select version from sales where id = ?", Long.class, sale.getId())).isZero();
        assertThat(jdbcTemplate.queryForObject("select total_amount from sales where id = ?", BigDecimal.class, sale.getId()))
                .isEqualByComparingTo("0.0000");
        assertThat(jdbcTemplate.queryForObject("select count(*) from sale_items where sale_id = ?", Long.class, sale.getId())).isZero();
        assertThat(jdbcTemplate.queryForObject(
                "select count(*) from sale_item_modifiers sim join sale_items si on si.id = sim.sale_item_id where si.sale_id = ?",
                Long.class, sale.getId())).isZero();
        assertThat(jdbcTemplate.queryForObject("select count(*) from sale_item_inventory_components where sale_item_id in (select id from sale_items where sale_id = ?)", Long.class, sale.getId())).isZero();
        assertThat(auditCount(fixture.orgId())).isEqualTo(auditsBefore);
    }

    @Test
    void auditFailureRollsBackCancellationWithoutChangingTheAggregate() {
        RbacFixture fixture = fixture();
        OperationalScope scope = operationalScope(fixture, "audit-cancel");
        Sale sale = saleRepository.saveAndFlush(Sale.open(
                fixture.orgId(), scope.branch().getId(), scope.register().getId(), scope.shift().getId(), fixture.userId()));
        long auditsBefore = auditCount(fixture.orgId());

        assertThrows(AuditException.class, () -> saleService.cancel(
                fixture.orgId(), scope.branch().getId(), scope.register().getId(), scope.shift().getId(), sale.getId(), 0, fixture.userId()));

        assertThat(jdbcTemplate.queryForObject("select status from sales where id = ?", String.class, sale.getId())).isEqualTo("OPEN");
        assertThat(jdbcTemplate.queryForObject("select version from sales where id = ?", Long.class, sale.getId())).isZero();
        assertThat(jdbcTemplate.queryForObject("select cancelled_at from sales where id = ?", java.sql.Timestamp.class, sale.getId())).isNull();
        assertThat(jdbcTemplate.queryForObject("select cancelled_by_user_id from sales where id = ?", UUID.class, sale.getId())).isNull();
        assertThat(auditCount(fixture.orgId())).isEqualTo(auditsBefore);
    }

    private OperationalScope operationalScope(RbacFixture fixture, String branchCode) {
        Organization organization = organizationRepository.findById(fixture.orgId()).orElseThrow();
        Branch branch = branchRepository.saveAndFlush(Branch.create(organization, "Branch", branchCode, null));
        CashRegister register = cashRegisterRepository.saveAndFlush(CashRegister.create(organization, branch, "register", "Register"));
        CashShift shift = cashShiftRepository.saveAndFlush(
                CashShift.open(fixture.orgId(), branch.getId(), register.getId(), fixture.userId(), BigDecimal.ZERO, Instant.now()));
        return new OperationalScope(organization, branch, register, shift);
    }

    private long auditCount(UUID organizationId) {
        Long count = jdbcTemplate.queryForObject(
                "select count(*) from audit_events where organization_id = ?", Long.class, organizationId);
        return count == null ? 0 : count;
    }

    private record OperationalScope(Organization organization, Branch branch, CashRegister register, CashShift shift) {}

    @TestConfiguration
    static class FailingAuditConfig {
        @Bean
        @Primary
        AuditPort failingAuditPort() {
            return (AuditRecord record) -> {
                throw new AuditException("audit unavailable (forced test failure)");
            };
        }
    }
}
