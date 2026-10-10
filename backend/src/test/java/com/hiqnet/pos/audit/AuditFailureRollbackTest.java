package com.hiqnet.pos.audit;

import static org.assertj.core.api.Assertions.assertThat;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.patch;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import com.hiqnet.pos.AbstractRbacIntegrationTest;
import com.hiqnet.pos.branch.domain.Branch;
import com.hiqnet.pos.branch.domain.BranchRepository;
import com.hiqnet.pos.organization.domain.Organization;
import com.hiqnet.pos.organization.domain.OrganizationRepository;
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

/**
 * A required audit record that cannot be persisted FAILS the whole mutation:
 * the {@code record} call runs inside the business transaction
 * ({@code Propagation.MANDATORY}), so a failing audit rolls the branch update
 * back — the optimistic "business commit + missing audit" gap is impossible.
 */
@Import(AuditFailureRollbackTest.FailingAuditConfig.class)
class AuditFailureRollbackTest extends AbstractRbacIntegrationTest {

    @Autowired
    private BranchRepository branchRepository;

    @Autowired
    private OrganizationRepository organizationRepository;

    @Test
    void auditFailureRollsBackTheBusinessMutation() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        Organization organization = organizationRepository.findById(fx.orgId()).orElseThrow();
        Branch branch = branchRepository.saveAndFlush(
                Branch.create(organization, "Old Name", "old-branch", null));
        UUID branchId = branch.getId();

        mockMvc.perform(patch("/api/v1/organizations/" + fx.orgId() + "/branches/" + branchId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"name\":\"New Name\"}"))
                .andExpect(status().isInternalServerError());

        Branch reloaded = branchRepository.findById(branchId).orElseThrow();
        assertThat(reloaded.getName()).isEqualTo("Old Name");
        assertThat(reloaded.getVersion()).isZero();
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