package com.hiqnet.pos.users.api;

import com.hiqnet.pos.users.domain.User;
import com.hiqnet.pos.users.domain.UserStatus;
import java.time.Instant;
import java.util.UUID;

/**
 * Administrative view of a user identity. Never exposes password material,
 * refresh sessions or token hashes. Lockout internals
 * ({@code failedLoginAttempts}, {@code lockedUntil}) are intentionally omitted
 * until an administrative need requires them.
 */
public record UserResponse(
        UUID id,
        UUID organizationId,
        String email,
        UserStatus status,
        Instant lastLoginAt,
        long version,
        Instant createdAt,
        Instant updatedAt) {

    public static UserResponse from(User user) {
        return new UserResponse(
                user.getId(),
                user.getOrganizationId(),
                user.getEmail(),
                user.getStatus(),
                user.getLastLoginAt(),
                user.getVersion(),
                user.getCreatedAt(),
                user.getUpdatedAt());
    }
}
