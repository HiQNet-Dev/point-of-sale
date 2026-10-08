package com.hiqnet.pos.shared.error;

import java.time.Instant;
import java.util.List;

/**
 * Stable API error contract. Never includes stack traces or internal details.
 */
public record ApiError(
        Instant timestamp,
        int status,
        String code,
        String message,
        String path,
        List<String> details) {

    public static ApiError of(int status, String code, String message, String path) {
        return new ApiError(Instant.now(), status, code, message, path, List.of());
    }

    public static ApiError of(int status, String code, String message, String path, List<String> details) {
        return new ApiError(Instant.now(), status, code, message, path, details);
    }
}
