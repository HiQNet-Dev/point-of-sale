package com.hiqnet.pos.purchasing.api;

import static org.assertj.core.api.Assertions.assertThat;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import com.hiqnet.pos.AbstractRbacIntegrationTest;
import com.hiqnet.pos.audit.infrastructure.AuditRecorder;
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

@Import(SupplierAuditRollbackTest.FailingSupplierAuditConfig.class)
class SupplierAuditRollbackTest extends AbstractRbacIntegrationTest {

    @Autowired
    private JdbcTemplate jdbcTemplate;

    @Test
    void auditFailureRollsBackSupplierCreation() throws Exception {
        RbacFixture fixture = fixture();
        String token = adminToken(fixture);
        mockMvc.perform(post("/api/v1/organizations/" + fixture.orgId() + "/suppliers")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"supplier\",\"name\":\"Supplier\"}"))
                .andExpect(status().isInternalServerError());

        Long stored = jdbcTemplate.queryForObject(
                "select count(*) from suppliers where organization_id = ?", Long.class, fixture.orgId());
        assertThat(stored).isZero();
    }

    @TestConfiguration
    static class FailingSupplierAuditConfig {

        @Bean
        @Primary
        AuditPort failingSupplierAuditPort(AuditRecorder delegate) {
            return record -> {
                if (record.action().startsWith("supplier.")) {
                    throw new AuditException("forced supplier audit failure");
                }
                delegate.record(record);
            };
        }
    }
}
