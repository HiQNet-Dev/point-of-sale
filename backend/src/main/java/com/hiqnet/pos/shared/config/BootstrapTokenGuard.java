package com.hiqnet.pos.shared.config;

import com.hiqnet.pos.shared.error.BusinessException;
import com.hiqnet.pos.shared.error.ErrorCodes;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.Base64;
import org.springframework.http.HttpStatus;
import org.springframework.stereotype.Component;

/**
 * Verifies the {@code X-Bootstrap-Token} header against the configured
 * deployment secret. Two explicit properties:
 *
 * <ul>
 *   <li><b>Timing-resistant comparison:</b> the expected and provided secrets
 *       are Base64-decoded and then compared through SHA-256 digests of fixed
 *       length using {@link MessageDigest#isEqual} — never {@code equals} on the
 *       raw secret. Malformed Base64 is simply "invalid" (no distinct error).</li>
 *   <li><b>Fail-closed when disabled:</b> if no secret is configured the guard
 *       is disabled and every token is invalid; the route is simultaneously
 *       closed by {@code SecurityConfig} (deny-by-default).</li>
 * </ul>
 *
 * <p>The received token is never logged or echoed. The public error is always
 * the same generic {@code 401 BOOTSTRAP_INVALID_TOKEN}, so an unauthenticated
 * caller cannot distinguish "wrong length", "bad Base64" or "wrong value".</p>
 */
@Component
public class BootstrapTokenGuard {

    /** HTTP header carrying the one-shot bootstrap secret. Never a query/body parameter. */
    public static final String HEADER = "X-Bootstrap-Token";

    private static final String HASH_ALGORITHM = "SHA-256";

    /** SHA-256 of the decoded expected secret, or {@code null} when disabled. */
    private final byte[] expectedDigest;

    public BootstrapTokenGuard(BootstrapProperties properties) {
        this.expectedDigest = properties.enabled() ? sha256(properties.decodedSecret()) : null;
    }

    public boolean enabled() {
        return expectedDigest != null;
    }

    /** Whether the provided header value matches the configured secret. */
    public boolean isValid(String providedToken) {
        if (expectedDigest == null || providedToken == null || providedToken.isBlank()) {
            return false;
        }
        byte[] decoded;
        try {
            decoded = Base64.getDecoder().decode(providedToken);
        } catch (IllegalArgumentException e) {
            return false;
        }
        return MessageDigest.isEqual(expectedDigest, sha256(decoded));
    }

    /**
     * Fail-closed guard for the bootstrap path: throws the generic
     * {@code 401 BOOTSTRAP_INVALID_TOKEN} unless the token is valid. Used as a
     * defence-in-depth re-check inside the controller, in addition to the
     * request filter that runs before the body is even read.
     */
    public void verify(String providedToken) {
        if (!isValid(providedToken)) {
            throw new BusinessException(
                    HttpStatus.UNAUTHORIZED,
                    ErrorCodes.BOOTSTRAP_INVALID_TOKEN,
                    "Bootstrap token is invalid or bootstrap is disabled");
        }
    }

    private static byte[] sha256(byte[] value) {
        try {
            return MessageDigest.getInstance(HASH_ALGORITHM).digest(value);
        } catch (NoSuchAlgorithmException e) {
            throw new IllegalStateException("SHA-256 is required but unavailable", e);
        }
    }
}
