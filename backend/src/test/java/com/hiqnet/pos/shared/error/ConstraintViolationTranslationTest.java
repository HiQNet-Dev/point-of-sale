package com.hiqnet.pos.shared.error;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNull;

import java.sql.SQLException;
import org.hibernate.exception.ConstraintViolationException;
import org.junit.jupiter.api.Test;
import org.springframework.dao.DataIntegrityViolationException;
import org.springframework.http.HttpStatus;

/**
 * Race-safe uniqueness relies on the database constraints: the service
 * pre-checks cannot close the concurrent window, so the real constraint names
 * must map to the stable duplicate-code errors. Unknown violations must never
 * be guessed as duplicates (null result -> generic 500 upstream).
 */
class ConstraintViolationTranslationTest {

    private DataIntegrityViolationException violation(String constraintName) {
        SQLException sql = new SQLException("duplicate key value violates unique constraint \"" + constraintName + "\"", "23505");
        ConstraintViolationException hibernate =
                new ConstraintViolationException("could not execute statement", sql, constraintName);
        return new DataIntegrityViolationException("StatementCallback; SQL [INSERT ...]", hibernate);
    }

    @Test
    void organizationCodeConstraintMapsToConflictWithStableCode() {
        BusinessException translated = ConstraintTranslator.translate(violation("organizations_code_unique"));

        assertEquals(HttpStatus.CONFLICT, translated.getStatus());
        assertEquals(ErrorCodes.ORGANIZATION_CODE_ALREADY_EXISTS, translated.getCode());
    }

    @Test
    void branchOrganizationCodeConstraintMapsToConflictWithStableCode() {
        BusinessException translated = ConstraintTranslator.translate(violation("branches_organization_code_unique"));

        assertEquals(HttpStatus.CONFLICT, translated.getStatus());
        assertEquals(ErrorCodes.BRANCH_CODE_ALREADY_EXISTS, translated.getCode());
    }

    @Test
    void unknownConstraintIsNotGuessedAsDuplicate() {
        assertNull(ConstraintTranslator.translate(violation("branches_some_other_constraint")));
    }

    @Test
    void violationWithoutConstraintMetadataIsNotGuessedAsDuplicate() {
        DataIntegrityViolationException generic =
                new DataIntegrityViolationException("Could not execute statement", new SQLException("db failure", "23505"));
        assertNull(ConstraintTranslator.translate(generic));
    }
}