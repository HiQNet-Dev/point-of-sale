package com.hiqnet.pos.auth;

import static org.assertj.core.api.Assertions.assertThat;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.get;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.jsonPath;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import com.hiqnet.pos.auth.domain.RefreshSession;
import com.hiqnet.pos.auth.domain.RefreshSessionRepository;
import com.hiqnet.pos.auth.infrastructure.RefreshTokenHasher;
import com.hiqnet.pos.users.domain.User;
import java.time.Duration;
import java.time.Instant;
import java.util.UUID;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.http.MediaType;

/**
 * Refresh rotation, reuse detection, revocation, logout semantics and /me,
 * all against the real security chain.
 */
class AuthSessionApiTest extends AbstractAuthIntegrationTest {

    @Autowired
    private RefreshSessionRepository refreshSessionRepository;

    @Test
    void refreshRotatesTokenAtoB() throws Exception {
        String orgCode = createOrganizationAndGetCode();
        User user = createUserForOrganization(orgIdOf(orgCode), uniqueEmail("rot@"), PASSWORD);
        String tokenA = loginAndGetRefreshToken(orgCode, user.getEmail(), PASSWORD);

        String tokenB = refreshRawToken(tokenA);

        assertThat(tokenB).isNotEqualTo(tokenA).isNotEmpty();
    }

    @Test
    void usedTokenAIsRejectedAfterRotation() throws Exception {
        String orgCode = createOrganizationAndGetCode();
        User user = createUserForOrganization(orgIdOf(orgCode), uniqueEmail("used@"), PASSWORD);
        String tokenA = loginAndGetRefreshToken(orgCode, user.getEmail(), PASSWORD);
        refreshRawToken(tokenA);

        mockMvc.perform(post("/api/v1/auth/refresh")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(refreshBody(tokenA)))
                .andExpect(status().isUnauthorized())
                .andExpect(jsonPath("$.code").value("INVALID_REFRESH_TOKEN"));
    }

    @Test
    void reuseOfConsumedTokenRevokesTheWholeFamily() throws Exception {
        String orgCode = createOrganizationAndGetCode();
        User user = createUserForOrganization(orgIdOf(orgCode), uniqueEmail("reuse@"), PASSWORD);
        String tokenA = loginAndGetRefreshToken(orgCode, user.getEmail(), PASSWORD);
        String tokenB = refreshRawToken(tokenA);

        // Reuse attack: A is presented again -> family must die.
        mockMvc.perform(post("/api/v1/auth/refresh")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(refreshBody(tokenA)))
                .andExpect(status().isUnauthorized())
                .andExpect(jsonPath("$.code").value("INVALID_REFRESH_TOKEN"));

        // B (legitimately derived from A) is now revoked too.
        mockMvc.perform(post("/api/v1/auth/refresh")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(refreshBody(tokenB)))
                .andExpect(status().isUnauthorized())
                .andExpect(jsonPath("$.code").value("INVALID_REFRESH_TOKEN"));
    }

    @Test
    void expiredRefreshSessionIsRejected() throws Exception {
        String orgCode = createOrganizationAndGetCode();
        User user = createUserForOrganization(orgIdOf(orgCode), uniqueEmail("exp@"), PASSWORD);
        String raw = RefreshTokenHasher.generateRawToken();
        RefreshSession expired = RefreshSession.create(
                user.getId(),
                RefreshTokenHasher.hash(raw),
                Instant.now().minus(Duration.ofDays(2)),
                Duration.ofHours(1),
                Duration.ofDays(30));
        refreshSessionRepository.saveAndFlush(expired);

        mockMvc.perform(post("/api/v1/auth/refresh")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(refreshBody(raw)))
                .andExpect(status().isUnauthorized())
                .andExpect(jsonPath("$.code").value("INVALID_REFRESH_TOKEN"));
    }

    @Test
    void revokedRefreshSessionIsRejected() throws Exception {
        String orgCode = createOrganizationAndGetCode();
        User user = createUserForOrganization(orgIdOf(orgCode), uniqueEmail("rev@"), PASSWORD);
        String raw = RefreshTokenHasher.generateRawToken();
        RefreshSession session = RefreshSession.create(
                user.getId(),
                RefreshTokenHasher.hash(raw),
                Instant.now(),
                Duration.ofDays(30),
                Duration.ofDays(30));
        session.revoke(Instant.now());
        refreshSessionRepository.saveAndFlush(session);

        mockMvc.perform(post("/api/v1/auth/refresh")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(refreshBody(raw)))
                .andExpect(status().isUnauthorized());
    }

    @Test
    void unknownRefreshTokenIsRejected() throws Exception {
        mockMvc.perform(post("/api/v1/auth/refresh")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(refreshBody(RefreshTokenHasher.generateRawToken())))
                .andExpect(status().isUnauthorized())
                .andExpect(jsonPath("$.code").value("INVALID_REFRESH_TOKEN"));
    }

    @Test
    void rawRefreshTokenIsNotPersistedOnlyItsHash() throws Exception {
        String orgCode = createOrganizationAndGetCode();
        User user = createUserForOrganization(orgIdOf(orgCode), uniqueEmail("hsh@"), PASSWORD);
        String raw = "0192-MB6Y-token-reuse-0-raw-value-1234567890xyz";
        String hash = RefreshTokenHasher.hash(raw);
        refreshSessionRepository.saveAndFlush(RefreshSession.create(
                user.getId(), hash, Instant.now(), Duration.ofDays(30), Duration.ofDays(30)));

        assertThat(refreshSessionRepository.findByTokenHash(raw)).isEmpty();
        assertThat(refreshSessionRepository.findByTokenHash(hash)).isPresent();
    }

    @Test
    void logoutRevokesTheCurrentRefreshSession() throws Exception {
        String orgCode = createOrganizationAndGetCode();
        User user = createUserForOrganization(orgIdOf(orgCode), uniqueEmail("out@"), PASSWORD);
        String refresh = loginAndGetRefreshToken(orgCode, user.getEmail(), PASSWORD);

        mockMvc.perform(post("/api/v1/auth/logout")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(refreshBody(refresh)))
                .andExpect(status().isNoContent());

        mockMvc.perform(post("/api/v1/auth/refresh")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(refreshBody(refresh)))
                .andExpect(status().isUnauthorized());
    }

    @Test
    void logoutIsIdempotentAndSilentForUnknownTokens() throws Exception {
        String orgCode = createOrganizationAndGetCode();
        User user = createUserForOrganization(orgIdOf(orgCode), uniqueEmail("ido@"), PASSWORD);
        String refresh = loginAndGetRefreshToken(orgCode, user.getEmail(), PASSWORD);

        mockMvc.perform(post("/api/v1/auth/logout")
                .contentType(MediaType.APPLICATION_JSON)
                .content(refreshBody(refresh)));
        mockMvc.perform(post("/api/v1/auth/logout")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(refreshBody(refresh)));
        mockMvc.perform(post("/api/v1/auth/logout")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(refreshBody(RefreshTokenHasher.generateRawToken())))
                .andExpect(status().isNoContent());
    }

    @Test
    void logoutAllRevokesEveryActiveRefreshSession() throws Exception {
        String orgCode = createOrganizationAndGetCode();
        User user = createUserForOrganization(orgIdOf(orgCode), uniqueEmail("all@"), PASSWORD);
        String access = postLoginExpectingToken(orgCode, user.getEmail(), PASSWORD);
        String r1 = loginAndGetRefreshToken(orgCode, user.getEmail(), PASSWORD);
        String r2 = createManualSession(user);
        String r3 = createManualSession(user);

        mockMvc.perform(post("/api/v1/auth/logout-all")
                        .header("Authorization", "Bearer " + access))
                .andExpect(status().isNoContent());

        for (String raw : new String[] {r1, r2, r3}) {
            mockMvc.perform(post("/api/v1/auth/refresh")
                            .contentType(MediaType.APPLICATION_JSON)
                            .content(refreshBody(raw)))
                    .andExpect(status().isUnauthorized());
        }
    }

    @Test
    void logoutAllWithoutAccessTokenIsUnauthorized() throws Exception {
        mockMvc.perform(post("/api/v1/auth/logout-all"))
                .andExpect(status().isUnauthorized())
                .andExpect(jsonPath("$.code").value("AUTHENTICATION_REQUIRED"));
    }

    @Test
    void meWithoutTokenIsUnauthorized() throws Exception {
        mockMvc.perform(get("/api/v1/auth/me"))
                .andExpect(status().isUnauthorized())
                .andExpect(jsonPath("$.code").value("AUTHENTICATION_REQUIRED"));
    }

    @Test
    void meWithValidTokenReturnsDatabaseIdentity() throws Exception {
        String orgCode = createOrganizationAndGetCode();
        User user = createUserForOrganization(orgIdOf(orgCode), "  Me.Ident@Example.com ", PASSWORD);
        String access = postLoginExpectingToken(orgCode, "me.ident@example.com", PASSWORD);

        mockMvc.perform(get("/api/v1/auth/me")
                        .header("Authorization", "Bearer " + access))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.userId").value(user.getId().toString()))
                .andExpect(jsonPath("$.organizationId").value(user.getOrganizationId().toString()))
                .andExpect(jsonPath("$.email").value("me.ident@example.com"))
                .andExpect(jsonPath("$.status").value("ACTIVE"));
    }

    private String createManualSession(User user) {
        String raw = RefreshTokenHasher.generateRawToken();
        refreshSessionRepository.saveAndFlush(RefreshSession.create(
                user.getId(),
                RefreshTokenHasher.hash(raw),
                Instant.now(),
                Duration.ofDays(30),
                Duration.ofDays(30)));
        return raw;
    }

    private String refreshBody(String refreshToken) {
        return """
                {"refreshToken":"%s"}
                """.formatted(refreshToken);
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