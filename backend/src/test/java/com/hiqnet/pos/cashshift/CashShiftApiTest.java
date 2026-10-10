package com.hiqnet.pos.cashshift;

import static org.assertj.core.api.Assertions.assertThat;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.get;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.patch;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.jsonPath;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import com.hiqnet.pos.AbstractRbacIntegrationTest;
import com.hiqnet.pos.branch.domain.Branch;
import com.hiqnet.pos.branch.domain.BranchRepository;
import com.hiqnet.pos.branch.domain.BranchStatus;
import com.hiqnet.pos.organization.domain.Organization;
import com.hiqnet.pos.rbac.domain.UserOrganizationRole;
import com.hiqnet.pos.users.domain.User;
import java.util.Map;
import java.util.UUID;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.http.MediaType;
import org.springframework.jdbc.core.JdbcTemplate;

/**
 * Cash shift lifecycle under the REAL security chain + RBAC: open, read, list,
 * close, closed immutability, actor derivation from the JWT, money precision,
 * register/branch ACTIVE rules and scoping.
 */
class CashShiftApiTest extends AbstractRbacIntegrationTest {

    @Autowired
    private BranchRepository branchRepository;

    @Autowired
    private JdbcTemplate jdbcTemplate;

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

    private String openShift(RbacFixture fx, UUID branchId, String registerId, String token, String body)
            throws Exception {
        return mockMvc.perform(post(shiftsUrl(fx, branchId, registerId))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(body))
                .andExpect(status().isCreated())
                .andReturn()
                .getResponse()
                .getContentAsString();
    }

    private static String registersUrl(RbacFixture fx, UUID branchId) {
        return "/api/v1/organizations/" + fx.orgId() + "/branches/" + branchId + "/cash-registers";
    }

    private static String shiftsUrl(RbacFixture fx, UUID branchId, String registerId) {
        return registersUrl(fx, branchId) + "/" + registerId + "/cash-shifts";
    }

    @Test
    void openReturns201WithServerAssignedStateAndActor() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        UUID branchId = createBranch(fx, "centro");
        String registerId = createRegister(fx, branchId, token, "register-1");

        mockMvc.perform(post(shiftsUrl(fx, branchId, registerId))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"openingAmount\":500.0000}"))
                .andExpect(status().isCreated())
                .andExpect(jsonPath("$.id").isNotEmpty())
                .andExpect(jsonPath("$.organizationId").value(fx.orgId().toString()))
                .andExpect(jsonPath("$.branchId").value(branchId.toString()))
                .andExpect(jsonPath("$.cashRegisterId").value(registerId))
                .andExpect(jsonPath("$.status").value("OPEN"))
                .andExpect(jsonPath("$.openingAmount").value(500.0))
                .andExpect(jsonPath("$.closingDeclaredAmount").doesNotExist())
                .andExpect(jsonPath("$.closedAt").doesNotExist())
                .andExpect(jsonPath("$.closedByUserId").doesNotExist())
                .andExpect(jsonPath("$.openedByUserId").value(fx.userId().toString()))
                .andExpect(jsonPath("$.openedAt").isNotEmpty())
                .andExpect(jsonPath("$.version").value(0));
    }

    @Test
    void openIgnoresClientSuppliedActorAndTimestamp() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        UUID branchId = createBranch(fx, "centro");
        String registerId = createRegister(fx, branchId, token, "register-1");
        String spoofed = UUID.randomUUID().toString();

        mockMvc.perform(post(shiftsUrl(fx, branchId, registerId))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"openingAmount\":100.0000,\"openedByUserId\":\"" + spoofed
                                + "\",\"openedAt\":\"2000-01-01T00:00:00Z\",\"status\":\"CLOSED\"}"))
                .andExpect(status().isCreated())
                .andExpect(jsonPath("$.openedByUserId").value(fx.userId().toString()))
                .andExpect(jsonPath("$.status").value("OPEN"));
    }

    @Test
    void openSecondShiftOnSameRegisterIs409() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        UUID branchId = createBranch(fx, "centro");
        String registerId = createRegister(fx, branchId, token, "register-1");
        openShift(fx, branchId, registerId, token, "{\"openingAmount\":100.0000}");

        mockMvc.perform(post(shiftsUrl(fx, branchId, registerId))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"openingAmount\":50.0000}"))
                .andExpect(status().isConflict())
                .andExpect(jsonPath("$.code").value("CASH_REGISTER_ALREADY_HAS_OPEN_SHIFT"));
    }

    @Test
    void openRejectsNegativeAmount() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        UUID branchId = createBranch(fx, "centro");
        String registerId = createRegister(fx, branchId, token, "register-1");

        mockMvc.perform(post(shiftsUrl(fx, branchId, registerId))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"openingAmount\":-0.0001}"))
                .andExpect(status().isBadRequest())
                .andExpect(jsonPath("$.code").value("VALIDATION_ERROR"));
    }

    @Test
    void openRejectsAmountWithMoreThanFourDecimalsWithoutRounding() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        UUID branchId = createBranch(fx, "centro");
        String registerId = createRegister(fx, branchId, token, "register-1");

        mockMvc.perform(post(shiftsUrl(fx, branchId, registerId))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"openingAmount\":10.12345}"))
                .andExpect(status().isBadRequest())
                .andExpect(jsonPath("$.code").value("VALIDATION_ERROR"));
    }

    @Test
    void openAllowsZeroAmount() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        UUID branchId = createBranch(fx, "centro");
        String registerId = createRegister(fx, branchId, token, "register-1");

        openShift(fx, branchId, registerId, token, "{\"openingAmount\":0.0000}");
    }

    @Test
    void openRejectsAmountBeyondColumnPrecision() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        UUID branchId = createBranch(fx, "centro");
        String registerId = createRegister(fx, branchId, token, "register-1");

        mockMvc.perform(post(shiftsUrl(fx, branchId, registerId))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"openingAmount\":1000000000000000}"))
                .andExpect(status().isBadRequest())
                .andExpect(jsonPath("$.code").value("VALIDATION_ERROR"));
    }

    @Test
    void openOnInactiveRegisterIs409() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        UUID branchId = createBranch(fx, "centro");
        String registerId = createRegister(fx, branchId, token, "register-1");
        mockMvc.perform(patch(registersUrl(fx, branchId) + "/" + registerId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"status\":\"INACTIVE\"}"))
                .andExpect(status().isOk());

        mockMvc.perform(post(shiftsUrl(fx, branchId, registerId))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"openingAmount\":100.0000}"))
                .andExpect(status().isConflict())
                .andExpect(jsonPath("$.code").value("CASH_REGISTER_INACTIVE"));
    }

    @Test
    void openOnInactiveBranchIs409() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        UUID branchId = createBranch(fx, "centro");
        String registerId = createRegister(fx, branchId, token, "register-1");
        Branch branch = branchRepository.findById(branchId).orElseThrow();
        branch.updateStatus(BranchStatus.INACTIVE);
        branchRepository.saveAndFlush(branch);

        mockMvc.perform(post(shiftsUrl(fx, branchId, registerId))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"openingAmount\":100.0000}"))
                .andExpect(status().isConflict())
                .andExpect(jsonPath("$.code").value("BRANCH_INACTIVE"));
    }

    @Test
    void closeSetsClosingStateAndBumpsVersion() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        UUID branchId = createBranch(fx, "centro");
        String registerId = createRegister(fx, branchId, token, "register-1");
        String shiftId = objectMapper.readTree(openShift(fx, branchId, registerId, token, "{\"openingAmount\":500.0000}"))
                .get("id")
                .asText();

        mockMvc.perform(post(shiftsUrl(fx, branchId, registerId) + "/" + shiftId + "/close")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"declaredCashAmount\":1375.5000}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.status").value("CLOSED"))
                .andExpect(jsonPath("$.closingDeclaredAmount").value(1375.5))
                .andExpect(jsonPath("$.closedByUserId").value(fx.userId().toString()))
                .andExpect(jsonPath("$.closedAt").isNotEmpty())
                .andExpect(jsonPath("$.version").value(1));
    }

    @Test
    void closeWithDifferentUserRecordsThatCloser() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        UUID branchId = createBranch(fx, "centro");
        String registerId = createRegister(fx, branchId, token, "register-1");
        String shiftId = objectMapper.readTree(openShift(fx, branchId, registerId, token, "{\"openingAmount\":500.0000}"))
                .get("id")
                .asText();

        User closer = createUserForOrganization(fx.orgId().toString(), uniqueEmail("closer@"), PASSWORD);
        UUID roleId = createRole(fx, "closer", "Closer", "cash-shift.close");
        userOrganizationRoleRepository.saveAndFlush(UserOrganizationRole.of(fx.orgId(), closer.getId(), roleId));
        String closerToken = postLoginExpectingToken(fx.orgCode(), closer.getEmail(), PASSWORD);

        mockMvc.perform(post(shiftsUrl(fx, branchId, registerId) + "/" + shiftId + "/close")
                        .header("Authorization", bearer(closerToken))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"declaredCashAmount\":600.0000}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.openedByUserId").value(fx.userId().toString()))
                .andExpect(jsonPath("$.closedByUserId").value(closer.getId().toString()));
    }

    @Test
    void closeAlreadyClosedIs409() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        UUID branchId = createBranch(fx, "centro");
        String registerId = createRegister(fx, branchId, token, "register-1");
        String shiftId = objectMapper.readTree(openShift(fx, branchId, registerId, token, "{\"openingAmount\":500.0000}"))
                .get("id")
                .asText();
        String closeUrl = shiftsUrl(fx, branchId, registerId) + "/" + shiftId + "/close";
        mockMvc.perform(post(closeUrl)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"declaredCashAmount\":500.0000}"))
                .andExpect(status().isOk());
        Map<String, Object> closed = shiftRow(shiftId);

        mockMvc.perform(post(closeUrl)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":1,\"declaredCashAmount\":500.0000}"))
                .andExpect(status().isConflict())
                .andExpect(jsonPath("$.code").value("CASH_SHIFT_ALREADY_CLOSED"));

        // A re-close is a no-op on the row: every column is byte-for-byte identical.
        assertThat(shiftRow(shiftId)).isEqualTo(closed);
    }

    @Test
    void closeAlreadyClosedWithStaleVersionStillReportsAlreadyClosed() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        UUID branchId = createBranch(fx, "centro");
        String registerId = createRegister(fx, branchId, token, "register-1");
        String shiftId = objectMapper.readTree(openShift(fx, branchId, registerId, token, "{\"openingAmount\":500.0000}"))
                .get("id")
                .asText();
        String closeUrl = shiftsUrl(fx, branchId, registerId) + "/" + shiftId + "/close";
        mockMvc.perform(post(closeUrl)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"declaredCashAmount\":500.0000}"))
                .andExpect(status().isOk());

        // Documented precedence: an already-CLOSED shift reports CASH_SHIFT_ALREADY_CLOSED
        // even when expectedVersion is also stale (status is checked before the version).
        mockMvc.perform(post(closeUrl)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"declaredCashAmount\":500.0000}"))
                .andExpect(status().isConflict())
                .andExpect(jsonPath("$.code").value("CASH_SHIFT_ALREADY_CLOSED"));
    }

    private Map<String, Object> shiftRow(String shiftId) {
        return jdbcTemplate.queryForMap(
                "select status, version, opened_by_user_id, opened_at, closed_by_user_id, closed_at, "
                        + "opening_amount, closing_declared_amount from cash_shifts where id = ?",
                UUID.fromString(shiftId));
    }

    @Test
    void closeWithStaleVersionIs409() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        UUID branchId = createBranch(fx, "centro");
        String registerId = createRegister(fx, branchId, token, "register-1");
        String shiftId = objectMapper.readTree(openShift(fx, branchId, registerId, token, "{\"openingAmount\":500.0000}"))
                .get("id")
                .asText();

        mockMvc.perform(post(shiftsUrl(fx, branchId, registerId) + "/" + shiftId + "/close")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":5,\"declaredCashAmount\":500.0000}"))
                .andExpect(status().isConflict())
                .andExpect(jsonPath("$.code").value("CONCURRENT_MODIFICATION"));
    }

    @Test
    void closeRejectsAmountWithTooManyDecimals() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        UUID branchId = createBranch(fx, "centro");
        String registerId = createRegister(fx, branchId, token, "register-1");
        String shiftId = objectMapper.readTree(openShift(fx, branchId, registerId, token, "{\"openingAmount\":500.0000}"))
                .get("id")
                .asText();

        mockMvc.perform(post(shiftsUrl(fx, branchId, registerId) + "/" + shiftId + "/close")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"declaredCashAmount\":1.00001}"))
                .andExpect(status().isBadRequest())
                .andExpect(jsonPath("$.code").value("VALIDATION_ERROR"));
    }

    @Test
    void closeWorksEvenWhenRegisterAndBranchBecameInactive() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        UUID branchId = createBranch(fx, "centro");
        String registerId = createRegister(fx, branchId, token, "register-1");
        String shiftId = objectMapper.readTree(openShift(fx, branchId, registerId, token, "{\"openingAmount\":500.0000}"))
                .get("id")
                .asText();

        mockMvc.perform(patch(registersUrl(fx, branchId) + "/" + registerId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"status\":\"INACTIVE\"}"))
                .andExpect(status().isOk());
        Branch branch = branchRepository.findById(branchId).orElseThrow();
        branch.updateStatus(BranchStatus.INACTIVE);
        branchRepository.saveAndFlush(branch);

        mockMvc.perform(post(shiftsUrl(fx, branchId, registerId) + "/" + shiftId + "/close")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"declaredCashAmount\":500.0000}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.status").value("CLOSED"));
    }

    @Test
    void listReturnsShiftsNewestFirstWithOptionalStatusFilter() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        UUID branchId = createBranch(fx, "centro");
        String registerId = createRegister(fx, branchId, token, "register-1");

        String first = objectMapper.readTree(openShift(fx, branchId, registerId, token, "{\"openingAmount\":10.0000}"))
                .get("id")
                .asText();
        mockMvc.perform(post(shiftsUrl(fx, branchId, registerId) + "/" + first + "/close")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"declaredCashAmount\":10.0000}"))
                .andExpect(status().isOk());
        String second = objectMapper.readTree(openShift(fx, branchId, registerId, token, "{\"openingAmount\":20.0000}"))
                .get("id")
                .asText();

        mockMvc.perform(get(shiftsUrl(fx, branchId, registerId)).header("Authorization", bearer(token)))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.items.length()").value(2))
                .andExpect(jsonPath("$.items[0].id").value(second))
                .andExpect(jsonPath("$.items[1].id").value(first));

        mockMvc.perform(get(shiftsUrl(fx, branchId, registerId))
                        .header("Authorization", bearer(token))
                        .param("status", "OPEN"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.items.length()").value(1))
                .andExpect(jsonPath("$.items[0].id").value(second));

        mockMvc.perform(get(shiftsUrl(fx, branchId, registerId))
                        .header("Authorization", bearer(token))
                        .param("status", "CLOSED"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.items.length()").value(1))
                .andExpect(jsonPath("$.items[0].id").value(first));
    }

    @Test
    void getShiftOfAnotherRegisterIs404() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        UUID branchId = createBranch(fx, "centro");
        String registerA = createRegister(fx, branchId, token, "register-1");
        String registerB = createRegister(fx, branchId, token, "register-2");
        String shiftId = objectMapper.readTree(openShift(fx, branchId, registerA, token, "{\"openingAmount\":10.0000}"))
                .get("id")
                .asText();

        mockMvc.perform(get(shiftsUrl(fx, branchId, registerB) + "/" + shiftId)
                        .header("Authorization", bearer(token)))
                .andExpect(status().isNotFound())
                .andExpect(jsonPath("$.code").value("CASH_SHIFT_NOT_FOUND"));
    }
}
