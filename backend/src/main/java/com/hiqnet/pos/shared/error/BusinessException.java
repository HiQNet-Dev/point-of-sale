package com.hiqnet.pos.shared.error;

import java.util.List;
import org.springframework.http.HttpStatus;

/**
 * Business rule violation with a stable public error code and HTTP status.
 * The API layer maps it to {@link ApiError}; no internal details leak. Optional
 * {@code details} carry safe, client-facing field context (never internals).
 */
public class BusinessException extends RuntimeException {

    private final HttpStatus status;
    private final String code;
    private final List<String> details;

    public BusinessException(HttpStatus status, String code, String message) {
        this(status, code, message, List.of());
    }

    public BusinessException(HttpStatus status, String code, String message, List<String> details) {
        super(message);
        this.status = status;
        this.code = code;
        this.details = details == null ? List.of() : List.copyOf(details);
    }

    public HttpStatus getStatus() {
        return status;
    }

    public String getCode() {
        return code;
    }

    public List<String> getDetails() {
        return details;
    }
}