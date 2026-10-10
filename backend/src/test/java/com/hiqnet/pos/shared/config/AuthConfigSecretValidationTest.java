package com.hiqnet.pos.shared.config;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;

import java.time.Duration;
import java.util.Base64;
import org.junit.jupiter.api.Test;

/**
 * Fail-fast contract of the JWT signing secret: {@code AUTH_JWT_SECRET} must be
 * a Base64 value decoding to at least 32 bytes (HS256 needs >= 256 bits).
 * An invalid or weak configured secret fails application startup — there is
 * never a fallback or a silent degradation.
 */
class AuthConfigSecretValidationTest {

    private static final String VALID_TEST_SECRET = "aGlxbmV0LXBvcy10ZXN0LW9ubHktand0LXNpZ25pbmcta2V5";

    private final AuthConfig authConfig = new AuthConfig();

    @Test
    void validSecretIsAccepted() {
        assertThat(authConfig.jwtSecretKey(props(VALID_TEST_SECRET))).isNotNull();
    }

    @Test
    void nonBase64SecretFailsStartup() {
        assertThatThrownBy(() -> authConfig.jwtSecretKey(props("not base64 at all !!!")))
                .isInstanceOf(IllegalStateException.class)
                .hasMessageContaining("Base64");
    }

    @Test
    void secretShorterThan32BytesFailsStartup() {
        String shortSecret = Base64.getEncoder().encodeToString(new byte[16]);
        assertThatThrownBy(() -> authConfig.jwtSecretKey(props(shortSecret)))
                .isInstanceOf(IllegalStateException.class)
                .hasMessageContaining("at least 32 bytes");
    }

    @Test
    void blankSecretFailsBeanValidation() {
        jakarta.validation.Validator validator = jakarta.validation.Validation.buildDefaultValidatorFactory()
                .getValidator();
        assertThat(validator.validate(props(""))).isNotEmpty();
    }

    private static AuthProperties props(String jwtSecret) {
        return new AuthProperties(
                jwtSecret,
                "hiqnet-pos",
                "hiqnet-pos-api",
                Duration.ofMinutes(10),
                Duration.ofDays(30),
                Duration.ofDays(30),
                5,
                Duration.ofMinutes(15),
                12);
    }
}