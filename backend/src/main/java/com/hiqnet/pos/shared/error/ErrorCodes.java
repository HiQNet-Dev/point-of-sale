package com.hiqnet.pos.shared.error;

/**
 * Stable public error codes used in {@link ApiError}. Clients may rely on
 * these values; keep them stable once released.
 */
public final class ErrorCodes {

    private ErrorCodes() {
    }

    public static final String VALIDATION_ERROR = "VALIDATION_ERROR";
    public static final String MALFORMED_REQUEST = "MALFORMED_REQUEST";
    public static final String RESOURCE_NOT_FOUND = "RESOURCE_NOT_FOUND";
    public static final String UNAUTHORIZED = "UNAUTHORIZED";
    public static final String FORBIDDEN = "FORBIDDEN";
    public static final String INTERNAL_ERROR = "INTERNAL_ERROR";
    public static final String CONCURRENT_MODIFICATION = "CONCURRENT_MODIFICATION";

    public static final String ORGANIZATION_NOT_FOUND = "ORGANIZATION_NOT_FOUND";
    public static final String ORGANIZATION_CODE_ALREADY_EXISTS = "ORGANIZATION_CODE_ALREADY_EXISTS";
    public static final String BRANCH_NOT_FOUND = "BRANCH_NOT_FOUND";
    public static final String BRANCH_CODE_ALREADY_EXISTS = "BRANCH_CODE_ALREADY_EXISTS";

    public static final String INVALID_CODE = "INVALID_CODE";
    public static final String INVALID_TIMEZONE = "INVALID_TIMEZONE";
    public static final String INVALID_CURRENCY = "INVALID_CURRENCY";
    public static final String INVALID_LOCALE = "INVALID_LOCALE";
}