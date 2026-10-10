package com.hiqnet.pos.catalog;

import static org.assertj.core.api.Assertions.assertThat;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import com.hiqnet.pos.AbstractRbacIntegrationTest;
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

/**
 * A required modifier audit record that cannot be persisted FAILS the whole
 * mutation ({@code Propagation.MANDATORY}): the group row is rolled back too, so
 * "business commit + missing audit" is impossible.
 */
@Import(ModifierAuditRollbackTest.FailingAuditConfig.class)
class ModifierAuditRollbackTest extends AbstractRbacIntegrationTest {

    @Autowired
    private JdbcTemplate jdbcTemplate;

    @Test
    void auditFailureRollsBackModifierGroupCreation() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        mockMvc.perform(post("/api/v1/organizations/" + fx.orgId() + "/modifier-groups")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"milk\",\"name\":\"Milk\"}"))
                .andExpect(status().isInternalServerError());

        Long stored = jdbcTemplate.queryForObject(
                "select count(*) from modifier_groups where organization_id = ?",
                Long.class,
                fx.orgId());
        assertThat(stored).isZero();
    }

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
