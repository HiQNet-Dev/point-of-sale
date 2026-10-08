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