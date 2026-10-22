package com.hiqnet.pos.purchasing.api;

import static org.assertj.core.api.Assertions.assertThat;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import com.hiqnet.pos.audit.AbstractAuditIntegrationTest;
import com.hiqnet.pos.audit.infrastructure.AuditRecorder;
import com.hiqnet.pos.branch.application.BranchService;
import com.hiqnet.pos.branch.domain.Branch;
import com.hiqnet.pos.inventory.application.InventoryItemService;
import com.hiqnet.pos.inventory.domain.InventoryBaseUnit;
import com.hiqnet.pos.inventory.domain.InventoryItem;
import com.hiqnet.pos.purchasing.application.SupplierService;
import com.hiqnet.pos.purchasing.domain.Supplier;
import com.hiqnet.pos.shared.audit.AuditException;
import com.hiqnet.pos.shared.audit.AuditPort;
import java.util.UUID;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.boot.test.context.TestConfiguration;
import org.springframework.context.annotation.Bean;
import org.springframework.context.annotation.Import;
import org.springframework.context.annotation.Primary;
import org.springframework.http.MediaType;

/**
 * La auditoría de las transiciones es transaccional: si el evento de emisión o
 * cancelación falla, el cambio de estado no se persiste.
 */
@Import(PurchaseOrderTransitionAuditRollbackTest.FailingTransitionAuditConfig.class)
class PurchaseOrderTransitionAuditRollbackTest extends AbstractAuditIntegrationTest {

    @Autowired
    private BranchService branchService;

    @Autowired
    private SupplierService supplierService;

    @Autowired
    private InventoryItemService inventoryItemService;

    private record Fixture(RbacFixture rbac, String token, UUID branchId, UUID supplierId, UUID itemId) {
    }

    private Fixture prepare() throws Exception {
        RbacFixture rbac = fixture();
        String token = adminToken(rbac);
        Branch branch = branchService.create(rbac.orgId(), "Centro", "sucursal-centro", null);
        Supplier supplier = supplierService.create(
                rbac.orgId(), "proveedor-centro", "Proveedor Centro", null, null, null, null, null, null);
        InventoryItem item = inventoryItemService.create(
                rbac.orgId(), "harina-trigo", "Harina de trigo", null, InventoryBaseUnit.GRAM, true);
        return new Fixture(rbac, token, branch.getId(), supplier.getId(), item.getId());
    }

    private String ordersUrl(Fixture fixture) {
        return "/api/v1/organizations/" + fixture.rbac().orgId() + "/branches/" + fixture.branchId()
                + "/purchase-orders";
    }

    private String createOrder(Fixture fixture, String code) throws Exception {
        String body = mockMvc.perform(post(ordersUrl(fixture))
                        .header("Authorization", bearer(fixture.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"" + code + "\",\"supplierId\":\"" + fixture.supplierId() + "\"}"))
                .andExpect(status().isCreated())
                .andReturn()
                .getResponse()
                .getContentAsString();
        return objectMapper.readTree(body).get("purchaseOrder").get("id").asText();
    }

    @Test
    void auditFailureRollsBackIssue() throws Exception {
        Fixture fixture = prepare();
        String orderId = createOrder(fixture, "po-issue-audit-fail");
        mockMvc.perform(post(ordersUrl(fixture) + "/" + orderId + "/lines")
                        .header("Authorization", bearer(fixture.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"inventoryItemId\":\"" + fixture.itemId()
                                + "\",\"quantity\":\"10.000000\"}"))
                .andExpect(status().isCreated());

        mockMvc.perform(post(ordersUrl(fixture) + "/" + orderId + "/issue")
                        .header("Authorization", bearer(fixture.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":1}"))
                .andExpect(status().isInternalServerError());

        String status = jdbcTemplate.queryForObject(
                "select status from purchase_orders where id = ?", String.class, UUID.fromString(orderId));
        Object issuedAt = jdbcTemplate.queryForObject(
                "select issued_at from purchase_orders where id = ?", Object.class, UUID.fromString(orderId));
        assertThat(status).isEqualTo("DRAFT");
        assertThat(issuedAt).isNull();
        assertThat(auditCountByAction(fixture.rbac().orgId(), "purchase_order.issued")).isZero();
    }

    @Test
    void auditFailureRollsBackCancel() throws Exception {
        Fixture fixture = prepare();
        String orderId = createOrder(fixture, "po-cancel-audit-fail");

        mockMvc.perform(post(ordersUrl(fixture) + "/" + orderId + "/cancel")
                        .header("Authorization", bearer(fixture.token()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0}"))
                .andExpect(status().isInternalServerError());

        String status = jdbcTemplate.queryForObject(
                "select status from purchase_orders where id = ?", String.class, UUID.fromString(orderId));
        Object cancelledAt = jdbcTemplate.queryForObject(
                "select cancelled_at from purchase_orders where id = ?", Object.class, UUID.fromString(orderId));
        assertThat(status).isEqualTo("DRAFT");
        assertThat(cancelledAt).isNull();
        assertThat(auditCountByAction(fixture.rbac().orgId(), "purchase_order.cancelled")).isZero();
    }

    private long auditCountByAction(UUID organizationId, String action) {
        Long count = jdbcTemplate.queryForObject(
                "select count(*) from audit_events where organization_id = ? and action = ?",
                Long.class,
                organizationId,
                action);
        return count == null ? 0 : count;
    }

    @TestConfiguration
    static class FailingTransitionAuditConfig {

        @Bean
        @Primary
        AuditPort failingTransitionAuditPort(AuditRecorder delegate) {
            return record -> {
                if ("purchase_order.issued".equals(record.action())
                        || "purchase_order.cancelled".equals(record.action())) {
                    throw new AuditException("forced purchase order transition audit failure");
                }
                delegate.record(record);
            };
        }
    }
}
