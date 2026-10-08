package com.hiqnet.pos.auth.api;

import io.swagger.v3.oas.annotations.media.Schema;
import jakarta.validation.constraints.NotBlank;
import jakarta.validation.constraints.Size;

/** POST /api/v1/auth/login */
public record LoginRequest(
        @Schema(description = "Organization code (slug).", example = "cafeteria-central")
        @NotBlank
        @Size(max = 64)
        String organizationCode,

        @Schema(description = "Email address. Whitepace and case are normalized server-side.", example = "admin@example.com")
        @NotBlank
        @Size(max = 254)
        String email,

        @Schema(description = "Password (never logged, never persisted in plaintext).", example = "***")
        @NotBlank
        @Size(max = 128)
        String password) {
}