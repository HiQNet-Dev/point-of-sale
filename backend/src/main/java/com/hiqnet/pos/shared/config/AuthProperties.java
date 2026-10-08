package com.hiqnet.pos.shared.config;

import jakarta.validation.constraints.Max;
import jakarta.validation.constraints.Min;
import jakarta.validation.constraints.NotBlank;
import java.time.Duration;
import org.hibernate.validator.constraints.time.DurationMin;
import org.springframework.boot.context.properties.ConfigurationProperties;
import org.springframework.validation.annotation.Validated;

/**
 * Typed, validated authentication configuration. Non-sensitive parameters
 * have safe defaults expressed in application.yml; the JWT secret has NO
 * default here or in YAML — the placeholder resolves to an empty value when
 * AUTH_JWT_SECRET is missing and the {@code @NotBlank} constraint fails
 * application startup (fail fast, never a fallback secret). The secret is a
 * Base64-encoded key; its decoded length (>= 32 bytes) is enforced in
 * {@code AuthConfig}.
 *
 * <p>Temporal parameters are validated to be strictly positive: a configured
 * {@code 0} or a negative value fails startup instead of silently producing an
 * unusable token. {@code maxFailedAttempts}/{@code passwordStrength} are NOT
 * coerced: an invalid value fails startup like a missing secret.</p>
 */
@Validated
@ConfigurationProperties(prefix = "app.auth")
public record AuthProperties(
        @NotBlank
        String jwtSecret,

        @NotBlank
        String issuer,

        @NotBlank
        String audience,

        @DurationMin(nanos = 1, message = "access-token-ttl must be greater than zero")
        Duration accessTokenTtl,

        @DurationMin(nanos = 1, message = "refresh-token-ttl must be greater than zero")
        Duration refreshTokenTtl,

        @DurationMin(nanos = 1, message = "family-ttl must be greater than zero")
        Duration familyTtl,

        @Min(1)
        int maxFailedAttempts,

        @DurationMin(nanos = 1, message = "lockout-duration must be greater than zero")
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
        // secret, instead of silently degrading to a default. A configured but
        // non-positive duration is rejected by @DurationMin above.
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