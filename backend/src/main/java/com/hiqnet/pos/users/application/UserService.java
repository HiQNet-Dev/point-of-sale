package com.hiqnet.pos.users.application;

import com.hiqnet.pos.organization.domain.Organization;
import com.hiqnet.pos.organization.domain.OrganizationRepository;
import com.hiqnet.pos.shared.error.BusinessException;
import com.hiqnet.pos.shared.error.ErrorCodes;
import com.hiqnet.pos.users.domain.User;
import com.hiqnet.pos.users.domain.UserRepository;
import com.hiqnet.pos.users.domain.UserStatus;
import java.util.UUID;
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
 * NOT depend on {@code auth} (that would create a module cycle): deactivating a
 * user is sufficient because the auth module already rejects INACTIVE users on
 * login/refresh/me and never resurrects revoked sessions.</p>
 *
 * <p>Password handling: the raw password exists only during the request; it is
 * hashed with the shared {@link PasswordEncoder} and never stored, logged or
 * returned (the response DTO carries no password material).</p>
 */
@Service
public class UserService {

    private final UserRepository userRepository;
    private final OrganizationRepository organizationRepository;
    private final PasswordEncoder passwordEncoder;

    public UserService(
            UserRepository userRepository,
            OrganizationRepository organizationRepository,
            PasswordEncoder passwordEncoder) {
        this.userRepository = userRepository;
        this.organizationRepository = organizationRepository;
        this.passwordEncoder = passwordEncoder;
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
        User user = requireUser(organizationId, userId);
        if (command.status() != null) {
            if (command.status() == UserStatus.ACTIVE) {
                user.activate();
            } else {
                user.deactivate();
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
