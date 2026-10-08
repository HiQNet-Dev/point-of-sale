package com.hiqnet.pos.shared.config;

import static org.assertj.core.api.Assertions.assertThat;

import jakarta.validation.Validation;
import jakarta.validation.Validator;
import java.time.Duration;
import org.junit.jupiter.api.Test;

/**
 * Fail-fast contract of the typed auth configuration: durations must be
 * strictly positive; explicit zero/negative values are rejected while omitted
 * (null) values fall back to safe defaults.
 */
class AuthPropertiesValidationTest {

    private final Validator validator = Validation.buildDefaultValidatorFactory().getValidator();

    private AuthProperties props(Duration access, Duration refresh, Duration family, Duration lockout) {
        return new AuthProperties(
                "a-secret", "hiqnet-pos", "hiqnet-pos-api", access, refresh, family, 5, lockout, 12);
    }

    @Test
    void positiveDurationsAreAccepted() {
        assertThat(validator.validate(props(
                        Duration.ofMinutes(10), Duration.ofDays(30), Duration.ofDays(30), Duration.ofMinutes(15))))
                .isEmpty();
    }

    @Test
    void nullDurationsFallBackToDefaultsAndAreValid() {
        assertThat(validator.validate(props(null, null, null, null))).isEmpty();
    }

    @Test
    void zeroAccessTokenTtlIsRejected() {
        assertThat(validator.validate(props(
                        Duration.ZERO, Duration.ofDays(30), Duration.ofDays(30), Duration.ofMinutes(15))))
                .isNotEmpty();
    }

    @Test
    void negativeRefreshTokenTtlIsRejected() {
        assertThat(validator.validate(props(
                        Duration.ofMinutes(10), Duration.ofMinutes(-1), Duration.ofDays(30), Duration.ofMinutes(15))))
                .isNotEmpty();
    }

    @Test
    void zeroFamilyTtlIsRejected() {
        assertThat(validator.validate(props(
                        Duration.ofMinutes(10), Duration.ofDays(30), Duration.ZERO, Duration.ofMinutes(15))))
                .isNotEmpty();
    }

    @Test
    void zeroLockoutDurationIsRejected() {
        assertThat(validator.validate(props(
                        Duration.ofMinutes(10), Duration.ofDays(30), Duration.ofDays(30), Duration.ZERO)))
                .isNotEmpty();
    }
}
