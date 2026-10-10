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
    public static final String ORGANIZATION_INACTIVE = "ORGANIZATION_INACTIVE";
    public static final String ORGANIZATION_CODE_ALREADY_EXISTS = "ORGANIZATION_CODE_ALREADY_EXISTS";
    public static final String BRANCH_NOT_FOUND = "BRANCH_NOT_FOUND";
    public static final String BRANCH_CODE_ALREADY_EXISTS = "BRANCH_CODE_ALREADY_EXISTS";
    public static final String BRANCH_INACTIVE = "BRANCH_INACTIVE";

    public static final String BRANCH_MEMBERSHIP_NOT_FOUND = "BRANCH_MEMBERSHIP_NOT_FOUND";
    public static final String BRANCH_MEMBERSHIP_ALREADY_EXISTS = "BRANCH_MEMBERSHIP_ALREADY_EXISTS";

    public static final String CATEGORY_NOT_FOUND = "CATEGORY_NOT_FOUND";
    public static final String CATEGORY_CODE_ALREADY_EXISTS = "CATEGORY_CODE_ALREADY_EXISTS";
    public static final String CATEGORY_INACTIVE = "CATEGORY_INACTIVE";
    public static final String PRODUCT_NOT_FOUND = "PRODUCT_NOT_FOUND";
    public static final String PRODUCT_SKU_ALREADY_EXISTS = "PRODUCT_SKU_ALREADY_EXISTS";
    public static final String PRODUCT_BARCODE_ALREADY_EXISTS = "PRODUCT_BARCODE_ALREADY_EXISTS";

    public static final String MODIFIER_GROUP_NOT_FOUND = "MODIFIER_GROUP_NOT_FOUND";
    public static final String MODIFIER_GROUP_CODE_ALREADY_EXISTS = "MODIFIER_GROUP_CODE_ALREADY_EXISTS";
    public static final String MODIFIER_OPTION_NOT_FOUND = "MODIFIER_OPTION_NOT_FOUND";
    public static final String MODIFIER_OPTION_CODE_ALREADY_EXISTS = "MODIFIER_OPTION_CODE_ALREADY_EXISTS";

    public static final String CASH_REGISTER_NOT_FOUND = "CASH_REGISTER_NOT_FOUND";
    public static final String CASH_REGISTER_CODE_ALREADY_EXISTS = "CASH_REGISTER_CODE_ALREADY_EXISTS";

    public static final String USER_NOT_FOUND = "USER_NOT_FOUND";
    public static final String USER_EMAIL_ALREADY_EXISTS = "USER_EMAIL_ALREADY_EXISTS";
    public static final String WEAK_PASSWORD = "WEAK_PASSWORD";
    public static final String USER_INACTIVE = "USER_INACTIVE";

    public static final String ROLE_NOT_FOUND = "ROLE_NOT_FOUND";
    public static final String ROLE_CODE_ALREADY_EXISTS = "ROLE_CODE_ALREADY_EXISTS";
    public static final String PERMISSION_NOT_FOUND = "PERMISSION_NOT_FOUND";

    public static final String INVALID_CODE = "INVALID_CODE";
    public static final String INVALID_TIMEZONE = "INVALID_TIMEZONE";
    public static final String INVALID_CURRENCY = "INVALID_CURRENCY";
    public static final String INVALID_LOCALE = "INVALID_LOCALE";

    public static final String INVALID_CREDENTIALS = "INVALID_CREDENTIALS";
    public static final String INVALID_REFRESH_TOKEN = "INVALID_REFRESH_TOKEN";
    public static final String AUTHENTICATION_REQUIRED = "AUTHENTICATION_REQUIRED";
    public static final String TOKEN_INVALID = "TOKEN_INVALID";
    public static final String TOKEN_EXPIRED = "TOKEN_EXPIRED";

    public static final String BOOTSTRAP_INVALID_TOKEN = "BOOTSTRAP_INVALID_TOKEN";
    public static final String BOOTSTRAP_ALREADY_COMPLETED = "BOOTSTRAP_ALREADY_COMPLETED";
    public static final String BOOTSTRAP_NOT_ALLOWED = "BOOTSTRAP_NOT_ALLOWED";
}