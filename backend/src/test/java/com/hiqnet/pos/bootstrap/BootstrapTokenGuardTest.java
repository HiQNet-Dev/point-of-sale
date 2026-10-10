package com.hiqnet.pos.bootstrap.application;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;

import com.hiqnet.pos.shared.config.BootstrapProperties;
import com.hiqnet.pos.shared.config.BootstrapTokenGuard;
import com.hiqnet.pos.shared.error.BusinessException;
import com.hiqnet.pos.shared.error.ErrorCodes;
import java.util.Base64;
import org.junit.jupiter.api.Test;
import org.springframework.http.HttpStatus;

/**
 * Pure unit tests of the bootstrap secret semantics: Base64 validation,
 * fail-fast startup for weak secrets, constant-time comparison behavior and the
 * fail-closed guard when disabled.
 */
class BootstrapPropertiesAndTokenGuardTest {

    private static final String VALID_SECRET = "aGlxbmV0LXBvcy10ZXN0LW9ubHktYm9vdHN0cmFwLXNlY3JldC0wMDAx";

    @Test
    void absentOrBlankSecretDisablesBootstrap() {
        assertThat(new BootstrapProperties(null).enabled()).isFalse();
        assertThat(new BootstrapProperties("").enabled()).isFalse();
        assertThat(new BootstrapProperties("   ").enabled()).isFalse();
    }

    @Test
    void configuredSecretMustBeValidBase64() {
        assertThatThrownBy(() -> new BootstrapProperties("not base64!!"))
                .isInstanceOf(IllegalStateException.class)
                .hasMessageContaining("Base64");
    }

    @Test
    void configuredSecretMustDecodeToAtLeast32Bytes() {
        String shortBase64 = Base64.getEncoder().encodeToString(new byte[16]);
        assertThatThrownBy(() -> new BootstrapProperties(shortBase64))
                .isInstanceOf(IllegalStateException.class)
                .hasMessageContaining("at least 32 bytes");
    }

    @Test
    void tostringNeverPrintsTheSecret() {
        BootstrapProperties properties = new BootstrapProperties(VALID_SECRET);
        assertThat(properties.toString()).doesNotContain("aGlxbmV0");
    }

    @Test
    void guardAcceptsTheConfiguredSecretAndRejectsEverythingElse() {
        BootstrapTokenGuard guard = new BootstrapTokenGuard(new BootstrapProperties(VALID_SECRET));
        assertThat(guard.enabled()).isTrue();
        assertThat(guard.isValid(VALID_SECRET)).isTrue();
        assertThat(guard.isValid("wrong-token")).isFalse();
        assertThat(guard.isValid(null)).isFalse();
        assertThat(guard.isValid("")).isFalse();
        assertThat(guard.isValid("not base64!!")).isFalse();
        // A different, well-formed Base64 value must not match.
        String otherValidBase64 = Base64.getEncoder().encodeToString(new byte[32]);
        assertThat(guard.isValid(otherValidBase64)).isFalse();
    }

    @Test
    void verifyThrowsTheStableGenericUnauthorizedContract() {
        BootstrapTokenGuard guard = new BootstrapTokenGuard(new BootstrapProperties(VALID_SECRET));
        assertThatThrownBy(() -> guard.verify("wrong"))
                .isInstanceOfSatisfying(BusinessException.class, e -> {
                    assertThat(e.getStatus()).isEqualTo(HttpStatus.UNAUTHORIZED);
                    assertThat(e.getCode()).isEqualTo(ErrorCodes.BOOTSTRAP_INVALID_TOKEN);
                });
        // A valid token passes without exception.
        guard.verify(VALID_SECRET);
    }

    @Test
    void disabledGuardFailsClosed() {
        BootstrapTokenGuard guard = new BootstrapTokenGuard(new BootstrapProperties(null));
        assertThat(guard.enabled()).isFalse();
        assertThat(guard.isValid(VALID_SECRET)).isFalse();
        assertThatThrownBy(() -> guard.verify(VALID_SECRET))
                .isInstanceOf(BusinessException.class)
                .extracting(e -> ((BusinessException) e).getCode())
                .isEqualTo(ErrorCodes.BOOTSTRAP_INVALID_TOKEN);
    }
}