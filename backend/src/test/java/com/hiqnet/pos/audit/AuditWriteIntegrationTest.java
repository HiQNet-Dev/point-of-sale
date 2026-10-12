package com.hiqnet.pos.audit;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.patch;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.put;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.jsonPath;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import com.hiqnet.pos.shared.audit.AuditException;
import com.hiqnet.pos.shared.audit.AuditMetadata;
import com.hiqnet.pos.shared.audit.AuditRecord;
import com.hiqnet.pos.users.domain.User;
import java.util.HashMap;
import java.util.Map;
import java.util.UUID;
import org.junit.jupiter.api.Test;
import org.springframework.http.MediaType;
import org.springframework.transaction.IllegalTransactionStateException;
import org.springframework.transaction.support.TransactionTemplate;

/**
 * The write matrix of the audit foundation: every successful administrative
 * mutation produces EXACTLY ONE persistent event (no duplicates) with the
 * correct action/entity actor (from the real JWT: sub → actorUserId, jti →
 * tokenJti), failed operations produce ZERO events, and an audit failure rolls
 * the business mutation back.
 */
class AuditWriteIntegrationTest extends AbstractAuditIntegrationTest {

    @Test
    void organizationUpdateProducesExactlyOneAuditEvent() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        long before = auditCount(fx.orgId());

        mockMvc.perform(patch("/api/v1/organizations/" + fx.orgId())
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"name\":\"Renamed Org\"}"))
                .andExpect(status().isOk());

        assertThat(auditCount(fx.orgId())).isEqualTo(before + 1);
        AuditRow row = latestAudit(fx.orgId());
        assertThat(row.action()).isEqualTo("organization.updated");
        assertThat(row.entityType()).isEqualTo("organization");
        assertThat(row.entityId()).isEqualTo(fx.orgId());
        assertThat(row.branchId()).isNull();
        assertThat(row.actorType()).isEqualTo("USER");
        assertThat(row.actorUserId()).isEqualTo(fx.userId());
        assertThat(row.tokenJti()).isEqualTo(jtiOf(token));
        assertThat(row.metadata()).contains("\"changedFields\"");
    }

    @Test
    void organizationNegativeStockPolicyChangeAuditsChangedFieldAndNoOpDoesNot() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        long before = auditCount(fx.orgId());

        mockMvc.perform(patch("/api/v1/organizations/" + fx.orgId())
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"inventoryNegativeStockPolicy\":\"ALLOW_NEGATIVE\"}"))
                .andExpect(status().isOk());

        assertThat(auditCount(fx.orgId())).isEqualTo(before + 1);
        AuditRow row = latestAudit(fx.orgId());
        assertThat(row.action()).isEqualTo("organization.updated");
        assertThat(row.entityType()).isEqualTo("organization");
        assertThat(metadataOf(row).get("changedFields").toString())
                .isEqualTo("[inventoryNegativeStockPolicy]");

        // STRICT -> ALLOW_NEGATIVE -> ALLOW_NEGATIVE is a no-op: no new event.
        mockMvc.perform(patch("/api/v1/organizations/" + fx.orgId())
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":1,\"inventoryNegativeStockPolicy\":\"ALLOW_NEGATIVE\"}"))
                .andExpect(status().isOk());
        assertThat(auditCount(fx.orgId())).isEqualTo(before + 1);
    }

    @Test
    void branchCreateAndUpdateProduceAudits() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        long before = auditCount(fx.orgId());

        String branchId = createBranchViaApi(fx, token, "suc-1");
        assertThat(auditCount(fx.orgId())).isEqualTo(before + 1);
        AuditRow created = latestAudit(fx.orgId());
        assertThat(created.action()).isEqualTo("branch.created");
        assertThat(created.entityType()).isEqualTo("branch");
        assertThat(created.branchId()).isEqualTo(UUID.fromString(branchId));
        assertThat(created.entityId()).isEqualTo(UUID.fromString(branchId));
        assertThat(created.metadata()).contains("\"code\"");

        mockMvc.perform(patch("/api/v1/organizations/" + fx.orgId() + "/branches/" + branchId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"name\":\"Renamed Branch\"}"))
                .andExpect(status().isOk());

        assertThat(auditCount(fx.orgId())).isEqualTo(before + 2);
        AuditRow updated = latestAudit(fx.orgId());
        assertThat(updated.action()).isEqualTo("branch.updated");
        assertThat(updated.branchId()).isEqualTo(UUID.fromString(branchId));
        assertThat(updated.metadata()).contains("name");
    }

    @Test
    void userCreateProducesAuditWithoutPasswordMaterial() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        long before = auditCount(fx.orgId());
        UUID userId = createUserViaApi(fx, token, "audit-create@example.com");

        AuditRow row = latestAudit(fx.orgId());
        assertThat(auditCount(fx.orgId())).isEqualTo(before + 1);
        assertThat(row.action()).isEqualTo("user.created");
        assertThat(row.entityType()).isEqualTo("user");
        assertThat(row.entityId()).isEqualTo(userId);
        assertThat(row.metadata()).isNull();
    }

    @Test
    void userDeactivateAndReactivateProduceDistinctAudits() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        UUID userId = createUserViaApi(fx, token, "audit-status@example.com");
        long before = auditCount(fx.orgId());

        mockMvc.perform(patch("/api/v1/organizations/" + fx.orgId() + "/users/" + userId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"status\":\"INACTIVE\"}"))
                .andExpect(status().isOk());

        AuditRow deactivated = latestAudit(fx.orgId());
        assertThat(deactivated.action()).isEqualTo("user.deactivated");
        assertThat(metadataOf(deactivated)).containsEntry("fromStatus", "ACTIVE");
        assertThat(metadataOf(deactivated)).containsEntry("toStatus", "INACTIVE");

        mockMvc.perform(patch("/api/v1/organizations/" + fx.orgId() + "/users/" + userId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":1,\"status\":\"ACTIVE\"}"))
                .andExpect(status().isOk());

        AuditRow reactivated = latestAudit(fx.orgId());
        assertThat(auditCount(fx.orgId())).isEqualTo(before + 2);
        assertThat(reactivated.action()).isEqualTo("user.reactivated");
        assertThat(metadataOf(reactivated)).containsEntry("fromStatus", "INACTIVE");
        assertThat(metadataOf(reactivated)).containsEntry("toStatus", "ACTIVE");
    }

    @Test
    void idempotentSameStatusPatchProducesNoAudit() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        UUID userId = createUserViaApi(fx, token, "audit-idem@example.com");
        long before = auditCount(fx.orgId());

        // INACTIVE -> INACTIVE is a no-op: no new audit event.
        mockMvc.perform(patch("/api/v1/organizations/" + fx.orgId() + "/users/" + userId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"status\":\"INACTIVE\"}"))
                .andExpect(status().isOk());
        assertThat(auditCount(fx.orgId())).isEqualTo(before + 1);

        mockMvc.perform(patch("/api/v1/organizations/" + fx.orgId() + "/users/" + userId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":1,\"status\":\"INACTIVE\"}"))
                .andExpect(status().isOk());

        assertThat(auditCount(fx.orgId())).isEqualTo(before + 1);
        assertThat(latestAudit(fx.orgId()).action()).isEqualTo("user.deactivated");
    }

    @Test
    void userEmailUpdateProducesAudit() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        UUID userId = createUserViaApi(fx, token, "audit-email@example.com");
        long before = auditCount(fx.orgId());

        mockMvc.perform(patch("/api/v1/organizations/" + fx.orgId() + "/users/" + userId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"email\":\"audit-email2@example.com\"}"))
                .andExpect(status().isOk());

        AuditRow row = latestAudit(fx.orgId());
        assertThat(auditCount(fx.orgId())).isEqualTo(before + 1);
        assertThat(row.action()).isEqualTo("user.updated");
        assertThat(row.metadata()).contains("\"email\"");
    }

    @Test
    void roleLifecycleProducesAudits() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        long before = auditCount(fx.orgId());

        String roleId = createRoleViaApi(fx, token, "manager", "Manager");
        AuditRow created = latestAudit(fx.orgId());
        assertThat(created.action()).isEqualTo("role.created");
        assertThat(created.entityType()).isEqualTo("role");
        assertThat(created.entityId()).isEqualTo(UUID.fromString(roleId));
        assertThat(metadataOf(created)).containsEntry("code", "manager");

        mockMvc.perform(patch("/api/v1/organizations/" + fx.orgId() + "/roles/" + roleId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"name\":\"Manager v2\"}"))
                .andExpect(status().isOk());
        assertThat(latestAudit(fx.orgId()).action()).isEqualTo("role.updated");

        mockMvc.perform(put("/api/v1/organizations/" + fx.orgId() + "/roles/" + roleId + "/permissions")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"permissions\":[\"user.read\",\"branch.read\"]}"))
                .andExpect(status().isOk());

        AuditRow replaced = latestAudit(fx.orgId());
        assertThat(auditCount(fx.orgId())).isEqualTo(before + 3);
        assertThat(replaced.action()).isEqualTo("role.permissions.replaced");
        assertThat(replaced.entityType()).isEqualTo("role");
        assertThat(replaced.entityId()).isEqualTo(UUID.fromString(roleId));
        assertThat(metadataOf(replaced)).containsEntry("permissionCount", 2);
    }

    @Test
    void userOrganizationRolesReplaceProducesAudit() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        UUID target = createUserViaApi(fx, token, "audit-roles@example.com");
        String roleA = createRoleViaApi(fx, token, "role-a", "Role A");
        String roleB = createRoleViaApi(fx, token, "role-b", "Role B");
        long before = auditCount(fx.orgId());

        mockMvc.perform(put("/api/v1/organizations/" + fx.orgId() + "/users/" + target + "/roles")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"roleIds\":[\"" + roleA + "\",\"" + roleB + "\"]}"))
                .andExpect(status().isOk());

        AuditRow row = latestAudit(fx.orgId());
        assertThat(auditCount(fx.orgId())).isEqualTo(before + 1);
        assertThat(row.action()).isEqualTo("user.roles.replaced");
        assertThat(row.entityType()).isEqualTo("user");
        assertThat(row.entityId()).isEqualTo(target);
        assertThat(metadataOf(row)).containsEntry("roleCount", 2);
    }

    @Test
    void branchMembershipLifecycleProducesAudits() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String branchId = createBranchViaApi(fx, token, "suc-m");
        UUID member = createUserViaApi(fx, token, "audit-member@example.com");
        long before = auditCount(fx.orgId());

        mockMvc.perform(post("/api/v1/organizations/" + fx.orgId() + "/branches/" + branchId + "/memberships")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"userId\":\"" + member + "\"}"))
                .andExpect(status().isCreated());

        AuditRow created = latestAudit(fx.orgId());
        assertThat(created.action()).isEqualTo("branch.membership.created");
        assertThat(created.entityType()).isEqualTo("branch_membership");
        assertThat(created.branchId()).isEqualTo(UUID.fromString(branchId));

        mockMvc.perform(patch("/api/v1/organizations/" + fx.orgId() + "/branches/" + branchId
                                + "/memberships/" + member)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"status\":\"INACTIVE\"}"))
                .andExpect(status().isOk());
        AuditRow updated = latestAudit(fx.orgId());
        assertThat(updated.action()).isEqualTo("branch.membership.updated");
        assertThat(metadataOf(updated)).containsEntry("fromStatus", "ACTIVE");

        String roleId = createRoleViaApi(fx, token, "suc-manager", "Suc Manager");
        long beforePut = auditCount(fx.orgId());
        mockMvc.perform(put("/api/v1/organizations/" + fx.orgId() + "/branches/" + branchId
                                + "/memberships/" + member + "/roles")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"roleIds\":[\"" + roleId + "\"]}"))
                .andExpect(status().isOk());

        AuditRow roles = latestAudit(fx.orgId());
        assertThat(auditCount(fx.orgId())).isEqualTo(beforePut + 1);
        assertThat(roles.action()).isEqualTo("branch.membership.roles.replaced");
        assertThat(roles.branchId()).isEqualTo(UUID.fromString(branchId));
        assertThat(metadataOf(roles)).containsEntry("roleCount", 1);
    }

    @Test
    void staleExpectedVersionProducesNoAudit() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        UUID userId = createUserViaApi(fx, token, "audit-stale@example.com");
        long before = auditCount(fx.orgId());

        mockMvc.perform(patch("/api/v1/organizations/" + fx.orgId() + "/users/" + userId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":99,\"status\":\"INACTIVE\"}"))
                .andExpect(status().isConflict());

        assertThat(auditCount(fx.orgId())).isEqualTo(before);
    }

    @Test
    void authorizationDenialProducesNoAudit() throws Exception {
        RbacFixture fx = fixture();
        String adminToken = adminToken(fx);
        UUID target = createUserViaApi(fx, adminToken, "audit-deny@example.com");

        User restricted = createUserForOrganization(
                fx.orgId().toString(), uniqueEmail("audit-restricted@"), PASSWORD);
        assignRoleTo(restricted.getId(), fx, "user.read");
        String restrictedToken = postLoginExpectingToken(fx.orgCode(), emailFor(restricted), PASSWORD);
        long before = auditCount(fx.orgId());

        mockMvc.perform(patch("/api/v1/organizations/" + fx.orgId() + "/users/" + target)
                        .header("Authorization", bearer(restrictedToken))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"status\":\"INACTIVE\"}"))
                .andExpect(status().isForbidden())
                .andExpect(jsonPath("$.code").value("FORBIDDEN"));

        assertThat(auditCount(fx.orgId())).isEqualTo(before);
    }

    @Test
    void crossOrganizationMutationProducesNoAuditInEitherOrganization() throws Exception {
        RbacFixture fxA = fixture();
        RbacFixture fxB = fixture();
        String tokenA = adminToken(fxA);
        UUID userB = createUserViaApi(fxB, adminToken(fxB), "audit-cross@example.com");
        long beforeA = auditCount(fxA.orgId());
        long beforeB = auditCount(fxB.orgId());

        mockMvc.perform(patch("/api/v1/organizations/" + fxB.orgId() + "/users/" + userB)
                        .header("Authorization", bearer(tokenA))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"status\":\"INACTIVE\"}"))
                .andExpect(status().isForbidden());

        assertThat(auditCount(fxA.orgId())).isEqualTo(beforeA);
        assertThat(auditCount(fxB.orgId())).isEqualTo(beforeB);
    }

    @Test
    void systemActorIsRecordedWithoutAuthentication() {
        RbacFixture fx = fixture();
        new TransactionTemplate(transactionManager).executeWithoutResult(status ->
                auditPort.record(AuditRecord.of(
                        fx.orgId(), "internal.process", "system", null, AuditMetadata.empty())));

        AuditRow row = latestAudit(fx.orgId());
        assertThat(row.actorType()).isEqualTo("SYSTEM");
        assertThat(row.actorUserId()).isNull();
        assertThat(row.tokenJti()).isNull();
        assertThat(row.action()).isEqualTo("internal.process");
    }

    @Test
    void recordOutsideAnActiveTransactionIsRejected() {
        RbacFixture fx = fixture();
        // Propagation.MANDATORY: an audit record without a surrounding business
        // transaction is a programming error and fails fast.
        assertThatThrownBy(() -> auditPort.record(AuditRecord.of(
                        fx.orgId(), "no.tx", "x", null, AuditMetadata.empty())))
                .isInstanceOf(IllegalTransactionStateException.class);
        assertThat(auditCount(fx.orgId())).isZero();
    }

    @Test
    void metadataOverSizeLimitRollsBack() {
        RbacFixture fx = fixture();
        long before = auditCount(fx.orgId());
        Map<String, Object> big = new HashMap<>();
        big.put("payload", "x".repeat(20 * 1024));
        TransactionTemplate tt = new TransactionTemplate(transactionManager);

        assertThatThrownBy(() -> tt.executeWithoutResult(status ->
                        auditPort.record(AuditRecord.of(
                                fx.orgId(), "metadata.too.large", "test", null, AuditMetadata.of(big)))))
                .isInstanceOf(AuditException.class);

        assertThat(auditCount(fx.orgId())).isEqualTo(before);
    }

    // --- fixtures ----------------------------------------------------------

    private String createBranchViaApi(RbacFixture fx, String token, String code) throws Exception {
        String body = "{\"name\":\"Branch " + code + "\",\"code\":\"" + code + "\"}";
        String response = mockMvc.perform(post("/api/v1/organizations/" + fx.orgId() + "/branches")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(body))
                .andExpect(status().isCreated())
                .andReturn()
                .getResponse()
                .getContentAsString();
        return objectMapper.readTree(response).get("id").asText();
    }

    private UUID createUserViaApi(RbacFixture fx, String token, String email) throws Exception {
        String response = mockMvc.perform(post("/api/v1/organizations/" + fx.orgId() + "/users")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"email\":\"" + email + "\",\"password\":\"" + PASSWORD + "\"}"))
                .andExpect(status().isCreated())
                .andReturn()
                .getResponse()
                .getContentAsString();
        return UUID.fromString(objectMapper.readTree(response).get("id").asText());
    }

    private String createRoleViaApi(RbacFixture fx, String token, String code, String name) throws Exception {
        String response = mockMvc.perform(post("/api/v1/organizations/" + fx.orgId() + "/roles")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"" + code + "\",\"name\":\"" + name + "\"}"))
                .andExpect(status().isCreated())
                .andReturn()
                .getResponse()
                .getContentAsString();
        return objectMapper.readTree(response).get("id").asText();
    }

    private void assignRoleTo(UUID targetUserId, RbacFixture fx, String permissionCode) {
        UUID roleId = createRole(fx, "actor-" + UUID.randomUUID().toString().substring(0, 8), "Actor", permissionCode);
        userOrganizationRoleRepository.saveAndFlush(
                com.hiqnet.pos.rbac.domain.UserOrganizationRole.of(fx.orgId(), targetUserId, roleId));
    }
}