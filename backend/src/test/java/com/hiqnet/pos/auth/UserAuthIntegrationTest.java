package com.hiqnet.pos.auth;

import static org.assertj.core.api.Assertions.assertThat;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.get;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.jsonPath;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import com.hiqnet.pos.auth.domain.RefreshSession;
import com.hiqnet.pos.auth.domain.RefreshSessionRepository;
import com.hiqnet.pos.auth.infrastructure.RefreshTokenHasher;
import com.hiqnet.pos.organization.domain.Organization;
import com.hiqnet.pos.users.application.UpdateUserCommand;
import com.hiqnet.pos.users.application.UserService;
import com.hiqnet.pos.users.domain.User;
import com.hiqnet.pos.users.domain.UserStatus;
import java.nio.charset.StandardCharsets;
import java.time.Instant;
import java.util.List;
import java.util.UUID;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.http.MediaType;

/**
 * Cross-module contract Users -> Auth (one direction only): deactivating a user
 * through the users module must revoke ALL of its refresh sessions (every
 * device family) in the same transaction, and a later reactivation must NOT
 * resurrect them. The users module never calls auth; it delegates session
 * revocation to a port implemented by auth, so there is no module cycle.
 */
class UserAuthIntegrationTest extends AbstractAuthIntegrationTest {

    @Autowired
    private UserService userService;

    @Autowired
    private RefreshSessionRepository refreshSessionRepository;

    @Test
    void deactivationThenReactivationRequiresANewLogin() throws Exception {
        Fixture fx = fixture();
        String email = fx.email();

        /* 1. login -> capture refresh token A */
        String loginA = loginResponse(fx.code(), email, PASSWORD);
        String refreshA = objectMapper.readTree(loginA).get("refreshToken").asText();

        /* 2. deactivate; the DB must show every session revoked */
        userService.update(fx.orgId(), fx.user().getId(),
                new UpdateUserCommand(currentVersion(fx.user().getId()), null, UserStatus.INACTIVE));
        assertAllSessionsRevoked(fx.user().getId());

        /* 3. reactivate WITHOUT ever using A */
        userService.update(fx.orgId(), fx.user().getId(),
                new UpdateUserCommand(currentVersion(fx.user().getId()), null, UserStatus.ACTIVE));

        /* 4. old refresh token A stays dead */
        mockMvc.perform(post("/api/v1/auth/refresh")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(refreshBody(refreshA)))
                .andExpect(status().isUnauthorized())
                .andExpect(jsonPath("$.code").value("INVALID_REFRESH_TOKEN"));

        /* 5. a NEW login works and issues a different refresh token B */
        String loginB = loginResponse(fx.code(), email, PASSWORD);
        String refreshB = objectMapper.readTree(loginB).get("refreshToken").asText();
        assertThat(refreshB).isNotEqualTo(refreshA);

        /* 6. B is immediately usable */
        mockMvc.perform(post("/api/v1/auth/refresh")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(refreshBody(refreshB)))
                .andExpect(status().isOk());
    }

    @Test
    void deactivationRevokesEveryDeviceFamilyAndReactivationDoesNotResurrectThem() throws Exception {
        Fixture fx = fixture();
        String email = fx.email();

        /* Device A and device B: two logins = two independent families. */
        String refreshA = loginAndGetRefreshToken(fx.code(), email, PASSWORD);
        String refreshB = loginAndGetRefreshToken(fx.code(), email, PASSWORD);
        UUID familyA = familyIdOf(refreshA);
        UUID familyB = familyIdOf(refreshB);
        assertThat(familyA).isNotEqualTo(familyB);

        /* Deactivate: every family must be revoked, not just the last session. */
        userService.update(fx.orgId(), fx.user().getId(),
                new UpdateUserCommand(currentVersion(fx.user().getId()), null, UserStatus.INACTIVE));
        assertNoUsableSessionInFamily(familyA);
        assertNoUsableSessionInFamily(familyB);
        assertAllSessionsRevoked(fx.user().getId());

        /* Reactivate: neither family comes back to life. */
        userService.update(fx.orgId(), fx.user().getId(),
                new UpdateUserCommand(currentVersion(fx.user().getId()), null, UserStatus.ACTIVE));
        mockMvc.perform(post("/api/v1/auth/refresh")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(refreshBody(refreshA)))
                .andExpect(status().isUnauthorized());
        mockMvc.perform(post("/api/v1/auth/refresh")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(refreshBody(refreshB)))
                .andExpect(status().isUnauthorized());

        /* A fresh login works again. */
        mockMvc.perform(post("/api/v1/auth/login")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(loginJson(fx.code(), email, PASSWORD)))
                .andExpect(status().isOk());
    }

    private void assertAllSessionsRevoked(UUID userId) {
        Instant now = Instant.now();
        List<RefreshSession> sessions = refreshSessionRepository.findByUserId(userId);
        assertThat(sessions).isNotEmpty();
        // A session is either rotated (already consumed) or revoked: none usable.
        assertThat(sessions).allMatch(session -> !session.isReusable(now));
        // Every formerly-active (non-rotated) session carries a revoked_at.
        assertThat(sessions)
                .allMatch(session -> session.getRotatedAt() != null || session.getRevokedAt() != null);
    }

    private void assertNoUsableSessionInFamily(UUID familyId) {
        Instant now = Instant.now();
        assertThat(refreshSessionRepository.findByFamilyId(familyId))
                .isNotEmpty()
                .allMatch(session -> !session.isReusable(now));
    }

    private UUID familyIdOf(String rawToken) {
        return refreshSessionRepository
                .findByTokenHash(RefreshTokenHasher.hash(rawToken))
                .orElseThrow()
                .getFamilyId();
    }

    private long currentVersion(UUID userId) {
        return userRepository.findById(userId).orElseThrow().getVersion();
    }

    private Fixture fixture() {
        String code = "uorg-" + UUID.randomUUID().toString().substring(0, 8);
        Organization organization =
                Organization.create("Org " + code, code, "America/Mexico_City", "MXN", "es-MX");
        organizationRepository.saveAndFlush(organization);
        String email = uniqueEmail("lifecycle@");
        User user = userService.create(organization.getId(), email, PASSWORD);
        return new Fixture(organization.getId(), code, email, user);
    }

    private String loginResponse(String code, String email, String password) throws Exception {
        return mockMvc.perform(post("/api/v1/auth/login")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(loginJson(code, email, password)))
                .andExpect(status().isOk())
                .andReturn()
                .getResponse()
                .getContentAsString(StandardCharsets.UTF_8);
    }

    private static String refreshBody(String refreshToken) {
        return "{\"refreshToken\":\"" + refreshToken + "\"}";
    }

    private record Fixture(UUID orgId, String code, String email, User user) {
    }
}