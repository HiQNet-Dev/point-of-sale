package com.hiqnet.pos.shared.error;

import static org.junit.jupiter.api.Assertions.assertEquals;

import org.junit.jupiter.api.Test;
import org.springframework.http.HttpStatus;
import org.springframework.http.ResponseEntity;
import org.springframework.mock.web.MockHttpServletRequest;
import org.springframework.orm.ObjectOptimisticLockingFailureException;

/**
 * Optimistic locking failures must surface as 409 CONCURRENT_MODIFICATION,
 * never 500. Unit check of the handler mapping.
 */
class ConcurrentModificationHandlerTest {

    private final GlobalExceptionHandler handler = new GlobalExceptionHandler();

    @Test
    void mapsLockFailureTo409ConflictWithStableCode() {
        MockHttpServletRequest request = new MockHttpServletRequest("GET", "/api/v1/organizations/abc/branches/def");

        ResponseEntity<ApiError> response = handler.handleConcurrentModification(
                new ObjectOptimisticLockingFailureException("x", new RuntimeException("stale")), request);

        assertEquals(HttpStatus.CONFLICT, response.getStatusCode());
        assertEquals(ErrorCodes.CONCURRENT_MODIFICATION, response.getBody().code());
        assertEquals(409, response.getBody().status());
    }
}