package com.hiqnet.pos.auth.api;

import com.hiqnet.pos.users.domain.UserStatus;
import io.swagger.v3.oas.annotations.media.Schema;
import java.time.Instant;
import java.util.UUID;

/** Public response DTOs for the auth endpoints. */
public final class AuthResponses {

    private AuthResponses() {
    }

    public record TokenResponse(
            @Schema(description = "Short-lived JWT access token (10 minutes).", example = "eyJ...")
            String accessToken,
            @Schema(description = "Token type for the Authorization header.", example = "Bearer")
            String tokenType,
            @Schema(description = "Access token lifetime in seconds.", example = "600")
            long expiresIn,
            @Schema(description = "Opaque refresh token. Delivered once; never stored by the server.", example = "ab3Q_9...")
            String refreshToken,
            @Schema(description = "Absolute expiry of the refresh session family (UTC ISO-8601).", example = "2026-11-07T00:00:00Z")
            Instant refreshExpiresAt) {
    }

    public record MeResponse(
            @Schema(description = "User UUID (claim sub of the access token).")
            UUID userId,
            @Schema(description = "Organization UUID the user belongs to.")
            UUID organizationId,
            @Schema(description = "Normalized email (trimmed, lowercase). This is the database identity.")
            String email,
            @Schema(description = "User status from the database.", example = "ACTIVE")
            UserStatus status) {
    }
}