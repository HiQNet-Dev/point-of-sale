package com.hiqnet.pos.purchasing.api;

import static org.assertj.core.api.Assertions.assertThat;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import com.hiqnet.pos.audit.AbstractAuditIntegrationTest;
import com.hiqnet.pos.audit.infrastructure.AuditRecorder;
import com.hiqnet.pos.branch.application.BranchService;
import com.hiqnet.pos.branch.domain.Branch;
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
 * La auditoría es transaccional: si el evento falla, la orden no se persiste.
 */
@Import(PurchaseOrderAuditRollbackTest.FailingPurchaseOrderAuditConfig.class)
class PurchaseOrderAuditRollbackTest extends AbstractAuditIntegrationTest {

    @Autowired
    private BranchService branchService;

    @Autowired
    private SupplierService supplierService;

    @Test
    void auditFailureRollsBackPurchaseOrderCreation() throws Exception {
        RbacFixture fixture = fixture();
        String token = adminToken(fixture);
        Branch branch = branchService.create(fixture.orgId(), "Centro", "sucursal-centro", null);
        Supplier supplier = supplierService.create(
                fixture.orgId(), "proveedor-centro", "Proveedor Centro", null, null, null, null, null, null);

        mockMvc.perform(post("/api/v1/organizations/" + fixture.orgId() + "/branches/" + branch.getId()
                        + "/purchase-orders")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"po-audit-fail\",\"supplierId\":\"" + supplier.getId() + "\"}"))
                .andExpect(status().isInternalServerError());

        Long stored = jdbcTemplate.queryForObject(
                "select count(*) from purchase_orders where organization_id = ?",
                Long.class,
                fixture.orgId());
        assertThat(stored).isZero();
    }

    @TestConfiguration
    static class FailingPurchaseOrderAuditConfig {

        @Bean
        @Primary
        AuditPort failingPurchaseOrderAuditPort(AuditRecorder delegate) {
            return record -> {
                if (record.action().startsWith("purchase_order.")) {
                    throw new AuditException("forced purchase order audit failure");
                }
                delegate.record(record);
            };
        }
    }
}
