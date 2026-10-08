package com.hiqnet.pos.shared.error;

import jakarta.servlet.http.HttpServletRequest;
import java.util.List;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.dao.DataIntegrityViolationException;
import org.springframework.dao.OptimisticLockingFailureException;
import org.springframework.http.HttpStatus;
import org.springframework.http.ResponseEntity;
import org.springframework.http.converter.HttpMessageNotReadableException;
import org.springframework.web.bind.MethodArgumentNotValidException;
import org.springframework.web.bind.annotation.ExceptionHandler;
import org.springframework.web.bind.annotation.RestControllerAdvice;
import org.springframework.web.method.annotation.MethodArgumentTypeMismatchException;
import org.springframework.web.servlet.resource.NoResourceFoundException;

/**
 * Maps internal exceptions to the public {@link ApiError} contract.
 * Internal messages and stack traces are logged, never returned to clients.
 */
@RestControllerAdvice
public class GlobalExceptionHandler {

    private static final Logger log = LoggerFactory.getLogger(GlobalExceptionHandler.class);

    @ExceptionHandler(MethodArgumentNotValidException.class)
    public ResponseEntity<ApiError> handleValidation(
            MethodArgumentNotValidException ex, HttpServletRequest request) {
        List<String> details = ex.getBindingResult().getFieldErrors().stream()
                .map(error -> error.getField() + ": " + error.getDefaultMessage())
                .sorted()
                .toList();
        return ResponseEntity.badRequest().body(ApiError.of(
                HttpStatus.BAD_REQUEST.value(),
                "VALIDATION_ERROR",
                "Request validation failed",
                request.getRequestURI(),
                details));
    }

    @ExceptionHandler(HttpMessageNotReadableException.class)
    public ResponseEntity<ApiError> handleUnreadableBody(
            HttpMessageNotReadableException ex, HttpServletRequest request) {
        return ResponseEntity.badRequest().body(ApiError.of(
                HttpStatus.BAD_REQUEST.value(),
                "MALFORMED_REQUEST",
                "Malformed request body",
                request.getRequestURI()));
    }

    @ExceptionHandler(BusinessException.class)
    public ResponseEntity<ApiError> handleBusinessException(BusinessException ex, HttpServletRequest request) {
        return ResponseEntity.status(ex.getStatus()).body(ApiError.of(
                ex.getStatus().value(),
                ex.getCode(),
                ex.getMessage(),
                request.getRequestURI()));
    }

    /**
     * PostgreSQL is the final authority on uniqueness; a concurrent duplicate
     * can reach the constraint before the service pre-check sees it. Known
     * unique constraints map to 409 with the stable duplicate-code error;
     * unknown integrity violations are logged and returned as a generic 500,
     * never guessed as duplicates.
     */
    @ExceptionHandler(DataIntegrityViolationException.class)
    public ResponseEntity<ApiError> handleDataIntegrityViolation(
            DataIntegrityViolationException ex, HttpServletRequest request) {
        BusinessException mapped = ConstraintTranslator.translate(ex);
        if (mapped != null) {
            return ResponseEntity.status(mapped.getStatus()).body(ApiError.of(
                    mapped.getStatus().value(),
                    mapped.getCode(),
                    mapped.getMessage(),
                    request.getRequestURI()));
        }
        log.error("Unhandled data integrity violation on {} {}", request.getMethod(), request.getRequestURI(), ex);
        return ResponseEntity.internalServerError().body(ApiError.of(
                HttpStatus.INTERNAL_SERVER_ERROR.value(),
                ErrorCodes.INTERNAL_ERROR,
                "An unexpected error occurred",
                request.getRequestURI()));
    }

    @ExceptionHandler(OptimisticLockingFailureException.class)
    public ResponseEntity<ApiError> handleConcurrentModification(
            OptimisticLockingFailureException ex, HttpServletRequest request) {
        return ResponseEntity.status(HttpStatus.CONFLICT).body(ApiError.of(
                HttpStatus.CONFLICT.value(),
                ErrorCodes.CONCURRENT_MODIFICATION,
                "The resource was modified concurrently; retry with fresh data",
                request.getRequestURI()));
    }

    @ExceptionHandler(MethodArgumentTypeMismatchException.class)
    public ResponseEntity<ApiError> handleTypeMismatch(
            MethodArgumentTypeMismatchException ex, HttpServletRequest request) {
        return ResponseEntity.badRequest().body(ApiError.of(
                HttpStatus.BAD_REQUEST.value(),
                ErrorCodes.VALIDATION_ERROR,
                "Invalid path parameter value",
                request.getRequestURI()));
    }

    @ExceptionHandler(NoResourceFoundException.class)
    public ResponseEntity<ApiError> handleNotFound(
            NoResourceFoundException ex, HttpServletRequest request) {
        return ResponseEntity.status(HttpStatus.NOT_FOUND).body(ApiError.of(
                HttpStatus.NOT_FOUND.value(),
                "RESOURCE_NOT_FOUND",
                "Resource not found",
                request.getRequestURI()));
    }

    @ExceptionHandler(Exception.class)
    public ResponseEntity<ApiError> handleUnexpected(Exception ex, HttpServletRequest request) {
        log.error("Unhandled exception on {} {}", request.getMethod(), request.getRequestURI(), ex);
        return ResponseEntity.internalServerError().body(ApiError.of(
                HttpStatus.INTERNAL_SERVER_ERROR.value(),
                "INTERNAL_ERROR",
                "An unexpected error occurred",
                request.getRequestURI()));
    }
}
