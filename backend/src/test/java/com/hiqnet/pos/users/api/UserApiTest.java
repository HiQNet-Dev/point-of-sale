package com.hiqnet.pos.users.api;

import static org.assertj.core.api.Assertions.assertThat;
import static org.hamcrest.Matchers.nullValue;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.get;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.patch;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.jsonPath;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import com.hiqnet.pos.AbstractRbacIntegrationTest;
import com.hiqnet.pos.organization.domain.Organization;
import com.hiqnet.pos.organization.domain.OrganizationStatus;
import com.hiqnet.pos.users.domain.User;
import java.util.UUID;
import org.junit.jupiter.api.Test;
import org.springframework.http.MediaType;

/**
 * User administration under the REAL security chain + RBAC (user.create /
 * user.read / user.update) and expectedVersion on PATCH. Legal behavior
 * (409 duplicate, 404 cross-org) is tested with an actor that holds the
 * permission; the authorization boundary itself returns 403 before the service
 * when the actor lacks permission or the organization is not operational.
 */
class UserApiTest extends AbstractRbacIntegrationTest {

    private static final String STRONG_PASSWORD = "Correct-horse-battery-1";

    @Test
    void createUserReturns201WithoutPasswordMaterial() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        mockMvc.perform(post(usersUrl(fx.orgId()))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"email\":\"cashier@example.com\",\"password\":\"" + STRONG_PASSWORD + "\"}"))
                .andExpect(status().isCreated())
                .andExpect(jsonPath("$.id").isNotEmpty())
                .andExpect(jsonPath("$.organizationId").value(fx.orgId().toString()))
                .andExpect(jsonPath("$.email").value("cashier@example.com"))
                .andExpect(jsonPath("$.status").value("ACTIVE"))
                .andExpect(jsonPath("$.lastLoginAt").value(nullValue()))
                .andExpect(jsonPath("$.version").value(0))
                .andExpect(jsonPath("$.createdAt").isNotEmpty())
                .andExpect(jsonPath("$.password").doesNotExist())
                .andExpect(jsonPath("$.passwordHash").doesNotExist());
    }

    @Test
    void createUserWithoutPermissionIsForbidden() throws Exception {
        RbacFixture fx = fixture();
        String token = login(fx);
        mockMvc.perform(post(usersUrl(fx.orgId()))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"email\":\"cashier@example.com\",\"password\":\"" + STRONG_PASSWORD + "\"}"))
                .andExpect(status().isForbidden())
                .andExpect(jsonPath("$.code").value("FORBIDDEN"));
    }

    @Test
    void createUserNormalizesEmail() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        mockMvc.perform(post(usersUrl(fx.orgId()))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"email\":\"  Case.Mix@Example.COM \",\"password\":\"" + STRONG_PASSWORD + "\"}"))
                .andExpect(status().isCreated())
                .andExpect(jsonPath("$.email").value("case.mix@example.com"));
    }

    @Test
    void createUserPersistsBcryptHashOfTheRawPassword() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String id = createUser(fx.orgId(), token, "hashed@example.com", STRONG_PASSWORD);

        User persisted = userRepository.findById(UUID.fromString(id)).orElseThrow();
        assertThat(persisted.getPasswordHash())
                .isNotEqualTo(STRONG_PASSWORD)
                .doesNotContain(STRONG_PASSWORD)
                .startsWith("$2");
        assertThat(passwordEncoder.matches(STRONG_PASSWORD, persisted.getPasswordHash())).isTrue();
    }

    @Test
    void createUserOutsideReachableOrganizationIsForbidden() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        mockMvc.perform(post(usersUrl(UUID.randomUUID()))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"email\":\"a@example.com\",\"password\":\"" + STRONG_PASSWORD + "\"}"))
                .andExpect(status().isForbidden())
                .andExpect(jsonPath("$.code").value("FORBIDDEN"));
    }

    @Test
    void createUserInInactiveOrganizationIsForbidden() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        deactivateOrganization(fx.orgId());

        // The authorization query refuses INACTIVE organizations: the request
        // never reaches the service (403), so no session/user is created.
        mockMvc.perform(post(usersUrl(fx.orgId()))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"email\":\"a@example.com\",\"password\":\"" + STRONG_PASSWORD + "\"}"))
                .andExpect(status().isForbidden())
                .andExpect(jsonPath("$.code").value("FORBIDDEN"));
        // No user was created: only the fixture user exists in this organization.
        assertThat(userRepository
                        .findByOrganizationId(fx.orgId(), org.springframework.data.domain.PageRequest.of(0, 100))
                        .getTotalElements())
                .isEqualTo(1);
    }

    @Test
    void createUserRejectsDuplicateEmailWithinSameOrganization() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        createUser(fx.orgId(), token, "dup@example.com", STRONG_PASSWORD);

        mockMvc.perform(post(usersUrl(fx.orgId()))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"email\":\"DUP@example.com\",\"password\":\"" + STRONG_PASSWORD + "\"}"))
                .andExpect(status().isConflict())
                .andExpect(jsonPath("$.code").value("USER_EMAIL_ALREADY_EXISTS"));
    }

    @Test
    void createUserAllowsSameEmailInDifferentOrganizations() throws Exception {
        RbacFixture fxA = fixture();
        RbacFixture fxB = fixture();
        String tokenA = adminToken(fxA);
        String tokenB = adminToken(fxB);
        createUser(fxA.orgId(), tokenA, "shared@example.com", STRONG_PASSWORD);

        mockMvc.perform(post(usersUrl(fxB.orgId()))
                        .header("Authorization", bearer(tokenB))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"email\":\"shared@example.com\",\"password\":\"" + STRONG_PASSWORD + "\"}"))
                .andExpect(status().isCreated())
                .andExpect(jsonPath("$.organizationId").value(fxB.orgId().toString()));
    }

    @Test
    void createUserRejectsShortPassword() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        mockMvc.perform(post(usersUrl(fx.orgId()))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"email\":\"short@example.com\",\"password\":\"too-short\"}"))
                .andExpect(status().isBadRequest())
                .andExpect(jsonPath("$.code").value("WEAK_PASSWORD"));
    }

    @Test
    void createUserRejectsTooLongPassword() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String tooLong = "x".repeat(129);
        mockMvc.perform(post(usersUrl(fx.orgId()))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"email\":\"long@example.com\",\"password\":\"" + tooLong + "\"}"))
                .andExpect(status().isBadRequest())
                .andExpect(jsonPath("$.code").value("WEAK_PASSWORD"));
    }

    @Test
    void createUserRejectsMalformedEmail() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        mockMvc.perform(post(usersUrl(fx.orgId()))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"email\":\"not-an-email\",\"password\":\"" + STRONG_PASSWORD + "\"}"))
                .andExpect(status().isBadRequest())
                .andExpect(jsonPath("$.code").value("VALIDATION_ERROR"));
    }

    @Test
    void getUserWithinOrganizationReturns200() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String userId = createUser(fx.orgId(), token, "get@example.com", STRONG_PASSWORD);

        mockMvc.perform(get(usersUrl(fx.orgId()) + "/" + userId).header("Authorization", bearer(token)))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.id").value(userId))
                .andExpect(jsonPath("$.email").value("get@example.com"));
    }

    @Test
    void getUserOfAnotherOrganizationReturns404() throws Exception {
        RbacFixture fxA = fixture();
        RbacFixture fxB = fixture();
        String tokenA = adminToken(fxA);
        String tokenB = adminToken(fxB);
        String userB = createUser(fxB.orgId(), tokenB, "cross@example.com", STRONG_PASSWORD);

        mockMvc.perform(get(usersUrl(fxA.orgId()) + "/" + userB).header("Authorization", bearer(tokenA)))
                .andExpect(status().isNotFound())
                .andExpect(jsonPath("$.code").value("USER_NOT_FOUND"));
    }

    @Test
    void listUsersIsPaginatedAndOrderedByEmail() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        createUser(fx.orgId(), token, "c@example.com", STRONG_PASSWORD);
        createUser(fx.orgId(), token, "a@example.com", STRONG_PASSWORD);
        createUser(fx.orgId(), token, "b@example.com", STRONG_PASSWORD);

        mockMvc.perform(get(usersUrl(fx.orgId())).header("Authorization", bearer(token)))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.page").value(0))
                .andExpect(jsonPath("$.size").value(20))
                .andExpect(jsonPath("$.totalElements").value(4))
                .andExpect(jsonPath("$.totalPages").value(1))
                .andExpect(jsonPath("$.items[0].email").value("a@example.com"))
                .andExpect(jsonPath("$.items[1].email").value("b@example.com"))
                .andExpect(jsonPath("$.items[2].email").value("c@example.com"));
    }

    @Test
    void listUsersSupportsSecondPage() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        createUser(fx.orgId(), token, "a@example.com", STRONG_PASSWORD);
        createUser(fx.orgId(), token, "b@example.com", STRONG_PASSWORD);
        createUser(fx.orgId(), token, "c@example.com", STRONG_PASSWORD);

        mockMvc.perform(get(usersUrl(fx.orgId())).param("page", "0").param("size", "2")
                        .header("Authorization", bearer(token)))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.items.length()").value(2))
                .andExpect(jsonPath("$.totalPages").value(2))
                .andExpect(jsonPath("$.totalElements").value(4)); // actor + 3 created

        mockMvc.perform(get(usersUrl(fx.orgId())).param("page", "1").param("size", "2")
                        .header("Authorization", bearer(token)))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.items.length()").value(2))
                .andExpect(jsonPath("$.totalElements").value(4));
    }

    @Test
    void listUsersRejectsSizeAboveMaximum() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        mockMvc.perform(get(usersUrl(fx.orgId())).param("size", "101").header("Authorization", bearer(token)))
                .andExpect(status().isBadRequest())
                .andExpect(jsonPath("$.code").value("VALIDATION_ERROR"));
    }

    @Test
    void listUsersRejectsZeroSize() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        mockMvc.perform(get(usersUrl(fx.orgId())).param("size", "0").header("Authorization", bearer(token)))
                .andExpect(status().isBadRequest())
                .andExpect(jsonPath("$.code").value("VALIDATION_ERROR"));
    }

    @Test
    void listUsersRejectsNegativePage() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        mockMvc.perform(get(usersUrl(fx.orgId())).param("page", "-1").header("Authorization", bearer(token)))
                .andExpect(status().isBadRequest())
                .andExpect(jsonPath("$.code").value("VALIDATION_ERROR"));
    }

    @Test
    void listUsersRejectsInvalidStatusFilter() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        mockMvc.perform(get(usersUrl(fx.orgId())).param("status", "INVALID").header("Authorization", bearer(token)))
                .andExpect(status().isBadRequest())
                .andExpect(jsonPath("$.code").value("VALIDATION_ERROR"));
    }

    @Test
    void listUsersFiltersByStatus() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        createUser(fx.orgId(), token, "active1@example.com", STRONG_PASSWORD);
        createUser(fx.orgId(), token, "active2@example.com", STRONG_PASSWORD);
        String inactive = createUser(fx.orgId(), token, "inactive@example.com", STRONG_PASSWORD);
        patchUser(fx.orgId(), inactive, token, "{\"expectedVersion\":0,\"status\":\"INACTIVE\"}");

        mockMvc.perform(get(usersUrl(fx.orgId())).param("status", "INACTIVE").header("Authorization", bearer(token)))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.totalElements").value(1))
                .andExpect(jsonPath("$.items[0].email").value("inactive@example.com"));
    }

    @Test
    void listUsersOutsideReachableOrganizationIsForbidden() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        mockMvc.perform(get(usersUrl(UUID.randomUUID())).header("Authorization", bearer(token)))
                .andExpect(status().isForbidden())
                .andExpect(jsonPath("$.code").value("FORBIDDEN"));
    }

    @Test
    void patchUserUpdatesEmailWithNormalization() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String userId = createUser(fx.orgId(), token, "before@example.com", STRONG_PASSWORD);

        mockMvc.perform(patch(usersUrl(fx.orgId()) + "/" + userId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"email\":\"  After@Example.COM \"}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.email").value("after@example.com"))
                .andExpect(jsonPath("$.version").value(1));
    }

    @Test
    void patchUserDeactivatesAndReactivates() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String userId = createUser(fx.orgId(), token, "lifecycle@example.com", STRONG_PASSWORD);

        mockMvc.perform(patch(usersUrl(fx.orgId()) + "/" + userId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"status\":\"INACTIVE\"}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.status").value("INACTIVE"))
                .andExpect(jsonPath("$.version").value(1));

        mockMvc.perform(patch(usersUrl(fx.orgId()) + "/" + userId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":1,\"status\":\"ACTIVE\"}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.status").value("ACTIVE"));
    }

    @Test
    void patchUserRejectsDuplicateEmail() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        createUser(fx.orgId(), token, "taken@example.com", STRONG_PASSWORD);
        String userId = createUser(fx.orgId(), token, "mine@example.com", STRONG_PASSWORD);

        mockMvc.perform(patch(usersUrl(fx.orgId()) + "/" + userId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"email\":\"taken@example.com\"}"))
                .andExpect(status().isConflict())
                .andExpect(jsonPath("$.code").value("USER_EMAIL_ALREADY_EXISTS"));
    }

    @Test
    void patchUserEmptyBodyIsNoOp() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String userId = createUser(fx.orgId(), token, "noop@example.com", STRONG_PASSWORD);

        mockMvc.perform(patch(usersUrl(fx.orgId()) + "/" + userId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.email").value("noop@example.com"))
                .andExpect(jsonPath("$.status").value("ACTIVE"))
                .andExpect(jsonPath("$.version").value(0));
    }

    @Test
    void patchUserOfAnotherOrganizationReturns404() throws Exception {
        RbacFixture fxA = fixture();
        RbacFixture fxB = fixture();
        String tokenA = adminToken(fxA);
        String tokenB = adminToken(fxB);
        String userB = createUser(fxB.orgId(), tokenB, "crosspatch@example.com", STRONG_PASSWORD);

        mockMvc.perform(patch(usersUrl(fxA.orgId()) + "/" + userB)
                        .header("Authorization", bearer(tokenA))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"status\":\"INACTIVE\"}"))
                .andExpect(status().isNotFound())
                .andExpect(jsonPath("$.code").value("USER_NOT_FOUND"));
    }

    @Test
    void patchUserCannotMassAssignManagedFields() throws Exception {
        RbacFixture fx = fixture();
        UUID otherOrg = fixture().orgId();
        String token = adminToken(fx);
        String userId = createUser(fx.orgId(), token, "mass@example.com", STRONG_PASSWORD);
        String hashBefore = userRepository.findById(UUID.fromString(userId)).orElseThrow().getPasswordHash();

        mockMvc.perform(patch(usersUrl(fx.orgId()) + "/" + userId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"organizationId\":\"" + otherOrg + "\","
                                + "\"passwordHash\":\"injected\",\"id\":\"" + UUID.randomUUID() + "\","
                                + "\"failedLoginAttempts\":99,\"version\":42,"
                                + "\"lockedUntil\":\"2099-01-01T00:00:00Z\"}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.organizationId").value(fx.orgId().toString()))
                .andExpect(jsonPath("$.id").value(userId))
                .andExpect(jsonPath("$.version").value(0));

        User persisted = userRepository.findById(UUID.fromString(userId)).orElseThrow();
        assertThat(persisted.getOrganizationId()).isEqualTo(fx.orgId());
        assertThat(persisted.getPasswordHash()).isEqualTo(hashBefore);
        assertThat(persisted.getFailedLoginAttempts()).isZero();
        assertThat(persisted.getLockedUntil()).isNull();
    }

    @Test
    void patchUserRejectsMalformedEmail() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String userId = createUser(fx.orgId(), token, "badchange@example.com", STRONG_PASSWORD);

        mockMvc.perform(patch(usersUrl(fx.orgId()) + "/" + userId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"email\":\"not-an-email\"}"))
                .andExpect(status().isBadRequest())
                .andExpect(jsonPath("$.code").value("VALIDATION_ERROR"));
    }

    @Test
    void patchUserStaleExpectedVersionIs409() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String userId = createUser(fx.orgId(), token, "stale@example.com", STRONG_PASSWORD);

        mockMvc.perform(patch(usersUrl(fx.orgId()) + "/" + userId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"email\":\"new@example.com\"}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.version").value(1));

        mockMvc.perform(patch(usersUrl(fx.orgId()) + "/" + userId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"email\":\"stale@example.com\"}"))
                .andExpect(status().isConflict())
                .andExpect(jsonPath("$.code").value("CONCURRENT_MODIFICATION"));
    }

    private String createUser(UUID organizationId, String token, String email, String password) throws Exception {
        String response = mockMvc.perform(post(usersUrl(organizationId))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(objectMapper.writeValueAsString(new CreateUserRequest(email, password))))
                .andExpect(status().isCreated())
                .andReturn()
                .getResponse()
                .getContentAsString();
        return objectMapper.readTree(response).get("id").asText();
    }

    private void patchUser(UUID organizationId, String userId, String token, String body) throws Exception {
        mockMvc.perform(patch(usersUrl(organizationId) + "/" + userId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(body))
                .andExpect(status().isOk());
    }

    private void deactivateOrganization(UUID organizationId) {
        Organization organization = organizationRepository.findById(organizationId).orElseThrow();
        organization.updateStatus(OrganizationStatus.INACTIVE);
        organizationRepository.saveAndFlush(organization);
    }

    private static String usersUrl(UUID organizationId) {
        return "/api/v1/organizations/" + organizationId + "/users";
    }
}