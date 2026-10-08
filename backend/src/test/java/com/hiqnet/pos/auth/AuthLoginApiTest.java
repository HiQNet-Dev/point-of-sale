package com.hiqnet.pos.auth;

import static org.assertj.core.api.Assertions.assertThat;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.jsonPath;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import com.hiqnet.pos.users.domain.User;
import java.time.Instant;
import java.util.Locale;
import java.util.UUID;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.http.MediaType;
import org.springframework.security.oauth2.jwt.Jwt;
import org.springframework.security.oauth2.jwt.JwtDecoder;

/**
 * Login security: generic credential errors, normalization, failed-attempt
 * accounting, temporal lockout and counter reset on success. Uses the real
 * security chain (public login endpoint).
 */
class AuthLoginApiTest extends AbstractAuthIntegrationTest {

    @Autowired
    private JwtDecoder jwtDecoder;

    @Test
    void loginSuccessReturnsTokensAndValidAccessJwt() throws Exception {
        String orgCode = createOrganizationAndGetCode();
        User user = createUserForOrganization(orgIdOf(orgCode), uniqueEmail("ok@"), PASSWORD);

        String response = mockMvc.perform(post("/api/v1/auth/login")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(loginJson(orgCode, emailOf(user), PASSWORD)))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.accessToken").isNotEmpty())
                .andExpect(jsonPath("$.tokenType").value("Bearer"))
                .andExpect(jsonPath("$.expiresIn").value(600))
                .andExpect(jsonPath("$.refreshToken").isNotEmpty())
                .andReturn()
                .getResponse()
                .getContentAsString();

        String accessToken = objectMapper.readTree(response).get("accessToken").asText();
        Jwt jwt = jwtDecoder.decode(accessToken);
        assertThat(jwt.getSubject()).isEqualTo(user.getId().toString());
        assertThat(jwt.getClaimAsString("org_id")).isEqualTo(user.getOrganizationId().toString());
        assertThat(jwt.getClaimAsString("token_type")).isEqualTo("access");
        assertThat(jwt.getClaimAsString("iss")).isEqualTo("hiqnet-pos");
        assertThat(jwt.getAudience()).contains("hiqnet-pos-api");
        assertThat(jwt.getClaimAsInstant("exp")).isAfter(Instant.now());
    }

    @Test
    void loginWrongOrganizationIsGeneric() throws Exception {
        String email = uniqueEmail("worg@");
        createActiveUser(email, PASSWORD);

        mockMvc.perform(post("/api/v1/auth/login")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(loginJson("org-does-not-exist", email, PASSWORD)))
                .andExpect(status().isUnauthorized())
                .andExpect(jsonPath("$.code").value("INVALID_CREDENTIALS"))
                .andExpect(jsonPath("$.message")
                        .value(org.hamcrest.Matchers.not(
                                org.hamcrest.Matchers.containsString("organization"))))
                .andExpect(jsonPath("$.message")
                        .value(org.hamcrest.Matchers.not(
                                org.hamcrest.Matchers.containsString("email"))));
    }

    @Test
    void loginUnknownEmailIsGeneric() throws Exception {
        String orgCode = createOrganizationAndGetCode();
        createUserForOrganization(orgIdOf(orgCode), uniqueEmail("know@"), PASSWORD);

        mockMvc.perform(post("/api/v1/auth/login")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(loginJson(orgCode, uniqueEmail("ghost@"), PASSWORD)))
                .andExpect(status().isUnauthorized())
                .andExpect(jsonPath("$.code").value("INVALID_CREDENTIALS"));
    }

    @Test
    void loginWrongPasswordIsGeneric() throws Exception {
        String orgCode = createOrganizationAndGetCode();
        User user = createUserForOrganization(orgIdOf(orgCode), uniqueEmail("pwd@"), PASSWORD);

        mockMvc.perform(post("/api/v1/auth/login")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(loginJson(orgCode, emailOf(user), "wrong-password")))
                .andExpect(status().isUnauthorized())
                .andExpect(jsonPath("$.code").value("INVALID_CREDENTIALS"));
    }

    @Test
    void loginInactiveUserIsGeneric() throws Exception {
        String orgCode = createOrganizationAndGetCode();
        User user = createUserForOrganization(orgIdOf(orgCode), uniqueEmail("in@"), PASSWORD);
        user.deactivate();
        userRepository.saveAndFlush(user);

        mockMvc.perform(post("/api/v1/auth/login")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(loginJson(orgCode, emailOf(user), PASSWORD)))
                .andExpect(status().isUnauthorized())
                .andExpect(jsonPath("$.code").value("INVALID_CREDENTIALS"));
    }

    @Test
    void loginNormalizesEmailCasingAndWhitespace() throws Exception {
        String orgCode = createOrganizationAndGetCode();
        User user = createUserForOrganization(orgIdOf(orgCode), "  Case.Mix@Example.COM  ", PASSWORD);

        mockMvc.perform(post("/api/v1/auth/login")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(loginJson(orgCode, "  CASE.MIX@example.com ", PASSWORD)))
                .andExpect(status().isOk());
    }

    @Test
    void failedAttemptsIncrementPersistently() throws Exception {
        String orgCode = createOrganizationAndGetCode();
        User user = createUserForOrganization(orgIdOf(orgCode), uniqueEmail("cnt@"), PASSWORD);

        mockMvc.perform(post("/api/v1/auth/login")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(loginJson(orgCode, emailOf(user), "bad-1")));
        mockMvc.perform(post("/api/v1/auth/login")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(loginJson(orgCode, emailOf(user), "bad-2")));

        assertThat(userRepository.findById(user.getId()).orElseThrow().getFailedLoginAttempts())
                .isEqualTo(2);
    }

    @Test
    void lockoutActivatesAfterMaxFailedAttempts() throws Exception {
        String orgCode = createOrganizationAndGetCode();
        User user = createUserForOrganization(orgIdOf(orgCode), uniqueEmail("lock@"), PASSWORD);

        for (int i = 0; i < 5; i++) {
            mockMvc.perform(post("/api/v1/auth/login")
                            .contentType(MediaType.APPLICATION_JSON)
                            .content(loginJson(orgCode, emailOf(user), "bad-" + i)))
                    .andExpect(status().isUnauthorized());
        }

        User persisted = userRepository.findById(user.getId()).orElseThrow();
        assertThat(persisted.getLockedUntil()).isNotNull().isAfter(Instant.now());

        // Even the correct password is rejected while locked.
        mockMvc.perform(post("/api/v1/auth/login")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(loginJson(orgCode, emailOf(user), PASSWORD)))
                .andExpect(status().isUnauthorized())
                .andExpect(jsonPath("$.code").value("INVALID_CREDENTIALS"));
    }

    @Test
    void successfulLoginResetsFailureStateAndUpdatesLastLogin() throws Exception {
        String orgCode = createOrganizationAndGetCode();
        User user = createUserForOrganization(orgIdOf(orgCode), uniqueEmail("reset@"), PASSWORD);

        mockMvc.perform(post("/api/v1/auth/login")
                .contentType(MediaType.APPLICATION_JSON)
                .content(loginJson(orgCode, emailOf(user), "bad-1")));
        mockMvc.perform(post("/api/v1/auth/login")
                .contentType(MediaType.APPLICATION_JSON)
                .content(loginJson(orgCode, emailOf(user), "bad-2")));

        mockMvc.perform(post("/api/v1/auth/login")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(loginJson(orgCode, emailOf(user), PASSWORD)))
                .andExpect(status().isOk());

        User persisted = userRepository.findById(user.getId()).orElseThrow();
        assertThat(persisted.getFailedLoginAttempts()).isZero();
        assertThat(persisted.getLockedUntil()).isNull();
        assertThat(persisted.getLastLoginAt()).isNotNull();
    }

    @Test
    void errorResponsesNeverLeakCredentialDetails() throws Exception {
        String orgCode = createOrganizationAndGetCode();
        createUserForOrganization(orgIdOf(orgCode), uniqueEmail("leak@"), PASSWORD);

        mockMvc.perform(post("/api/v1/auth/login")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(loginJson(orgCode, uniqueEmail("nobody@"), "whatever")))
                .andExpect(status().isUnauthorized())
                .andExpect(jsonPath("$.password").doesNotExist())
                .andExpect(jsonPath("$.details").isArray())
                .andExpect(jsonPath("$.message").isNotEmpty());
    }

    @Test
    void malformedEmailIsGenericCredentialErrorEvenForUnknownOrg() throws Exception {
        String unknownOrg = "org-" + UUID.randomUUID().toString().substring(0, 8);

        mockMvc.perform(post("/api/v1/auth/login")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(loginJson(unknownOrg, "not-an-email", PASSWORD)))
                .andExpect(status().isUnauthorized())
                .andExpect(jsonPath("$.code").value("INVALID_CREDENTIALS"))
                .andExpect(jsonPath("$.code")
                        .value(org.hamcrest.Matchers.not(
                                org.hamcrest.Matchers.is("VALIDATION_ERROR"))));
    }

    @Test
    void loginNormalizesOrganizationCodeCasingAndWhitespace() throws Exception {
        String orgCode = createOrganizationAndGetCode();
        User user = createUserForOrganization(orgIdOf(orgCode), uniqueEmail("cse@"), PASSWORD);

        mockMvc.perform(post("/api/v1/auth/login")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("""
                                {"organizationCode":"%s ","email":"%s","password":"%s"}
                                """.formatted(orgCode.toUpperCase(Locale.ROOT), emailOf(user), PASSWORD)))
                .andExpect(status().isOk());
    }

    private String createOrganizationAndGetCode() {
        return "org-" + UUID.randomUUID().toString().substring(0, 8);
    }

    private String orgIdOf(String code) {
        return createOrganizationWithCode(code);
    }

    private String emailOf(User user) {
        return user.getEmail();
    }
}