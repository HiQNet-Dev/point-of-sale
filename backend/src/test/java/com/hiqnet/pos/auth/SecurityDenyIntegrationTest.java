package com.hiqnet.pos.auth;

import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.get;
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

    private String createOrganizationAndGetCode() {
        String code = "org-" + UUID.randomUUID().toString().substring(0, 8);
        createOrganizationWithCode(code);
        return code;
    }

    private String orgIdOf(String code) {
        return organizationRepository.findByCode(code).orElseThrow().getId().toString();
    }
}