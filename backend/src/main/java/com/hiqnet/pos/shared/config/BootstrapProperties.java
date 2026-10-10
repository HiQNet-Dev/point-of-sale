package com.hiqnet.pos.shared.config;

import java.util.Base64;
import org.springframework.boot.context.properties.ConfigurationProperties;

/**
 * Typed, validated bootstrap configuration. Unlike {@code AUTH_JWT_SECRET}, the
 * bootstrap secret is OPTIONAL: an installation that is already provisioned
 * must start without it, and when it is absent (or blank) the HTTP bootstrap
 * route stays CLOSED (deny-by-default). When it IS configured it must be a
 * Base64 value decoding to at least 32 bytes (256 bits of entropy, e.g.
 * {@code openssl rand -base64 32}); an invalid/weak configured value fails
 * application startup (fail fast, never a fallback secret).
 *
 * <p>The secret is deliberately distinct from {@code AUTH_JWT_SECRET}: the
 * bootstrap token authorizes one-shot provisioning, it is NOT the administrator
 * password and NOT a signing key. It is never logged, exposed through actuator
 * endpoints or printed by {@link #toString()}.</p>
 */
@ConfigurationProperties(prefix = "app.bootstrap")
public record BootstrapProperties(String secret) {

    /** Minimum decoded secret length (256 bits) for a high-entropy deployment secret. */
    public static final int MIN_SECRET_BYTES = 32;

    public BootstrapProperties {
        if (secret != null && !secret.isBlank()) {
            byte[] decoded;
            try {
                decoded = Base64.getDecoder().decode(secret);
            } catch (IllegalArgumentException e) {
                throw new IllegalStateException(
                        "BOOTSTRAP_SECRET must be a Base64-encoded value; generate one with `openssl rand -base64 32`",
                        e);
            }
            if (decoded.length < MIN_SECRET_BYTES) {
                throw new IllegalStateException("BOOTSTRAP_SECRET must decode to at least "
                        + MIN_SECRET_BYTES + " bytes (256 bits)");
            }
        }
    }

    /** Bootstrap is enabled only when a non-blank secret is configured. */
    public boolean enabled() {
        return secret != null && !secret.isBlank();
    }

    /** The decoded secret bytes; only valid when {@link #enabled()}. */
    public byte[] decodedSecret() {
        return Base64.getDecoder().decode(secret);
    }

    /** Redacts the secret: configuration objects may be logged by the framework. */
    @Override
    public String toString() {
        return "BootstrapProperties[secret=***]";
    }
}
