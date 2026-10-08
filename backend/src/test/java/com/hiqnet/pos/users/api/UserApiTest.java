package com.hiqnet.pos.users.api;

import static org.assertj.core.api.Assertions.assertThat;
import static org.hamcrest.Matchers.nullValue;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.get;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.patch;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.jsonPath;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import com.hiqnet.pos.TestSecurityConfig;
import com.hiqnet.pos.TestcontainersConfiguration;
import com.hiqnet.pos.organization.api.CreateOrganizationRequest;
import com.hiqnet.pos.organization.domain.Organization;
import com.hiqnet.pos.organization.domain.OrganizationRepository;
import com.hiqnet.pos.organization.domain.OrganizationStatus;
import com.hiqnet.pos.users.domain.User;
import com.hiqnet.pos.users.domain.UserRepository;
import java.util.UUID;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.boot.test.context.SpringBootTest;
import org.springframework.boot.webmvc.test.autoconfigure.AutoConfigureMockMvc;
import org.springframework.context.annotation.Import;
import org.springframework.http.MediaType;
import org.springframework.security.crypto.password.PasswordEncoder;
import org.springframework.test.context.ActiveProfiles;
import org.springframework.test.web.servlet.MockMvc;
import tools.jackson.databind.ObjectMapper;

/**
 * User administration API, exercised through the test-only security chain
 * (TestSecurityConfig), exactly like organization/branch. The production chain
 * keeps these endpoints denied (see SecurityDenyIntegrationTest).
 */
@ActiveProfiles("test")
@Import({TestcontainersConfiguration.class, TestSecurityConfig.class})
@SpringBootTest
@AutoConfigureMockMvc
class UserApiTest {

    private static final String STRONG_PASSWORD = "Correct-horse-battery-1";

    @Autowired
    private MockMvc mockMvc;

    @Autowired
    private ObjectMapper objectMapper;

    @Autowired
    private OrganizationRepository organizationRepository;

    @Autowired
    private UserRepository userRepository;

    @Autowired
    private PasswordEncoder passwordEncoder;

    @Test
    void createUserReturns201WithoutPasswordMaterial() throws Exception {
        String orgId = createOrganization();
        mockMvc.perform(post(usersUrl(orgId))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"email\":\"cashier@example.com\",\"password\":\"" + STRONG_PASSWORD + "\"}"))
                .andExpect(status().isCreated())
                .andExpect(jsonPath("$.id").isNotEmpty())
                .andExpect(jsonPath("$.organizationId").value(orgId))
                .andExpect(jsonPath("$.email").value("cashier@example.com"))
                .andExpect(jsonPath("$.status").value("ACTIVE"))
                .andExpect(jsonPath("$.lastLoginAt").value(nullValue()))
                .andExpect(jsonPath("$.version").value(0))
                .andExpect(jsonPath("$.createdAt").isNotEmpty())
                .andExpect(jsonPath("$.password").doesNotExist())
                .andExpect(jsonPath("$.passwordHash").doesNotExist());
    }

    @Test
    void createUserNormalizesEmail() throws Exception {
        String orgId = createOrganization();
        mockMvc.perform(post(usersUrl(orgId))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"email\":\"  Case.Mix@Example.COM \",\"password\":\"" + STRONG_PASSWORD + "\"}"))
                .andExpect(status().isCreated())
                .andExpect(jsonPath("$.email").value("case.mix@example.com"));
    }

    @Test
    void createUserPersistsBcryptHashOfTheRawPassword() throws Exception {
        String orgId = createOrganization();
        String id = createUser(orgId, "hashed@example.com", STRONG_PASSWORD);

        User persisted = userRepository.findById(UUID.fromString(id)).orElseThrow();
        assertThat(persisted.getPasswordHash())
                .isNotEqualTo(STRONG_PASSWORD)
                .doesNotContain(STRONG_PASSWORD)
                .startsWith("$2");
        assertThat(passwordEncoder.matches(STRONG_PASSWORD, persisted.getPasswordHash())).isTrue();
    }

    @Test
    void createUserRejectsUnknownOrganization() throws Exception {
        mockMvc.perform(post(usersUrl(UUID.randomUUID().toString()))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"email\":\"a@example.com\",\"password\":\"" + STRONG_PASSWORD + "\"}"))
                .andExpect(status().isNotFound())
                .andExpect(jsonPath("$.code").value("ORGANIZATION_NOT_FOUND"));
    }

    @Test
    void createUserRejectsInactiveOrganization() throws Exception {
        String orgId = createOrganization();
        deactivateOrganization(orgId);

        mockMvc.perform(post(usersUrl(orgId))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"email\":\"a@example.com\",\"password\":\"" + STRONG_PASSWORD + "\"}"))
                .andExpect(status().isConflict())
                .andExpect(jsonPath("$.code").value("ORGANIZATION_INACTIVE"));
    }

    @Test
    void createUserRejectsDuplicateEmailWithinSameOrganization() throws Exception {
        String orgId = createOrganization();
        createUser(orgId, "dup@example.com", STRONG_PASSWORD);

        mockMvc.perform(post(usersUrl(orgId))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"email\":\"DUP@example.com\",\"password\":\"" + STRONG_PASSWORD + "\"}"))
                .andExpect(status().isConflict())
                .andExpect(jsonPath("$.code").value("USER_EMAIL_ALREADY_EXISTS"));
    }

    @Test
    void createUserAllowsSameEmailInDifferentOrganizations() throws Exception {
        String orgA = createOrganization();
        String orgB = createOrganization();
        createUser(orgA, "shared@example.com", STRONG_PASSWORD);

        mockMvc.perform(post(usersUrl(orgB))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"email\":\"shared@example.com\",\"password\":\"" + STRONG_PASSWORD + "\"}"))
                .andExpect(status().isCreated())
                .andExpect(jsonPath("$.organizationId").value(orgB));
    }

    @Test
    void createUserRejectsShortPassword() throws Exception {
        String orgId = createOrganization();
        mockMvc.perform(post(usersUrl(orgId))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"email\":\"short@example.com\",\"password\":\"too-short\"}"))
                .andExpect(status().isBadRequest())
                .andExpect(jsonPath("$.code").value("WEAK_PASSWORD"));
    }

    @Test
    void createUserRejectsTooLongPassword() throws Exception {
        String orgId = createOrganization();
        String tooLong = "x".repeat(129);
        mockMvc.perform(post(usersUrl(orgId))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"email\":\"long@example.com\",\"password\":\"" + tooLong + "\"}"))
                .andExpect(status().isBadRequest())
                .andExpect(jsonPath("$.code").value("WEAK_PASSWORD"));
    }

    @Test
    void createUserRejectsMalformedEmail() throws Exception {
        String orgId = createOrganization();
        mockMvc.perform(post(usersUrl(orgId))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"email\":\"not-an-email\",\"password\":\"" + STRONG_PASSWORD + "\"}"))
                .andExpect(status().isBadRequest())
                .andExpect(jsonPath("$.code").value("VALIDATION_ERROR"));
    }

    @Test
    void getUserWithinOrganizationReturns200() throws Exception {
        String orgId = createOrganization();
        String userId = createUser(orgId, "get@example.com", STRONG_PASSWORD);

        mockMvc.perform(get(usersUrl(orgId) + "/" + userId))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.id").value(userId))
                .andExpect(jsonPath("$.email").value("get@example.com"));
    }

    @Test
    void getUserOfAnotherOrganizationReturns404() throws Exception {
        String orgA = createOrganization();
        String orgB = createOrganization();
        String userB = createUser(orgB, "cross@example.com", STRONG_PASSWORD);

        mockMvc.perform(get(usersUrl(orgA) + "/" + userB))
                .andExpect(status().isNotFound())
                .andExpect(jsonPath("$.code").value("USER_NOT_FOUND"));
    }

    @Test
    void listUsersIsPaginatedAndOrderedByEmail() throws Exception {
        String orgId = createOrganization();
        createUser(orgId, "c@example.com", STRONG_PASSWORD);
        createUser(orgId, "a@example.com", STRONG_PASSWORD);
        createUser(orgId, "b@example.com", STRONG_PASSWORD);

        mockMvc.perform(get(usersUrl(orgId)))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.page").value(0))
                .andExpect(jsonPath("$.size").value(20))
                .andExpect(jsonPath("$.totalElements").value(3))
                .andExpect(jsonPath("$.totalPages").value(1))
                .andExpect(jsonPath("$.items.length()").value(3))
                .andExpect(jsonPath("$.items[0].email").value("a@example.com"))
                .andExpect(jsonPath("$.items[1].email").value("b@example.com"))
                .andExpect(jsonPath("$.items[2].email").value("c@example.com"));
    }

    @Test
    void listUsersSupportsSecondPage() throws Exception {
        String orgId = createOrganization();
        createUser(orgId, "a@example.com", STRONG_PASSWORD);
        createUser(orgId, "b@example.com", STRONG_PASSWORD);
        createUser(orgId, "c@example.com", STRONG_PASSWORD);

        mockMvc.perform(get(usersUrl(orgId)).param("page", "0").param("size", "2"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.items.length()").value(2))
                .andExpect(jsonPath("$.totalPages").value(2))
                .andExpect(jsonPath("$.items[0].email").value("a@example.com"));

        mockMvc.perform(get(usersUrl(orgId)).param("page", "1").param("size", "2"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.items.length()").value(1))
                .andExpect(jsonPath("$.items[0].email").value("c@example.com"));
    }

    @Test
    void listUsersRejectsSizeAboveMaximum() throws Exception {
        String orgId = createOrganization();
        mockMvc.perform(get(usersUrl(orgId)).param("size", "101"))
                .andExpect(status().isBadRequest())
                .andExpect(jsonPath("$.code").value("VALIDATION_ERROR"));
    }

    @Test
    void listUsersRejectsZeroSize() throws Exception {
        String orgId = createOrganization();
        mockMvc.perform(get(usersUrl(orgId)).param("size", "0"))
                .andExpect(status().isBadRequest())
                .andExpect(jsonPath("$.code").value("VALIDATION_ERROR"));
    }

    @Test
    void listUsersRejectsNegativePage() throws Exception {
        String orgId = createOrganization();
        mockMvc.perform(get(usersUrl(orgId)).param("page", "-1"))
                .andExpect(status().isBadRequest())
                .andExpect(jsonPath("$.code").value("VALIDATION_ERROR"));
    }

    @Test
    void listUsersRejectsInvalidStatusFilter() throws Exception {
        String orgId = createOrganization();
        mockMvc.perform(get(usersUrl(orgId)).param("status", "INVALID"))
                .andExpect(status().isBadRequest())
                .andExpect(jsonPath("$.code").value("VALIDATION_ERROR"));
    }

    @Test
    void listUsersFiltersByStatus() throws Exception {
        String orgId = createOrganization();
        createUser(orgId, "active1@example.com", STRONG_PASSWORD);
        createUser(orgId, "active2@example.com", STRONG_PASSWORD);
        String inactive = createUser(orgId, "inactive@example.com", STRONG_PASSWORD);
        patchUser(orgId, inactive, "{\"status\":\"INACTIVE\"}");

        mockMvc.perform(get(usersUrl(orgId)).param("status", "INACTIVE"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.totalElements").value(1))
                .andExpect(jsonPath("$.items[0].email").value("inactive@example.com"));

        mockMvc.perform(get(usersUrl(orgId)).param("status", "ACTIVE"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.totalElements").value(2));
    }

    @Test
    void listUsersOfUnknownOrganizationReturns404() throws Exception {
        mockMvc.perform(get(usersUrl(UUID.randomUUID().toString())))
                .andExpect(status().isNotFound())
                .andExpect(jsonPath("$.code").value("ORGANIZATION_NOT_FOUND"));
    }

    @Test
    void patchUserUpdatesEmailWithNormalization() throws Exception {
        String orgId = createOrganization();
        String userId = createUser(orgId, "before@example.com", STRONG_PASSWORD);

        mockMvc.perform(patch(usersUrl(orgId) + "/" + userId)
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"email\":\"  After@Example.COM \"}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.email").value("after@example.com"))
                .andExpect(jsonPath("$.version").value(1));
    }

    @Test
    void patchUserDeactivatesAndReactivates() throws Exception {
        String orgId = createOrganization();
        String userId = createUser(orgId, "lifecycle@example.com", STRONG_PASSWORD);

        mockMvc.perform(patch(usersUrl(orgId) + "/" + userId)
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"status\":\"INACTIVE\"}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.status").value("INACTIVE"));

        mockMvc.perform(patch(usersUrl(orgId) + "/" + userId)
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"status\":\"ACTIVE\"}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.status").value("ACTIVE"));
    }

    @Test
    void patchUserRejectsDuplicateEmail() throws Exception {
        String orgId = createOrganization();
        createUser(orgId, "taken@example.com", STRONG_PASSWORD);
        String userId = createUser(orgId, "mine@example.com", STRONG_PASSWORD);

        mockMvc.perform(patch(usersUrl(orgId) + "/" + userId)
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"email\":\"taken@example.com\"}"))
                .andExpect(status().isConflict())
                .andExpect(jsonPath("$.code").value("USER_EMAIL_ALREADY_EXISTS"));
    }

    @Test
    void patchUserEmptyBodyIsNoOp() throws Exception {
        String orgId = createOrganization();
        String userId = createUser(orgId, "noop@example.com", STRONG_PASSWORD);

        mockMvc.perform(patch(usersUrl(orgId) + "/" + userId)
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.email").value("noop@example.com"))
                .andExpect(jsonPath("$.status").value("ACTIVE"))
                .andExpect(jsonPath("$.version").value(0));
    }

    @Test
    void patchUserOfAnotherOrganizationReturns404() throws Exception {
        String orgA = createOrganization();
        String orgB = createOrganization();
        String userB = createUser(orgB, "crosspatch@example.com", STRONG_PASSWORD);

        mockMvc.perform(patch(usersUrl(orgA) + "/" + userB)
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"status\":\"INACTIVE\"}"))
                .andExpect(status().isNotFound())
                .andExpect(jsonPath("$.code").value("USER_NOT_FOUND"));
    }

    @Test
    void patchUserCannotMassAssignManagedFields() throws Exception {
        String orgId = createOrganization();
        String otherOrg = createOrganization();
        String userId = createUser(orgId, "mass@example.com", STRONG_PASSWORD);
        String hashBefore = userRepository.findById(UUID.fromString(userId)).orElseThrow().getPasswordHash();

        mockMvc.perform(patch(usersUrl(orgId) + "/" + userId)
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"organizationId\":\"" + otherOrg + "\",\"passwordHash\":\"injected\","
                                + "\"id\":\"" + UUID.randomUUID() + "\",\"failedLoginAttempts\":99,"
                                + "\"version\":42,\"lockedUntil\":\"2099-01-01T00:00:00Z\"}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.organizationId").value(orgId))
                .andExpect(jsonPath("$.id").value(userId))
                .andExpect(jsonPath("$.version").value(0));

        User persisted = userRepository.findById(UUID.fromString(userId)).orElseThrow();
        assertThat(persisted.getOrganizationId()).isEqualTo(UUID.fromString(orgId));
        assertThat(persisted.getPasswordHash()).isEqualTo(hashBefore);
        assertThat(persisted.getFailedLoginAttempts()).isZero();
        assertThat(persisted.getLockedUntil()).isNull();
    }

    @Test
    void patchUserRejectsMalformedEmail() throws Exception {
        String orgId = createOrganization();
        String userId = createUser(orgId, "badchange@example.com", STRONG_PASSWORD);

        mockMvc.perform(patch(usersUrl(orgId) + "/" + userId)
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"email\":\"not-an-email\"}"))
                .andExpect(status().isBadRequest())
                .andExpect(jsonPath("$.code").value("VALIDATION_ERROR"));
    }

    private String createOrganization() throws Exception {
        String code = "uorg-" + UUID.randomUUID().toString().substring(0, 8);
        String response = mockMvc.perform(post("/api/v1/organizations")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(objectMapper.writeValueAsString(
                                new CreateOrganizationRequest(
                                        "Org " + code, code, "America/Mexico_City", "MXN", "es-MX"))))
                .andExpect(status().isCreated())
                .andReturn()
                .getResponse()
                .getContentAsString();
        return objectMapper.readTree(response).get("id").asText();
    }

    private String createUser(String organizationId, String email, String password) throws Exception {
        String response = mockMvc.perform(post(usersUrl(organizationId))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(objectMapper.writeValueAsString(new CreateUserRequest(email, password))))
                .andExpect(status().isCreated())
                .andReturn()
                .getResponse()
                .getContentAsString();
        return objectMapper.readTree(response).get("id").asText();
    }

    private void patchUser(String organizationId, String userId, String body) throws Exception {
        mockMvc.perform(patch(usersUrl(organizationId) + "/" + userId)
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(body))
                .andExpect(status().isOk());
    }

    private void deactivateOrganization(String organizationId) {
        Organization organization =
                organizationRepository.findById(UUID.fromString(organizationId)).orElseThrow();
        organization.updateStatus(OrganizationStatus.INACTIVE);
        organizationRepository.saveAndFlush(organization);
    }

    private static String usersUrl(String organizationId) {
        return "/api/v1/organizations/" + organizationId + "/users";
    }
}
