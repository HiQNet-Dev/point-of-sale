package com.hiqnet.pos.inventory.stock;
import static org.assertj.core.api.Assertions.assertThat;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;
import com.hiqnet.pos.audit.AbstractAuditIntegrationTest;
import com.hiqnet.pos.audit.infrastructure.AuditRecorder;
import com.hiqnet.pos.branch.domain.Branch;
import com.hiqnet.pos.branch.domain.BranchRepository;
import com.hiqnet.pos.inventory.application.InventoryItemService;
import com.hiqnet.pos.inventory.domain.InventoryBaseUnit;
import com.hiqnet.pos.organization.domain.OrganizationRepository;
import com.hiqnet.pos.shared.audit.AuditException;
import com.hiqnet.pos.shared.audit.AuditPort;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.boot.test.context.TestConfiguration;
import org.springframework.context.annotation.Bean;
import org.springframework.context.annotation.Import;
import org.springframework.context.annotation.Primary;
import org.springframework.http.MediaType;
import org.springframework.jdbc.core.JdbcTemplate;

@Import(InventoryStockAuditRollbackTest.FailingAuditConfig.class)
class InventoryStockAuditRollbackTest extends AbstractAuditIntegrationTest {
    @Autowired BranchRepository branches;
    @Autowired OrganizationRepository organizations;
    @Autowired InventoryItemService items;
    @Autowired JdbcTemplate jdbc;
    @Test void auditFailureRollsBackNewBalanceAndMovement() throws Exception {
        var fx=fixture(); var branch=branches.saveAndFlush(Branch.create(organizations.findById(fx.orgId()).orElseThrow(),"Main","rollback-"+java.util.UUID.randomUUID().toString().substring(0,8),null));
        var item=items.create(fx.orgId(),"rollback-"+java.util.UUID.randomUUID().toString().substring(0,8),"Item",null,InventoryBaseUnit.GRAM,false);
        long stockAuditsBefore=jdbc.queryForObject("select count(*) from audit_events where organization_id=? and action like 'inventory.stock.%'",Long.class,fx.orgId());
        mockMvc.perform(post("/api/v1/organizations/"+fx.orgId()+"/branches/"+branch.getId()+"/inventory-items/"+item.getId()+"/opening-stock")
                .header("Authorization",bearer(adminToken(fx))).contentType(MediaType.APPLICATION_JSON)
                .content("{\"expectedVersion\":0,\"quantity\":\"5\"}"))
                .andExpect(status().isInternalServerError());
        assertThat(jdbc.queryForObject("select count(*) from branch_inventory where branch_id=? and inventory_item_id=?",Long.class,branch.getId(),item.getId())).isZero();
        assertThat(jdbc.queryForObject("select count(*) from inventory_movements where branch_id=? and inventory_item_id=?",Long.class,branch.getId(),item.getId())).isZero();
        assertThat(jdbc.queryForObject("select count(*) from audit_events where organization_id=? and action like 'inventory.stock.%'",Long.class,fx.orgId())).isEqualTo(stockAuditsBefore);
    }
    @TestConfiguration static class FailingAuditConfig {
        @Bean @Primary AuditPort failingStockAudit(AuditRecorder delegate) { return record->{
            if(record.action().startsWith("inventory.stock.")) throw new AuditException("forced stock audit failure");
            delegate.record(record);
        }; }
    }
}
