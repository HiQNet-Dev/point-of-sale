package com.hiqnet.pos.shared.error;

import org.springframework.http.HttpStatus;

/**
 * Business rule violation with a stable public error code and HTTP status.
 * The API layer maps it to {@link ApiError}; no internal details leak.
 */
public class BusinessException extends RuntimeException {

    private final HttpStatus status;
    private final String code;

    public BusinessException(HttpStatus status, String code, String message) {
        super(message);
        this.status = status;
        this.code = code;
    }

    public HttpStatus getStatus() {
        return status;
    }

    public String getCode() {
        return code;
    }
}