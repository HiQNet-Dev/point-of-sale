package com.hiqnet.pos.auth;

import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.get;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.jsonPath;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import com.hiqnet.pos.organization.domain.Organization;
import com.hiqnet.pos.users.application.UpdateUserCommand;
import com.hiqnet.pos.users.application.UserService;
import com.hiqnet.pos.users.domain.User;
import com.hiqnet.pos.users.domain.UserStatus;
import java.nio.charset.StandardCharsets;
import java.util.UUID;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.http.MediaType;

/**
 * Cross-module contract Users -> Auth (one direction only): deactivating a user
 * through the users module must immediately block login/refresh/me, and
 * reactivation restores login. The users module never calls auth; auth simply
 * reacts to the persisted {@code User.status}, so there is no module cycle.
 */
class UserAuthIntegrationTest extends AbstractAuthIntegrationTest {

    @Autowired
    private UserService userService;

    @Test
    void deactivationBlocksAuthAndReactivationRestoresLogin() throws Exception {
        String code = "uorg-" + UUID.randomUUID().toString().substring(0, 8);
        Organization organization =
                Organization.create("Org " + code, code, "America/Mexico_City", "MXN", "es-MX");
        organizationRepository.saveAndFlush(organization);
        UUID orgId = organization.getId();
        String email = uniqueEmail("lifecycle@");

        User user = userService.create(orgId, email, PASSWORD);

        String loginResponse = mockMvc.perform(post("/api/v1/auth/login")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(loginJson(code, email, PASSWORD)))
                .andExpect(status().isOk())
                .andReturn()
                .getResponse()
                .getContentAsString(StandardCharsets.UTF_8);
        String access = objectMapper.readTree(loginResponse).get("accessToken").asText();
        String refresh = objectMapper.readTree(loginResponse).get("refreshToken").asText();

        userService.update(orgId, user.getId(), new UpdateUserCommand(null, UserStatus.INACTIVE));

        mockMvc.perform(post("/api/v1/auth/login")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(loginJson(code, email, PASSWORD)))
                .andExpect(status().isUnauthorized())
                .andExpect(jsonPath("$.code").value("INVALID_CREDENTIALS"));

        mockMvc.perform(post("/api/v1/auth/refresh")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(refreshBody(refresh)))
                .andExpect(status().isUnauthorized())
                .andExpect(jsonPath("$.code").value("INVALID_REFRESH_TOKEN"));

        mockMvc.perform(get("/api/v1/auth/me").header("Authorization", "Bearer " + access))
                .andExpect(status().isUnauthorized())
                .andExpect(jsonPath("$.code").value("TOKEN_INVALID"));

        userService.update(orgId, user.getId(), new UpdateUserCommand(null, UserStatus.ACTIVE));

        mockMvc.perform(post("/api/v1/auth/login")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(loginJson(code, email, PASSWORD)))
                .andExpect(status().isOk());

        // Revoked/consumed refresh sessions never come back to life.
        mockMvc.perform(post("/api/v1/auth/refresh")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(refreshBody(refresh)))
                .andExpect(status().isUnauthorized());
    }

    private static String refreshBody(String refreshToken) {
        return "{\"refreshToken\":\"" + refreshToken + "\"}";
    }
}
