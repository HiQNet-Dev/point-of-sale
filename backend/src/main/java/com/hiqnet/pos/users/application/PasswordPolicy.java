package com.hiqnet.pos.users.application;

import com.hiqnet.pos.shared.error.BusinessException;
import com.hiqnet.pos.shared.error.ErrorCodes;
import org.springframework.http.HttpStatus;

/**
 * Minimal, centralized password policy for locally-managed credentials. Kept
 * deliberately simple (length only): no artificial composition rules
 * (upper/lower/digit/symbol) without a documented product reason.
 *
 * <p>Used by the initial-password flow today; future change-password /
 * reset-password flows MUST reuse it. The raw password is never stored or
 * logged anywhere.</p>
 */
public final class PasswordPolicy {

    public static final int MIN_LENGTH = 12;
    public static final int MAX_LENGTH = 128;

    private PasswordPolicy() {
    }

    /** Throws {@code 400 WEAK_PASSWORD} when the raw password is out of bounds. */
    public static void validate(String rawPassword) {
        int length = rawPassword == null ? 0 : rawPassword.length();
        if (length < MIN_LENGTH) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST,
                    ErrorCodes.WEAK_PASSWORD,
                    "password must be at least " + MIN_LENGTH + " characters");
        }
        if (length > MAX_LENGTH) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST,
                    ErrorCodes.WEAK_PASSWORD,
                    "password must not exceed " + MAX_LENGTH + " characters");
        }
    }
}
