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
    public static final String SUPPLIER_NOT_FOUND = "SUPPLIER_NOT_FOUND";
    public static final String SUPPLIER_CODE_ALREADY_EXISTS = "SUPPLIER_CODE_ALREADY_EXISTS";
    public static final String SUPPLIER_INACTIVE = "SUPPLIER_INACTIVE";

    public static final String PURCHASE_ORDER_NOT_FOUND = "PURCHASE_ORDER_NOT_FOUND";
    public static final String PURCHASE_ORDER_CODE_ALREADY_EXISTS = "PURCHASE_ORDER_CODE_ALREADY_EXISTS";
    public static final String PURCHASE_ORDER_LINE_NOT_FOUND = "PURCHASE_ORDER_LINE_NOT_FOUND";
    public static final String PURCHASE_ORDER_LINE_DUPLICATE_ITEM = "PURCHASE_ORDER_LINE_DUPLICATE_ITEM";
    public static final String PURCHASE_ORDER_NOT_DRAFT = "PURCHASE_ORDER_NOT_DRAFT";
    public static final String PURCHASE_ORDER_EMPTY = "PURCHASE_ORDER_EMPTY";
    public static final String PURCHASE_ORDER_UNIT_CHANGED = "PURCHASE_ORDER_UNIT_CHANGED";
    public static final String PURCHASE_ORDER_ALREADY_CANCELLED = "PURCHASE_ORDER_ALREADY_CANCELLED";
    public static final String PURCHASE_ORDER_NOT_ISSUED = "PURCHASE_ORDER_NOT_ISSUED";
    public static final String PURCHASE_ORDER_NOT_RECEIVABLE = "PURCHASE_ORDER_NOT_RECEIVABLE";
    public static final String PURCHASE_ORDER_ALREADY_RECEIVED = "PURCHASE_ORDER_ALREADY_RECEIVED";
    public static final String PURCHASE_ORDER_HAS_RECEIPTS = "PURCHASE_ORDER_HAS_RECEIPTS";
    public static final String PURCHASE_RECEIPT_NOT_FOUND = "PURCHASE_RECEIPT_NOT_FOUND";
    public static final String PURCHASE_RECEIPT_EXCEEDS_ORDERED = "PURCHASE_RECEIPT_EXCEEDS_ORDERED";
    public static final String PURCHASE_RECEIPT_IDEMPOTENCY_CONFLICT = "PURCHASE_RECEIPT_IDEMPOTENCY_CONFLICT";

    public static final String INVENTORY_ITEM_NOT_FOUND = "INVENTORY_ITEM_NOT_FOUND";
    public static final String INVENTORY_ITEM_CODE_ALREADY_EXISTS = "INVENTORY_ITEM_CODE_ALREADY_EXISTS";
    public static final String INVALID_INVENTORY_QUANTITY = "INVALID_INVENTORY_QUANTITY";
    public static final String INVENTORY_OPENING_ALREADY_EXISTS = "INVENTORY_OPENING_ALREADY_EXISTS";
    public static final String INVENTORY_HISTORY_ALREADY_EXISTS = "INVENTORY_HISTORY_ALREADY_EXISTS";
    public static final String INVENTORY_ITEM_INACTIVE = "INVENTORY_ITEM_INACTIVE";
    public static final String INVENTORY_ITEM_UNIT_LOCKED = "INVENTORY_ITEM_UNIT_LOCKED";
    public static final String INVENTORY_RECIPE_NOT_FOUND = "INVENTORY_RECIPE_NOT_FOUND";
    public static final String INVENTORY_MODIFIER_CONFIG_NOT_FOUND = "INVENTORY_MODIFIER_CONFIG_NOT_FOUND";
    public static final String INVENTORY_CONFIGURATION_UNAVAILABLE = "INVENTORY_CONFIGURATION_UNAVAILABLE";

    public static final String CASH_REGISTER_NOT_FOUND = "CASH_REGISTER_NOT_FOUND";
    public static final String CASH_REGISTER_CODE_ALREADY_EXISTS = "CASH_REGISTER_CODE_ALREADY_EXISTS";
    public static final String CASH_REGISTER_INACTIVE = "CASH_REGISTER_INACTIVE";

    public static final String CASH_SHIFT_NOT_FOUND = "CASH_SHIFT_NOT_FOUND";
    public static final String CASH_REGISTER_ALREADY_HAS_OPEN_SHIFT = "CASH_REGISTER_ALREADY_HAS_OPEN_SHIFT";
    public static final String CASH_SHIFT_ALREADY_CLOSED = "CASH_SHIFT_ALREADY_CLOSED";
    public static final String CASH_SHIFT_HAS_OPEN_SALES = "CASH_SHIFT_HAS_OPEN_SALES";
    public static final String CASH_SHIFT_NOT_OPEN = "CASH_SHIFT_NOT_OPEN";

    public static final String SALE_NOT_FOUND = "SALE_NOT_FOUND";
    public static final String SALE_NOT_OPEN = "SALE_NOT_OPEN";
    public static final String SALE_ALREADY_CANCELLED = "SALE_ALREADY_CANCELLED";
    public static final String SALE_ITEM_NOT_FOUND = "SALE_ITEM_NOT_FOUND";
    public static final String SALE_ITEM_ALREADY_REMOVED = "SALE_ITEM_ALREADY_REMOVED";
    public static final String PRODUCT_INACTIVE = "PRODUCT_INACTIVE";
    public static final String PRODUCT_MODIFIER_CONFIGURATION_UNAVAILABLE = "PRODUCT_MODIFIER_CONFIGURATION_UNAVAILABLE";
    public static final String INVALID_MODIFIER_SELECTION = "INVALID_MODIFIER_SELECTION";
    public static final String NEGATIVE_EFFECTIVE_PRICE = "NEGATIVE_EFFECTIVE_PRICE";
    public static final String SALE_ALREADY_COMPLETED = "SALE_ALREADY_COMPLETED";
    public static final String SALE_HAS_NO_ACTIVE_ITEMS = "SALE_HAS_NO_ACTIVE_ITEMS";
    public static final String INSUFFICIENT_INVENTORY = "INSUFFICIENT_INVENTORY";
    public static final String INVENTORY_SNAPSHOT_INCONSISTENT = "INVENTORY_SNAPSHOT_INCONSISTENT";
    public static final String PAYMENT_TOTAL_MISMATCH = "PAYMENT_TOTAL_MISMATCH";
    public static final String INVALID_PAYMENT = "INVALID_PAYMENT";
    public static final String INVALID_CASH_TENDERED_AMOUNT = "INVALID_CASH_TENDERED_AMOUNT";
    public static final String TOO_MANY_PAYMENTS = "TOO_MANY_PAYMENTS";
    public static final String MULTIPLE_CASH_PAYMENTS_NOT_ALLOWED = "MULTIPLE_CASH_PAYMENTS_NOT_ALLOWED";

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