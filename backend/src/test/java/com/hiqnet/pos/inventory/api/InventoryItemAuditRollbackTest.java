package com.hiqnet.pos.inventory.api;

import static org.assertj.core.api.Assertions.assertThat;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import com.hiqnet.pos.AbstractRbacIntegrationTest;
import com.hiqnet.pos.audit.infrastructure.AuditRecorder;
import com.hiqnet.pos.shared.audit.AuditException;
import com.hiqnet.pos.shared.audit.AuditPort;
import com.hiqnet.pos.shared.audit.AuditRecord;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.boot.test.context.TestConfiguration;
import org.springframework.context.annotation.Bean;
import org.springframework.context.annotation.Import;
import org.springframework.context.annotation.Primary;
import org.springframework.http.MediaType;
import org.springframework.jdbc.core.JdbcTemplate;

@Import(InventoryItemAuditRollbackTest.FailingInventoryAuditConfig.class)
class InventoryItemAuditRollbackTest extends AbstractRbacIntegrationTest {

    @Autowired
    private JdbcTemplate jdbcTemplate;

    @Test
    void auditFailureRollsBackInventoryItemCreation() throws Exception {
        RbacFixture fixture = fixture();
        String token = adminToken(fixture);
        mockMvc.perform(post("/api/v1/organizations/" + fixture.orgId() + "/inventory-items")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"milk\",\"name\":\"Milk\",\"baseUnit\":\"GRAM\"}"))
                .andExpect(status().isInternalServerError());

        Long stored = jdbcTemplate.queryForObject(
                "select count(*) from inventory_items where organization_id = ?",
                Long.class,
                fixture.orgId());
        assertThat(stored).isZero();
    }

    @TestConfiguration
    static class FailingInventoryAuditConfig {
        @Bean
        @Primary
        AuditPort failingInventoryAuditPort(AuditRecorder delegate) {
            return record -> {
                if (record.action().startsWith("inventory.item.")) {
                    throw new AuditException("forced inventory audit failure");
                }
                delegate.record(record);
            };
        }
    }
}
