package com.hiqnet.pos.auth;

import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.get;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.patch;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.jsonPath;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import com.hiqnet.pos.users.domain.User;
import java.util.UUID;
import org.junit.jupiter.api.Test;
import org.springframework.http.MediaType;

/**
 * The most important posture check of this task: even with a VALID access
 * token, the business APIs (organizations, branches) remain closed. Auth must
 * not accidentally open the next layer before Roles/Permissions exist.
 */
class SecurityDenyIntegrationTest extends AbstractAuthIntegrationTest {

    @Test
    void businessApiWithoutTokenIsUnauthorized() throws Exception {
        mockMvc.perform(get("/api/v1/organizations/" + UUID.randomUUID()))
                .andExpect(status().isUnauthorized())
                .andExpect(jsonPath("$.code").value("AUTHENTICATION_REQUIRED"));
    }

    @Test
    void organizationApiWithValidTokenIsForbidden() throws Exception {
        String orgCode = createOrganizationAndGetCode();
        User user = createUserForOrganization(orgIdOf(orgCode), uniqueEmail("denyO@"), PASSWORD);
        String access = postLoginExpectingToken(orgCode, user.getEmail(), PASSWORD);
        String organizationId = orgIdOf(orgCode);

        mockMvc.perform(get("/api/v1/organizations/{id}", organizationId)
                        .header("Authorization", "Bearer " + access))
                .andExpect(status().isForbidden())
                .andExpect(jsonPath("$.code").value("FORBIDDEN"));

        mockMvc.perform(post("/api/v1/organizations")
                        .header("Authorization", "Bearer " + access)
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"name\":\"X\",\"code\":\"x-1\",\"timezone\":\"UTC\",\"currency\":\"MXN\",\"locale\":\"es-MX\"}"))
                .andExpect(status().isForbidden());
    }

    @Test
    void branchApiWithValidTokenIsForbidden() throws Exception {
        String orgCode = createOrganizationAndGetCode();
        User user = createUserForOrganization(orgIdOf(orgCode), uniqueEmail("denyB@"), PASSWORD);
        String access = postLoginExpectingToken(orgCode, user.getEmail(), PASSWORD);
        String organizationId = orgIdOf(orgCode);

        mockMvc.perform(get("/api/v1/organizations/{id}/branches", organizationId)
                        .header("Authorization", "Bearer " + access))
                .andExpect(status().isForbidden());
    }

    @Test
    void userAdminEndpointsRequireAuthentication() throws Exception {
        String orgId = UUID.randomUUID().toString();

        mockMvc.perform(get("/api/v1/organizations/" + orgId + "/users"))
                .andExpect(status().isUnauthorized())
                .andExpect(jsonPath("$.code").value("AUTHENTICATION_REQUIRED"));

        mockMvc.perform(post("/api/v1/organizations/" + orgId + "/users")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{}"))
                .andExpect(status().isUnauthorized())
                .andExpect(jsonPath("$.code").value("AUTHENTICATION_REQUIRED"));
    }

    @Test
    void userAdminEndpointsAreForbiddenEvenWithAValidAccessToken() throws Exception {
        String orgCode = createOrganizationAndGetCode();
        User user = createUserForOrganization(orgIdOf(orgCode), uniqueEmail("denyU@"), PASSWORD);
        String access = postLoginExpectingToken(orgCode, user.getEmail(), PASSWORD);
        String organizationId = orgIdOf(orgCode);
        String userId = user.getId().toString();

        mockMvc.perform(get("/api/v1/organizations/{id}/users", organizationId)
                        .header("Authorization", "Bearer " + access))
                .andExpect(status().isForbidden())
                .andExpect(jsonPath("$.code").value("FORBIDDEN"));

        mockMvc.perform(post("/api/v1/organizations/{id}/users", organizationId)
                        .header("Authorization", "Bearer " + access)
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"email\":\"new@example.com\",\"password\":\"correct-horse-1\"}"))
                .andExpect(status().isForbidden())
                .andExpect(jsonPath("$.code").value("FORBIDDEN"));

        mockMvc.perform(get("/api/v1/organizations/{id}/users/{userId}", organizationId, userId)
                        .header("Authorization", "Bearer " + access))
                .andExpect(status().isForbidden());

        mockMvc.perform(patch("/api/v1/organizations/{id}/users/{userId}", organizationId, userId)
                        .header("Authorization", "Bearer " + access)
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"status\":\"INACTIVE\"}"))
                .andExpect(status().isForbidden());

        // The user administration API is closed until RBAC exists, but the
        // account itself is untouched and still able to authenticate.
        mockMvc.perform(post("/api/v1/auth/login")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(loginJson(orgCode, user.getEmail(), PASSWORD)))
                .andExpect(status().isOk());
    }

    private String createOrganizationAndGetCode() {
        String code = "org-" + UUID.randomUUID().toString().substring(0, 8);
        createOrganizationWithCode(code);
        return code;
    }

    private String orgIdOf(String code) {
        return organizationRepository.findByCode(code).orElseThrow().getId().toString();
    }
}