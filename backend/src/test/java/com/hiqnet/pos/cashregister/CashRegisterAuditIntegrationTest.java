package com.hiqnet.pos.cashregister;

import static org.assertj.core.api.Assertions.assertThat;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.patch;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import com.hiqnet.pos.audit.AbstractAuditIntegrationTest;
import com.hiqnet.pos.branch.domain.Branch;
import com.hiqnet.pos.branch.domain.BranchRepository;
import com.hiqnet.pos.organization.domain.Organization;
import java.util.List;
import java.util.UUID;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.http.MediaType;

/**
 * Every cash-register mutation writes EXACTLY ONE append-only audit event inside
 * the business transaction (branch-scoped), and a no-op update writes none.
 */
class CashRegisterAuditIntegrationTest extends AbstractAuditIntegrationTest {

    @Autowired
    private BranchRepository branchRepository;

    private UUID createBranch(RbacFixture fx, String code) {
        Organization organization = organizationRepository.findById(fx.orgId()).orElseThrow();
        return branchRepository
                .saveAndFlush(Branch.create(organization, "Branch " + code, code, null))
                .getId();
    }

    private String createRegister(RbacFixture fx, UUID branchId, String token, String code, String name)
            throws Exception {
        String response = mockMvc.perform(post(url(fx, branchId))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"" + code + "\",\"name\":\"" + name + "\"}"))
                .andExpect(status().isCreated())
                .andReturn()
                .getResponse()
                .getContentAsString();
        return objectMapper.readTree(response).get("id").asText();
    }

    @Test
    void createRegisterRecordsBranchScopedAuditEvent() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        UUID branchId = createBranch(fx, "centro");
        String registerId = createRegister(fx, branchId, token, "register-1", "Front");

        AuditRow row = latestAudit(fx.orgId());
        assertThat(row.action()).isEqualTo("cash-register.created");
        assertThat(row.entityType()).isEqualTo("cash_register");
        assertThat(row.entityId()).hasToString(registerId);
        assertThat(row.branchId()).isEqualTo(branchId);
        assertThat(metadataOf(row)).containsEntry("code", "register-1");
        assertThat(metadataOf(row)).containsEntry("branchId", branchId.toString());
    }

    @Test
    void updateRegisterRecordsChangedFields() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        UUID branchId = createBranch(fx, "centro");
        String registerId = createRegister(fx, branchId, token, "register-1", "Front");

        mockMvc.perform(patch(url(fx, branchId) + "/" + registerId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"name\":\"Front Desk\",\"status\":\"INACTIVE\"}"))
                .andExpect(status().isOk());

        AuditRow row = latestAudit(fx.orgId());
        assertThat(row.action()).isEqualTo("cash-register.updated");
        assertThat(row.entityType()).isEqualTo("cash_register");
        assertThat(row.branchId()).isEqualTo(branchId);
        @SuppressWarnings("unchecked")
        List<String> changedFields = (List<String>) metadataOf(row).get("changedFields");
        assertThat(changedFields).containsExactlyInAnyOrder("name", "status");
    }

    @Test
    void noOpUpdateWritesNoAuditEvent() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        UUID branchId = createBranch(fx, "centro");
        String registerId = createRegister(fx, branchId, token, "register-1", "Front");
        long before = auditCount(fx.orgId());

        mockMvc.perform(patch(url(fx, branchId) + "/" + registerId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0}"))
                .andExpect(status().isOk());

        assertThat(auditCount(fx.orgId())).isEqualTo(before);
    }

    private static String url(RbacFixture fx, UUID branchId) {
        return "/api/v1/organizations/" + fx.orgId() + "/branches/" + branchId + "/cash-registers";
    }
}
