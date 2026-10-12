package com.hiqnet.pos.shared.error;

import org.springframework.dao.DataIntegrityViolationException;
import org.springframework.http.HttpStatus;

/**
 * Translates real database integrity violations into the public error contract.
 * PostgreSQL (via Hibernate or the JDBC driver) is the final authority on
 * uniqueness: the pre-checks in the services are UX-only and cannot close the
 * concurrent race. The constraint name is read from structured exception
 * metadata when available; SQL messages are never parsed, shown or logged
 * beyond the framework's own logging.
 */
public final class ConstraintTranslator {

    private ConstraintTranslator() {
    }

    public static final String ORGANIZATIONS_CODE_UNIQUE = "organizations_code_unique";
    public static final String BRANCHES_ORGANIZATION_CODE_UNIQUE = "branches_organization_code_unique";
    public static final String USERS_ORGANIZATION_EMAIL_UNIQUE = "users_organization_email_unique";
    public static final String USERS_EMAIL_NORMALIZED_CHECK = "users_email_normalized_check";
    public static final String ROLES_ORGANIZATION_CODE_UNIQUE = "roles_organization_code_unique";
    public static final String BRANCH_MEMBERSHIPS_ORG_BRANCH_USER_UNIQUE = "branch_memberships_org_branch_user_unique";
    public static final String CATEGORIES_ORGANIZATION_CODE_UNIQUE = "categories_organization_code_unique";
    public static final String PRODUCTS_ORGANIZATION_SKU_UNIQUE = "products_organization_sku_unique";
    public static final String PRODUCTS_ORGANIZATION_BARCODE_UNIQUE = "products_organization_barcode_unique";
    public static final String MODIFIER_GROUPS_ORGANIZATION_CODE_UNIQUE = "modifier_groups_organization_code_unique";
    public static final String MODIFIER_OPTIONS_GROUP_CODE_UNIQUE = "modifier_options_group_code_unique";
    public static final String INVENTORY_ITEMS_ORGANIZATION_CODE_UNIQUE = "inventory_items_organization_code_unique";
    public static final String CASH_REGISTERS_BRANCH_CODE_UNIQUE = "cash_registers_branch_code_unique";
    public static final String CASH_SHIFTS_ONE_OPEN_PER_REGISTER = "cash_shifts_one_open_per_register";

    /**
     * Returns the mapped {@link BusinessException} for a known constraint, or
     * {@code null} when the violation is not a known duplicate-code case.
     * Unknown violations are never guessed as duplicates: they bubble up as a
     * generic 500 in {@link GlobalExceptionHandler}.
     */
    public static BusinessException translate(DataIntegrityViolationException violation) {
        String constraintName = findConstraintName(violation);
        if (constraintName == null) {
            return null;
        }
        return switch (constraintName) {
            case ORGANIZATIONS_CODE_UNIQUE -> new BusinessException(
                    HttpStatus.CONFLICT,
                    ErrorCodes.ORGANIZATION_CODE_ALREADY_EXISTS,
                    "An organization with this code already exists");
            case BRANCHES_ORGANIZATION_CODE_UNIQUE -> new BusinessException(
                    HttpStatus.CONFLICT,
                    ErrorCodes.BRANCH_CODE_ALREADY_EXISTS,
                    "A branch with this code already exists in the organization");
            case USERS_ORGANIZATION_EMAIL_UNIQUE -> new BusinessException(
                    HttpStatus.CONFLICT,
                    ErrorCodes.USER_EMAIL_ALREADY_EXISTS,
                    "A user with this email already exists in the organization");
            case USERS_EMAIL_NORMALIZED_CHECK -> new BusinessException(
                    HttpStatus.BAD_REQUEST,
                    ErrorCodes.VALIDATION_ERROR,
                    "email must be normalized (trimmed, lowercase, non-empty)");
            case ROLES_ORGANIZATION_CODE_UNIQUE -> new BusinessException(
                    HttpStatus.CONFLICT,
                    ErrorCodes.ROLE_CODE_ALREADY_EXISTS,
                    "A role with this code already exists in the organization");
            case BRANCH_MEMBERSHIPS_ORG_BRANCH_USER_UNIQUE -> new BusinessException(
                    HttpStatus.CONFLICT,
                    ErrorCodes.BRANCH_MEMBERSHIP_ALREADY_EXISTS,
                    "The user already has a membership in this branch");
            case CATEGORIES_ORGANIZATION_CODE_UNIQUE -> new BusinessException(
                    HttpStatus.CONFLICT,
                    ErrorCodes.CATEGORY_CODE_ALREADY_EXISTS,
                    "A category with this code already exists in the organization");
            case PRODUCTS_ORGANIZATION_SKU_UNIQUE -> new BusinessException(
                    HttpStatus.CONFLICT,
                    ErrorCodes.PRODUCT_SKU_ALREADY_EXISTS,
                    "A product with this sku already exists in the organization");
            case PRODUCTS_ORGANIZATION_BARCODE_UNIQUE -> new BusinessException(
                    HttpStatus.CONFLICT,
                    ErrorCodes.PRODUCT_BARCODE_ALREADY_EXISTS,
                    "A product with this barcode already exists in the organization");
            case MODIFIER_GROUPS_ORGANIZATION_CODE_UNIQUE -> new BusinessException(
                    HttpStatus.CONFLICT,
                    ErrorCodes.MODIFIER_GROUP_CODE_ALREADY_EXISTS,
                    "A modifier group with this code already exists in the organization");
            case MODIFIER_OPTIONS_GROUP_CODE_UNIQUE -> new BusinessException(
                    HttpStatus.CONFLICT,
                    ErrorCodes.MODIFIER_OPTION_CODE_ALREADY_EXISTS,
                    "A modifier option with this code already exists in the group");
            case INVENTORY_ITEMS_ORGANIZATION_CODE_UNIQUE -> new BusinessException(
                    HttpStatus.CONFLICT,
                    ErrorCodes.INVENTORY_ITEM_CODE_ALREADY_EXISTS,
                    "An inventory item with this code already exists in the organization");
            case CASH_REGISTERS_BRANCH_CODE_UNIQUE -> new BusinessException(
                    HttpStatus.CONFLICT,
                    ErrorCodes.CASH_REGISTER_CODE_ALREADY_EXISTS,
                    "A cash register with this code already exists in the branch");
            case CASH_SHIFTS_ONE_OPEN_PER_REGISTER -> new BusinessException(
                    HttpStatus.CONFLICT,
                    ErrorCodes.CASH_REGISTER_ALREADY_HAS_OPEN_SHIFT,
                    "The cash register already has an open cash shift");
            default -> null;
        };
    }

    private static String findConstraintName(Throwable cause) {
        for (Throwable current = cause; current != null; current = current.getCause()) {
            if (current instanceof org.hibernate.exception.ConstraintViolationException hibernate
                    && hibernate.getConstraintName() != null) {
                return hibernate.getConstraintName();
            }
        }
        return null;
    }
}