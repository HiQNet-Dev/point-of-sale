package com.hiqnet.pos.cashshift;

import static org.assertj.core.api.Assertions.assertThat;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import com.hiqnet.pos.audit.AbstractAuditIntegrationTest;
import com.hiqnet.pos.branch.domain.Branch;
import com.hiqnet.pos.branch.domain.BranchRepository;
import com.hiqnet.pos.organization.domain.Organization;
import java.util.UUID;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.http.MediaType;

/**
 * Every cash-shift mutation writes EXACTLY ONE append-only audit event inside
 * the business transaction (branch-scoped), with server-derived actor and money
 * rendered as plain strings.
 */
class CashShiftAuditIntegrationTest extends AbstractAuditIntegrationTest {

    @Autowired
    private BranchRepository branchRepository;

    private UUID createBranch(RbacFixture fx, String code) {
        Organization organization = organizationRepository.findById(fx.orgId()).orElseThrow();
        return branchRepository
                .saveAndFlush(Branch.create(organization, "Branch " + code, code, null))
                .getId();
    }

    private String createRegister(RbacFixture fx, UUID branchId, String token, String code) throws Exception {
        String response = mockMvc.perform(post(registersUrl(fx, branchId))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"" + code + "\",\"name\":\"" + code + "\"}"))
                .andExpect(status().isCreated())
                .andReturn()
                .getResponse()
                .getContentAsString();
        return objectMapper.readTree(response).get("id").asText();
    }

    private static String registersUrl(RbacFixture fx, UUID branchId) {
        return "/api/v1/organizations/" + fx.orgId() + "/branches/" + branchId + "/cash-registers";
    }

    private static String shiftsUrl(RbacFixture fx, UUID branchId, String registerId) {
        return registersUrl(fx, branchId) + "/" + registerId + "/cash-shifts";
    }

    @Test
    void openRecordsBranchScopedAuditEvent() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        UUID branchId = createBranch(fx, "centro");
        String registerId = createRegister(fx, branchId, token, "register-1");
        long before = auditCount(fx.orgId());

        String response = mockMvc.perform(post(shiftsUrl(fx, branchId, registerId))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"openingAmount\":100.0000}"))
                .andExpect(status().isCreated())
                .andReturn()
                .getResponse()
                .getContentAsString();
        String shiftId = objectMapper.readTree(response).get("id").asText();

        assertThat(auditCount(fx.orgId())).isEqualTo(before + 1);
        AuditRow row = latestAudit(fx.orgId());
        assertThat(row.action()).isEqualTo("cash-shift.opened");
        assertThat(row.entityType()).isEqualTo("cash_shift");
        assertThat(row.entityId()).hasToString(shiftId);
        assertThat(row.branchId()).isEqualTo(branchId);
        assertThat(row.actorType()).isEqualTo("USER");
        assertThat(row.actorUserId()).isEqualTo(fx.userId());
        assertThat(metadataOf(row)).containsEntry("branchId", branchId.toString());
        assertThat(metadataOf(row)).containsEntry("cashRegisterId", registerId);
        assertThat(metadataOf(row)).containsEntry("openingAmount", "100.0000");
    }

    @Test
    void closeRecordsBranchScopedAuditEvent() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        UUID branchId = createBranch(fx, "centro");
        String registerId = createRegister(fx, branchId, token, "register-1");
        String shiftId = objectMapper.readTree(mockMvc.perform(post(shiftsUrl(fx, branchId, registerId))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"openingAmount\":100.0000}"))
                .andExpect(status().isCreated())
                .andReturn()
                .getResponse()
                .getContentAsString())
                .get("id")
                .asText();
        long before = auditCount(fx.orgId());

        mockMvc.perform(post(shiftsUrl(fx, branchId, registerId) + "/" + shiftId + "/close")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"declaredCashAmount\":1375.5000}"))
                .andExpect(status().isOk());

        assertThat(auditCount(fx.orgId())).isEqualTo(before + 1);
        AuditRow row = latestAudit(fx.orgId());
        assertThat(row.action()).isEqualTo("cash-shift.closed");
        assertThat(row.entityType()).isEqualTo("cash_shift");
        assertThat(row.entityId()).hasToString(shiftId);
        assertThat(row.branchId()).isEqualTo(branchId);
        assertThat(row.actorUserId()).isEqualTo(fx.userId());
        assertThat(metadataOf(row)).containsEntry("cashRegisterId", registerId);
        assertThat(metadataOf(row)).containsEntry("declaredCashAmount", "1375.5000");
    }

    @Test
    void failedOpenWritesNoAuditEvent() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        UUID branchId = createBranch(fx, "centro");
        String registerId = createRegister(fx, branchId, token, "register-1");
        long before = auditCount(fx.orgId());

        mockMvc.perform(post(shiftsUrl(fx, branchId, registerId))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"openingAmount\":-1.0000}"))
                .andExpect(status().isBadRequest());

        assertThat(auditCount(fx.orgId())).isEqualTo(before);
    }
}
