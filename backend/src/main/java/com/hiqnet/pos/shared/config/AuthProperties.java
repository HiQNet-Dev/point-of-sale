package com.hiqnet.pos.shared.config;

import jakarta.validation.constraints.Max;
import jakarta.validation.constraints.Min;
import jakarta.validation.constraints.NotBlank;
import jakarta.validation.constraints.Size;
import java.time.Duration;
import org.springframework.boot.context.properties.ConfigurationProperties;
import org.springframework.validation.annotation.Validated;

/**
 * Typed, validated authentication configuration. Non-sensitive parameters
 * have safe defaults expressed in application.yml; the JWT secret has NO
 * default here or in YAML — the placeholder resolves to an empty value when
 * AUTH_JWT_SECRET is missing and the @NotBlank/@Size constraints fail
 * application startup (fail fast, never a fallback secret).
 */
@Validated
@ConfigurationProperties(prefix = "app.auth")
public record AuthProperties(
        @NotBlank
        @Size(min = 32, message = "AUTH_JWT_SECRET must be at least 32 bytes (256 bits) of real entropy")
        String jwtSecret,

        @NotBlank
        String issuer,

        @NotBlank
        String audience,

        Duration accessTokenTtl,

        Duration refreshTokenTtl,

        Duration familyTtl,

        @Min(1)
        int maxFailedAttempts,

        Duration lockoutDuration,

        @Min(4)
        @Max(31)
        int passwordStrength) {

    private static final Duration DEFAULT_ACCESS_TTL = Duration.ofMinutes(10);
    private static final Duration DEFAULT_REFRESH_TTL = Duration.ofDays(30);
    private static final Duration DEFAULT_FAMILY_TTL = Duration.ofDays(30);
    private static final Duration DEFAULT_LOCKOUT = Duration.ofMinutes(15);

    public AuthProperties {
        // Nullable durations get safe defaults (Validation-friendly); numeric
        // constraints (maxFailedAttempts, passwordStrength) are NOT coerced:
        // an invalid configured value fails application startup like a missing
        // secret, instead of silently degrading to a default.
        if (accessTokenTtl == null) {
            accessTokenTtl = DEFAULT_ACCESS_TTL;
        }
        if (refreshTokenTtl == null) {
            refreshTokenTtl = DEFAULT_REFRESH_TTL;
        }
        if (familyTtl == null) {
            familyTtl = DEFAULT_FAMILY_TTL;
        }
        if (lockoutDuration == null) {
            lockoutDuration = DEFAULT_LOCKOUT;
        }
    }
}