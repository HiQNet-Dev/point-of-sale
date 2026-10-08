package com.hiqnet.pos.users.api;

import io.swagger.v3.oas.annotations.media.Schema;

/**
 * Create payload for a user. The organization is taken ONLY from the path
 * (never the body); immutable/server-managed fields (id, organizationId,
 * passwordHash, failedLoginAttempts, lockedUntil, version, ...) are not part of
 * this contract, so they cannot be mass-assigned.
 */
public record CreateUserRequest(
        @Schema(description = "User email; normalized to trim+lowercase.", example = "cashier@example.com")
        String email,
        @Schema(description = "Initial password (hashed, never stored or returned).", example = "initial-password")
        String password) {
}
