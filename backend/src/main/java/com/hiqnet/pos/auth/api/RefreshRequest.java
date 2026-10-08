package com.hiqnet.pos.auth.api;

import io.swagger.v3.oas.annotations.media.Schema;
import jakarta.validation.constraints.NotBlank;
import jakarta.validation.constraints.Size;

/** Body for POST /api/v1/auth/refresh and POST /api/v1/auth/logout. */
public record RefreshRequest(
        @Schema(description = "Opaque refresh token previously returned by login or refresh.", example = "ab3Q_9...")
        @NotBlank
        @Size(max = 256)
        String refreshToken) {
}