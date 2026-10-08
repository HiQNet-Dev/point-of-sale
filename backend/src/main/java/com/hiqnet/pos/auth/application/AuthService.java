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
        User user = organization == null
                ? null
                : userRepository.findByOrganizationIdAndEmail(organization.getId(), normalizedEmail)
                        .orElse(null);

        if (user == null) {
            // Identical timing path for unknown organization/email.
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
        RefreshSession session = refreshSessionRepository
                .findByTokenHashForUpdate(tokenHash)
                .orElseThrow(() -> invalidRefreshToken());

        if (session.isReusable(now)) {
            User user = userRepository.findById(session.getUserId()).orElse(null);
            if (user == null || !user.isActive()) {
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
                    session.getFamilyExpiresAt());
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

    @Transactional
    public void logout(String rawToken) {
        if (rawToken == null || rawToken.isBlank()) {
            return;
        }
        Instant now = clock.instant();
        refreshSessionRepository
                .findByTokenHash(RefreshTokenHasher.hash(rawToken))
                .ifPresent(session -> session.revoke(now));
        // Unknown token: same success, nothing to reveal.
    }

    @Transactional
    public void logoutAll(UUID userId) {
        refreshSessionRepository.revokeActiveByUserId(userId, clock.instant());
        log.info("security-event=logout-all userId={}", userId);
    }

    /**
     * Resolves the identity behind a valid access token from the database:
     * the client-supplied claims never drive the response.
     */
    @Transactional(readOnly = true)
    public MeResponse me(UUID userId) {
        User user = userRepository.findById(userId)
                .orElseThrow(() -> new BusinessException(
                        HttpStatus.UNAUTHORIZED, ErrorCodes.TOKEN_INVALID, "Invalid token identity"));
        if (!user.isActive()) {
            throw new BusinessException(
                    HttpStatus.UNAUTHORIZED, ErrorCodes.TOKEN_INVALID, "Invalid token identity");
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

    private static BusinessException invalidCredentials() {
        return new BusinessException(
                HttpStatus.UNAUTHORIZED, ErrorCodes.INVALID_CREDENTIALS, "Invalid credentials");
    }

    private static BusinessException invalidRefreshToken() {
        return new BusinessException(
                HttpStatus.UNAUTHORIZED, ErrorCodes.INVALID_REFRESH_TOKEN, "Invalid refresh token");
    }
}