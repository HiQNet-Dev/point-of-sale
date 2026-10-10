package com.hiqnet.pos.cashregister;

import static org.assertj.core.api.Assertions.assertThat;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import com.hiqnet.pos.AbstractRbacIntegrationTest;
import com.hiqnet.pos.branch.domain.Branch;
import com.hiqnet.pos.branch.domain.BranchRepository;
import com.hiqnet.pos.organization.domain.Organization;
import com.hiqnet.pos.shared.audit.AuditException;
import com.hiqnet.pos.shared.audit.AuditPort;
import com.hiqnet.pos.shared.audit.AuditRecord;
import java.util.UUID;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.boot.test.context.TestConfiguration;
import org.springframework.context.annotation.Bean;
import org.springframework.context.annotation.Import;
import org.springframework.context.annotation.Primary;
import org.springframework.http.MediaType;
import org.springframework.jdbc.core.JdbcTemplate;

/**
 * A required cash-register audit record that cannot be persisted FAILS the whole
 * mutation ({@code Propagation.MANDATORY}): the register row is rolled back too,
 * so "business commit + missing audit" is impossible.
 */
@Import(CashRegisterAuditRollbackTest.FailingAuditConfig.class)
class CashRegisterAuditRollbackTest extends AbstractRbacIntegrationTest {

    @Autowired
    private BranchRepository branchRepository;

    @Autowired
    private JdbcTemplate jdbcTemplate;

    @Test
    void auditFailureRollsBackCashRegisterCreation() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        Organization organization = organizationRepository.findById(fx.orgId()).orElseThrow();
        UUID branchId = branchRepository
                .saveAndFlush(Branch.create(organization, "Centro", "centro", null))
                .getId();

        mockMvc.perform(post("/api/v1/organizations/" + fx.orgId() + "/branches/" + branchId + "/cash-registers")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"register-1\",\"name\":\"Front\"}"))
                .andExpect(status().isInternalServerError());

        Long stored = jdbcTemplate.queryForObject(
                "select count(*) from cash_registers where organization_id = ?", Long.class, fx.orgId());
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
