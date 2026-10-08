package com.hiqnet.pos.auth.application;

import com.hiqnet.pos.auth.api.AuthResponses.MeResponse;
import com.hiqnet.pos.auth.api.AuthResponses.TokenResponse;
import com.hiqnet.pos.auth.domain.RefreshSession;
import com.hiqnet.pos.auth.domain.RefreshSessionRepository;
import com.hiqnet.pos.auth.infrastructure.RefreshTokenHasher;
import com.hiqnet.pos.organization.domain.Organization;
import com.hiqnet.pos.organization.domain.OrganizationRepository;
import com.hiqnet.pos.shared.config.AuthProperties;
import com.hiqnet.pos.shared.error.BusinessException;
import com.hiqnet.pos.shared.error.ErrorCodes;
import com.hiqnet.pos.users.domain.User;
import com.hiqnet.pos.users.domain.UserRepository;
import java.time.Clock;
import java.time.Instant;
import java.util.Locale;
import java.util.UUID;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.http.HttpStatus;
import org.springframework.security.crypto.password.PasswordEncoder;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

/**
 * Authentication flows: login, refresh rotation, logout and /me identity.
 * Public errors are deliberately generic (INVALID_CREDENTIALS /
 * INVALID_REFRESH_TOKEN) to avoid user enumeration; security-relevant events
 * are logged internally with safe metadata (ids, never tokens or passwords).
 *
 * <p>Concurrency: every path that mutates a user's refresh sessions first
 * acquires the user row with {@code SELECT ... FOR UPDATE} (lock ordering:
 * user -> session). PostgreSQL is the authority, so the guarantee holds across
 * application instances; concurrent refresh/logout/logout-all/reuse cannot
 * leave a live successor behind a revocation.</p>
 */
@Service
public class AuthService {

    private static final Logger log = LoggerFactory.getLogger(AuthService.class);

    private final OrganizationRepository organizationRepository;
    private final UserRepository userRepository;
    private final RefreshSessionRepository refreshSessionRepository;
    private final PasswordEncoder passwordEncoder;
    private final JwtTokenService jwtTokenService;
    private final AuthProperties properties;
    private final Clock clock;
    /** Equalizes response time when the user does not exist. */
    private final String dummyPasswordHash;

    public AuthService(
            OrganizationRepository organizationRepository,
            UserRepository userRepository,
            RefreshSessionRepository refreshSessionRepository,
            PasswordEncoder passwordEncoder,
            JwtTokenService jwtTokenService,
            AuthProperties properties,
            Clock clock) {
        this.organizationRepository = organizationRepository;
        this.userRepository = userRepository;
        this.refreshSessionRepository = refreshSessionRepository;
        this.passwordEncoder = passwordEncoder;
        this.jwtTokenService = jwtTokenService;
        this.properties = properties;
        this.clock = clock;
        this.dummyPasswordHash = passwordEncoder.encode(UUID.randomUUID().toString());
    }

    /**
     * Failure paths log events and throw, but the failed-attempt accounting
     * (and, in refresh, the family revocation on reuse detection) must be
     * COMMITTED: committing on BusinessException keeps those writes while
     * still surfacing the public error.
     */
    @Transactional(noRollbackFor = BusinessException.class)
    public TokenResponse login(String organizationCode, String email, String password) {
        String passwordToCheck = password == null ? "" : password;
        Instant now = clock.instant();

        // Email is validated BEFORE any lookup; failures map to the generic
        // credential error (with timing equalization) so malformed input
        // cannot be used to probe whether an organization exists.
        String normalizedEmail;
        try {
            normalizedEmail = User.normalizeEmail(email);
        } catch (BusinessException e) {
            passwordEncoder.matches(passwordToCheck, dummyPasswordHash);
            throw invalidCredentials();
        }
        String normalizedCode = organizationCode == null ? null : organizationCode.trim().toLowerCase(Locale.ROOT);

        Organization organization =
                organizationRepository.findByCode(normalizedCode).orElse(null);
        // An INACTIVE organization is treated exactly like an unknown one: no
        // user lookup, dummy hash, generic INVALID_CREDENTIALS. This blocks any
        // tenant user from logging in without revealing organization state.
        User user = organization == null || !organization.isActive()
                ? null
                : userRepository.findByOrganizationIdAndEmail(organization.getId(), normalizedEmail)
                        .orElse(null);

        if (user == null) {
            // Identical timing path for unknown/deactivated organization/email.
            passwordEncoder.matches(passwordToCheck, dummyPasswordHash);
            throw invalidCredentials();
        }
        if (user.isLocked(now)) {
            passwordEncoder.matches(passwordToCheck, user.getPasswordHash());
            log.warn("security-event=login-locked userId={} organizationId={}", user.getId(), user.getOrganizationId());
            throw invalidCredentials();
        }
        if (!user.isActive()) {
            passwordEncoder.matches(passwordToCheck, user.getPasswordHash());
            log.warn("security-event=login-inactive userId={} organizationId={}", user.getId(), user.getOrganizationId());
            throw invalidCredentials();
        }

        boolean matched = passwordEncoder.matches(passwordToCheck, user.getPasswordHash());
        if (!matched) {
            user.recordFailedLogin(now, properties.maxFailedAttempts(), properties.lockoutDuration());
            userRepository.save(user);
            log.warn(
                    "security-event=login-failure userId={} organizationId={} attempts={} locked={}",
                    user.getId(),
                    user.getOrganizationId(),
                    user.getFailedLoginAttempts(),
                    user.isLocked(now));
            throw invalidCredentials();
        }

        user.recordSuccessfulLogin(now);
        userRepository.save(user);
        log.info(
                "security-event=login-success userId={} organizationId={}",
                user.getId(),
                user.getOrganizationId());
        return issueTokens(user);
    }

    @Transactional(noRollbackFor = BusinessException.class)
    public TokenResponse refresh(String rawToken) {
        Instant now = clock.instant();
        String tokenHash = RefreshTokenHasher.hash(rawToken);

        // Lock ordering (user -> session): resolve the owner WITHOUT locking,
        // acquire the per-user coordination lock, and only then re-read the
        // session FOR UPDATE. Every session mutation of this user (refresh,
        // logout, logout-all, family revocation) serializes on the user row, so
        // a concurrent rotation can never leave a live successor behind a
        // revocation.
        UUID userId = refreshSessionRepository.findUserIdByTokenHash(tokenHash).orElse(null);
        if (userId == null) {
            throw invalidRefreshToken();
        }
        User user = userRepository.findByIdForUpdate(userId).orElse(null);
        RefreshSession session = refreshSessionRepository
                .findByTokenHashForUpdate(tokenHash)
                .orElseThrow(AuthService::invalidRefreshToken);

        if (session.isReusable(now)) {
            if (user == null || !user.isActive() || !isOrganizationActive(user.getOrganizationId())) {
                refreshSessionRepository.revokeFamily(session.getFamilyId(), now);
                throw invalidRefreshToken();
            }
            String newRawToken = RefreshTokenHasher.generateRawToken();
            RefreshSession next =
                    session.rotateTo(RefreshTokenHasher.hash(newRawToken), now, properties.refreshTokenTtl());
            refreshSessionRepository.save(session);
            refreshSessionRepository.save(next);
            log.info(
                    "security-event=token-rotated userId={} familyId={} sessionId={}",
                    user.getId(),
                    session.getFamilyId(),
                    next.getId());
            return new TokenResponse(
                    jwtTokenService.issueAccessToken(user.getId(), user.getOrganizationId()),
                    "Bearer",
                    jwtTokenService.accessTokenTtlSeconds(),
                    newRawToken,
                    next.usableUntil());
        }

        if (session.wasRotatedOrRevoked()) {
            // Presenting an already consumed token: revoke the whole family.
            refreshSessionRepository.revokeFamily(session.getFamilyId(), now);
            log.warn(
                    "security-event=refresh-reuse userId={} familyId={}",
                    session.getUserId(),
                    session.getFamilyId());
        }
        throw invalidRefreshToken();
    }

    /**
     * Revokes the presented session's whole family (i.e. the device/login
     * rotation chain) under the per-user coordination lock. Revoking the family
     * rather than a single row is what makes logout safe against a concurrent
     * rotation: if a rotation already created a successor, it dies too.
     * Idempotent and silent: an unknown token is the same 204 as a known one.
     */
    @Transactional
    public void logout(String rawToken) {
        if (rawToken == null || rawToken.isBlank()) {
            return;
        }
        Instant now = clock.instant();
        String tokenHash = RefreshTokenHasher.hash(rawToken);

        UUID userId = refreshSessionRepository.findUserIdByTokenHash(tokenHash).orElse(null);
        if (userId == null) {
            // Unknown token: same success, nothing to reveal.
            return;
        }
        // Acquire the user lock BEFORE touching any session row so a concurrent
        // rotation cannot commit a live successor after this logout.
        userRepository.findByIdForUpdate(userId);
        refreshSessionRepository
                .findByTokenHashForUpdate(tokenHash)
                .ifPresent(session -> refreshSessionRepository.revokeFamily(session.getFamilyId(), now));
    }

    @Transactional
    public void logoutAll(UUID userId) {
        Instant now = clock.instant();
        // Serialize with any in-flight rotation of this user before the bulk
        // revoke, so no successor escapes the logout-all.
        userRepository.findByIdForUpdate(userId);
        refreshSessionRepository.revokeActiveByUserId(userId, now);
        log.info("security-event=logout-all userId={}", userId);
    }

    /**
     * Resolves the identity behind a valid access token from the database:
     * the client-supplied claims never drive the response. Both the user and
     * its organization must be operational.
     */
    @Transactional(readOnly = true)
    public MeResponse me(UUID userId) {
        User user = userRepository.findById(userId).orElseThrow(AuthService::invalidTokenIdentity);
        if (!user.isActive() || !isOrganizationActive(user.getOrganizationId())) {
            throw invalidTokenIdentity();
        }
        return new MeResponse(user.getId(), user.getOrganizationId(), user.getEmail(), user.getStatus());
    }

    private TokenResponse issueTokens(User user) {
        String rawToken = RefreshTokenHasher.generateRawToken();
        Instant now = clock.instant();
        RefreshSession session = RefreshSession.create(
                user.getId(),
                RefreshTokenHasher.hash(rawToken),
                now,
                properties.refreshTokenTtl(),
                properties.familyTtl());
        refreshSessionRepository.save(session);
        return new TokenResponse(
                jwtTokenService.issueAccessToken(user.getId(), user.getOrganizationId()),
                "Bearer",
                jwtTokenService.accessTokenTtlSeconds(),
                rawToken,
                session.getFamilyExpiresAt());
    }

    private boolean isOrganizationActive(UUID organizationId) {
        return organizationRepository.findById(organizationId).map(Organization::isActive).orElse(false);
    }

    private static BusinessException invalidCredentials() {
        return new BusinessException(
                HttpStatus.UNAUTHORIZED, ErrorCodes.INVALID_CREDENTIALS, "Invalid credentials");
    }

    private static BusinessException invalidRefreshToken() {
        return new BusinessException(
                HttpStatus.UNAUTHORIZED, ErrorCodes.INVALID_REFRESH_TOKEN, "Invalid refresh token");
    }

    private static BusinessException invalidTokenIdentity() {
        return new BusinessException(
                HttpStatus.UNAUTHORIZED, ErrorCodes.TOKEN_INVALID, "Invalid token identity");
    }
}