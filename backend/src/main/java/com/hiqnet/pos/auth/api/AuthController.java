package com.hiqnet.pos.auth.api;

import com.hiqnet.pos.auth.api.AuthResponses.MeResponse;
import com.hiqnet.pos.auth.api.AuthResponses.TokenResponse;
import com.hiqnet.pos.auth.application.AuthService;
import io.swagger.v3.oas.annotations.Operation;
import io.swagger.v3.oas.annotations.media.Content;
import io.swagger.v3.oas.annotations.responses.ApiResponse;
import io.swagger.v3.oas.annotations.responses.ApiResponses;
import io.swagger.v3.oas.annotations.security.SecurityRequirement;
import jakarta.validation.Valid;
import org.springframework.http.HttpStatus;
import org.springframework.security.core.annotation.AuthenticationPrincipal;
import org.springframework.security.oauth2.jwt.Jwt;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.RequestBody;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.ResponseStatus;
import org.springframework.web.bind.annotation.RestController;

/**
 * Authentication endpoints. There is deliberately NO public registration,
 * password reset or user administration in this task: identities are created
 * by the future users module.
 */
@RestController
@RequestMapping("/api/v1/auth")
public class AuthController {

    private final AuthService authService;

    public AuthController(AuthService authService) {
        this.authService = authService;
    }

    @PostMapping("/login")
    @Operation(summary = "Authenticate with organization code + email + password")
    @ApiResponses({
        @ApiResponse(responseCode = "200", description = "Access + refresh tokens issued", content = @Content),
        @ApiResponse(responseCode = "400", description = "VALIDATION_ERROR / MALFORMED_REQUEST", content = @Content),
        @ApiResponse(responseCode = "401", description = "INVALID_CREDENTIALS (generic for any credential failure)", content = @Content)
    })
    public TokenResponse login(@Valid @RequestBody LoginRequest request) {
        return authService.login(request.organizationCode(), request.email(), request.password());
    }

    @PostMapping("/refresh")
    @Operation(summary = "Rotate an opaque refresh token and issue fresh tokens")
    @ApiResponses({
        @ApiResponse(responseCode = "200", description = "New access + refresh tokens (old refresh token is consumed)", content = @Content),
        @ApiResponse(responseCode = "400", description = "VALIDATION_ERROR / MALFORMED_REQUEST", content = @Content),
        @ApiResponse(responseCode = "401", description = "INVALID_REFRESH_TOKEN", content = @Content)
    })
    public TokenResponse refresh(@Valid @RequestBody RefreshRequest request) {
        return authService.refresh(request.refreshToken());
    }

    @PostMapping("/logout")
    @Operation(summary = "Revoke the current refresh session; works without an access token")
    @ApiResponses({
        @ApiResponse(responseCode = "204", description = "Revoked (idempotent; unknown tokens get the same response)"),
        @ApiResponse(responseCode = "400", description = "VALIDATION_ERROR / MALFORMED_REQUEST", content = @Content)
    })
    @ResponseStatus(HttpStatus.NO_CONTENT)
    public void logout(@Valid @RequestBody RefreshRequest request) {
        authService.logout(request.refreshToken());
    }

    @PostMapping("/logout-all")
    @Operation(
            summary = "Revoke all active refresh sessions of the authenticated user",
            description = "Already-issued access tokens are NOT revoked; they expire by their short TTL.")
    @ApiResponses({
        @ApiResponse(responseCode = "204", description = "All active refresh sessions revoked"),
        @ApiResponse(responseCode = "400", description = "VALIDATION_ERROR / MALFORMED_REQUEST", content = @Content),
        @ApiResponse(responseCode = "401", description = "TOKEN_INVALID / TOKEN_EXPIRED / AUTHENTICATION_REQUIRED", content = @Content)
    })
    @SecurityRequirement(name = "bearerAuth")
    @ResponseStatus(HttpStatus.NO_CONTENT)
    public void logoutAll(@AuthenticationPrincipal Jwt jwt) {
        authService.logoutAll(userIdFrom(jwt));
    }

    @GetMapping("/me")
    @Operation(
            summary = "Current identity",
            description = "Loads the identity from the database; client claims never drive the response.")
    @ApiResponses({
        @ApiResponse(responseCode = "200", description = "Identity loaded from the database", content = @Content),
        @ApiResponse(responseCode = "401", description = "TOKEN_INVALID / TOKEN_EXPIRED / AUTHENTICATION_REQUIRED", content = @Content)
    })
    @SecurityRequirement(name = "bearerAuth")
    public MeResponse me(@AuthenticationPrincipal Jwt jwt) {
        return authService.me(userIdFrom(jwt));
    }

    private static java.util.UUID userIdFrom(Jwt jwt) {
        return java.util.UUID.fromString(jwt.getSubject());
    }
}