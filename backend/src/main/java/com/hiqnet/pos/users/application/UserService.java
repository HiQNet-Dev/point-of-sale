package com.hiqnet.pos.users.application;

import com.hiqnet.pos.organization.domain.Organization;
import com.hiqnet.pos.organization.domain.OrganizationRepository;
import com.hiqnet.pos.shared.error.BusinessException;
import com.hiqnet.pos.shared.error.ErrorCodes;
import com.hiqnet.pos.users.domain.User;
import com.hiqnet.pos.users.domain.UserRepository;
import com.hiqnet.pos.users.domain.UserStatus;
import java.util.UUID;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.data.domain.Page;
import org.springframework.data.domain.Pageable;
import org.springframework.http.HttpStatus;
import org.springframework.security.crypto.password.PasswordEncoder;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

/**
 * Administrative use cases for users, always scoped to an organization.
 *
 * <p>Module direction: {@code users -> organization, shared}. This service must
 * NOT depend on {@code auth} (that would create a module cycle): session
 * revocation on deactivation is delegated to a port
 * ({@link UserSessionRevocationPort}) implemented by the authentication
 * module, so the compile-time direction stays {@code auth -> users}.</p>
 *
 * <p>Deactivation is a security boundary: every active refresh session is
 * revoked in the SAME transaction as {@code status = INACTIVE} (atomic,
 * user-lock first), and a later reactivation never resurrects them.</p>
 *
 * <p>Password handling: the raw password exists only during the request; it is
 * hashed with the shared {@link PasswordEncoder} and never stored, logged or
 * returned (the response DTO carries no password material).</p>
 */
@Service
public class UserService {

    private static final Logger log = LoggerFactory.getLogger(UserService.class);

    private final UserRepository userRepository;
    private final OrganizationRepository organizationRepository;
    private final PasswordEncoder passwordEncoder;
    private final UserSessionRevocationPort sessionRevocationPort;

    public UserService(
            UserRepository userRepository,
            OrganizationRepository organizationRepository,
            PasswordEncoder passwordEncoder,
            UserSessionRevocationPort sessionRevocationPort) {
        this.userRepository = userRepository;
        this.organizationRepository = organizationRepository;
        this.passwordEncoder = passwordEncoder;
        this.sessionRevocationPort = sessionRevocationPort;
    }

    @Transactional
    public User create(UUID organizationId, String email, String rawPassword) {
        Organization organization = organizationRepository
                .findById(organizationId)
                .orElseThrow(() -> new BusinessException(
                        HttpStatus.NOT_FOUND, ErrorCodes.ORGANIZATION_NOT_FOUND, "Organization not found"));
        if (!organization.isActive()) {
            throw new BusinessException(
                    HttpStatus.CONFLICT,
                    ErrorCodes.ORGANIZATION_INACTIVE,
                    "Organization is inactive; new users cannot be created");
        }
        String normalizedEmail = User.normalizeEmail(email);
        PasswordPolicy.validate(rawPassword);
        if (userRepository.existsByOrganizationIdAndEmail(organizationId, normalizedEmail)) {
            throw emailAlreadyExists();
        }
        User user = User.create(organizationId, normalizedEmail, passwordEncoder.encode(rawPassword));
        return userRepository.saveAndFlush(user);
    }

    @Transactional(readOnly = true)
    public User get(UUID organizationId, UUID userId) {
        requireOrganization(organizationId);
        return requireUser(organizationId, userId);
    }

    @Transactional(readOnly = true)
    public Page<User> list(UUID organizationId, UserStatus status, Pageable pageable) {
        requireOrganization(organizationId);
        return status == null
                ? userRepository.findByOrganizationId(organizationId, pageable)
                : userRepository.findByOrganizationIdAndStatus(organizationId, status, pageable);
    }

    @Transactional
    public User update(UUID organizationId, UUID userId, UpdateUserCommand command) {
        requireOrganization(organizationId);
        // Locking lookup (org-scoped): serializes administrative mutations and
        // the later session revocation against auth flows on the user row
        // (lock order user -> sessions).
        User user = requireUserForUpdate(organizationId, userId);
        if (user.getVersion() != command.expectedVersion()) {
            // Stale write: fail BEFORE any mutation (including the
            // destructive session revocation of a deactivation).
            throw new BusinessException(
                    HttpStatus.CONFLICT,
                    ErrorCodes.CONCURRENT_MODIFICATION,
                    "The resource was modified concurrently; retry with fresh data");
        }
        if (command.status() != null) {
            boolean wasActive = user.isActive();
            if (command.status() == UserStatus.ACTIVE) {
                user.activate();
            } else {
                user.deactivate();
                if (wasActive) {
                    log.info(
                            "security-event=user-deactivated userId={} organizationId={}",
                            user.getId(),
                            organizationId);
                }
                // Idempotent: re-deactivating an already INACTIVE user revokes
                // nothing (0). Same transaction as the status change.
                int revoked = sessionRevocationPort.revokeAllForUser(user.getId());
                if (revoked > 0) {
                    log.info(
                            "security-event=user-sessions-revoked userId={} organizationId={} revoked={}",
                            user.getId(),
                            organizationId,
                            revoked);
                }
            }
        }
        if (command.email() != null) {
            String normalizedEmail = User.normalizeEmail(command.email());
            if (!normalizedEmail.equals(user.getEmail())
                    && userRepository.existsByOrganizationIdAndEmail(organizationId, normalizedEmail)) {
                throw emailAlreadyExists();
            }
            user.changeEmail(normalizedEmail);
        }
        return userRepository.saveAndFlush(user);
    }

    private User requireUser(UUID organizationId, UUID userId) {
        return userRepository
                .findByIdAndOrganizationId(userId, organizationId)
                .orElseThrow(() -> new BusinessException(
                        HttpStatus.NOT_FOUND, ErrorCodes.USER_NOT_FOUND, "User not found"));
    }

    private User requireUserForUpdate(UUID organizationId, UUID userId) {
        return userRepository
                .findByIdAndOrganizationIdForUpdate(userId, organizationId)
                .orElseThrow(() -> new BusinessException(
                        HttpStatus.NOT_FOUND, ErrorCodes.USER_NOT_FOUND, "User not found"));
    }

    private void requireOrganization(UUID organizationId) {
        if (!organizationRepository.existsById(organizationId)) {
            throw new BusinessException(
                    HttpStatus.NOT_FOUND, ErrorCodes.ORGANIZATION_NOT_FOUND, "Organization not found");
        }
    }

    private static BusinessException emailAlreadyExists() {
        return new BusinessException(
                HttpStatus.CONFLICT,
                ErrorCodes.USER_EMAIL_ALREADY_EXISTS,
                "A user with this email already exists in the organization");
    }
}
