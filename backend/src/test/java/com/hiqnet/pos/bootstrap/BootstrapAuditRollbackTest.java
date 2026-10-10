package com.hiqnet.pos.bootstrap;

import static org.assertj.core.api.Assertions.assertThat;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import com.hiqnet.pos.shared.audit.AuditException;
import com.hiqnet.pos.shared.audit.AuditPort;
import com.hiqnet.pos.shared.audit.AuditRecord;
import org.junit.jupiter.api.Test;
import org.springframework.boot.test.context.TestConfiguration;
import org.springframework.context.annotation.Bean;
import org.springframework.context.annotation.Import;
import org.springframework.context.annotation.Primary;
import org.springframework.http.MediaType;

/**
 * Audit is part of the bootstrap transaction: if the FIRST audit record
 * (organization.created) cannot be persisted, the entire provisioning rolls
 * back — zero organizations/users/roles, bootstrap stays PENDING and a retry
 * with the same secret works.
 */
@Import(BootstrapAuditRollbackTest.FailingAuditConfig.class)
class BootstrapAuditRollbackTest extends AbstractBootstrapIntegrationTest {

    @Test
    void auditFailureRollsBackTheEntireProvisioning() throws Exception {
        mockMvc.perform(post("/api/v1/bootstrap")
                        .header("X-Bootstrap-Token", bootstrapSecret())
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(bootstrapBody("rollback-org", uniqueEmail("admin@"), PASSWORD)))
                .andExpect(status().isInternalServerError());

        assertThat(organizationRepository.count()).isZero();
        assertThat(userRepository.count()).isZero();
        assertThat(roleRepository.count()).isZero();
        assertThat(installationCompleted()).isFalse();
        // The installation can be retried cleanly afterwards.
        assertThat(retryable()).isTrue();
    }

    private boolean retryable() {
        Boolean pending = jdbcTemplate.queryForObject(
                "select completed_at is null and organization_id is null from installation_bootstrap where id = 1",
                Boolean.class);
        return Boolean.TRUE.equals(pending);
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