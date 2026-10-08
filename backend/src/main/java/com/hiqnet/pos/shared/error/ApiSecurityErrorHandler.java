package com.hiqnet.pos.shared.error;

import jakarta.servlet.http.HttpServletRequest;
import jakarta.servlet.http.HttpServletResponse;
import java.io.IOException;
import java.util.Locale;
import org.springframework.http.MediaType;
import org.springframework.security.access.AccessDeniedException;
import org.springframework.security.core.AuthenticationException;
import org.springframework.security.oauth2.jwt.JwtValidationException;
import org.springframework.security.oauth2.server.resource.InvalidBearerTokenException;
import org.springframework.security.web.AuthenticationEntryPoint;
import org.springframework.security.web.access.AccessDeniedHandler;
import tools.jackson.databind.ObjectMapper;

/**
 * Writes the public {@link ApiError} contract for failures raised by the
 * security filter chain. Authentication failures map to stable codes:
 * missing credentials -> AUTHENTICATION_REQUIRED; a rejected bearer token ->
 * TOKEN_INVALID or TOKEN_EXPIRED (expiry is detected from the structured
 * JWT validation errors, no internal details are ever exposed).
 */
public class ApiSecurityErrorHandler implements AuthenticationEntryPoint, AccessDeniedHandler {

    private final ObjectMapper objectMapper;

    public ApiSecurityErrorHandler(ObjectMapper objectMapper) {
        this.objectMapper = objectMapper;
    }

    @Override
    public void commence(
            HttpServletRequest request, HttpServletResponse response, AuthenticationException authException)
            throws IOException {
        if (authException instanceof InvalidBearerTokenException bearerException) {
            String code = isExpiryError(bearerException)
                    ? ErrorCodes.TOKEN_EXPIRED
                    : ErrorCodes.TOKEN_INVALID;
            // Keep the RFC 6750 challenge that the default bearer entry point
            // would have set, now that this handler owns the response.
            response.setHeader("WWW-Authenticate", "Bearer");
            write(response, HttpServletResponse.SC_UNAUTHORIZED, code, "Invalid access token", request);
            return;
        }
        write(
                response,
                HttpServletResponse.SC_UNAUTHORIZED,
                ErrorCodes.AUTHENTICATION_REQUIRED,
                "Authentication required",
                request);
    }

    /**
     * Expiry is reported through the structured validation exception
     * (preferred) or, as a fallback, the root cause message. Both are
     * matched on the stable "expired" marker with case-insensitive
     * comparison.
     */
    private boolean isExpiryError(InvalidBearerTokenException ex) {
        String text = "";
        Throwable cause = ex.getCause();
        if (cause instanceof JwtValidationException jwtValidation) {
            text = jwtValidation.getMessage() == null ? "" : jwtValidation.getMessage();
        } else if (cause != null && cause.getMessage() != null) {
            text = cause.getMessage();
        }
        return text.toLowerCase(Locale.ROOT).contains("expired");
    }

    @Override
    public void handle(
            HttpServletRequest request, HttpServletResponse response, AccessDeniedException accessDeniedException)
            throws IOException {
        write(response, HttpServletResponse.SC_FORBIDDEN, ErrorCodes.FORBIDDEN, "Access denied", request);
    }

    private void write(
            HttpServletResponse response, int status, String code, String message, HttpServletRequest request)
            throws IOException {
        response.setStatus(status);
        response.setContentType(MediaType.APPLICATION_JSON_VALUE);
        objectMapper.writeValue(
                response.getOutputStream(),
                ApiError.of(status, code, message, request.getRequestURI()));
    }
}