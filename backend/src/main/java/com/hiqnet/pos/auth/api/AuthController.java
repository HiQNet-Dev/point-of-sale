package com.hiqnet.pos.auth.api;

import com.hiqnet.pos.auth.api.AuthResponses.MeResponse;
import com.hiqnet.pos.auth.api.AuthResponses.TokenResponse;
import com.hiqnet.pos.auth.application.AuthService;
import com.hiqnet.pos.auth.application.SelfBranchService;
import com.hiqnet.pos.branch.api.BranchResponse;
import com.hiqnet.pos.branch.domain.Branch;
import com.hiqnet.pos.branch.domain.BranchStatus;
import com.hiqnet.pos.shared.api.PageResponse;
import com.hiqnet.pos.shared.error.BusinessException;
import com.hiqnet.pos.shared.error.ErrorCodes;
import io.swagger.v3.oas.annotations.Operation;
import io.swagger.v3.oas.annotations.media.Content;
import io.swagger.v3.oas.annotations.responses.ApiResponse;
import io.swagger.v3.oas.annotations.responses.ApiResponses;
import io.swagger.v3.oas.annotations.security.SecurityRequirement;
import io.swagger.v3.oas.annotations.security.SecurityRequirements;
import io.swagger.v3.oas.annotations.tags.Tag;
import jakarta.validation.Valid;
import org.springframework.data.domain.Page;
import org.springframework.data.domain.PageRequest;
import org.springframework.data.domain.Sort;
import org.springframework.http.HttpStatus;
import org.springframework.security.core.annotation.AuthenticationPrincipal;
import org.springframework.security.oauth2.jwt.Jwt;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.RequestBody;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RequestParam;
import org.springframework.web.bind.annotation.ResponseStatus;
import org.springframework.web.bind.annotation.RestController;

/**
 * Authentication endpoints. There is deliberately NO public registration,
 * password reset or user administration in this task: identities are created
 * by the future users module.
 */
@RestController
@RequestMapping("/api/v1/auth")
@Tag(name = "Authentication", description = "Inicio y ciclo de vida de la sesión (login, refresh, logout) y la identidad autenticada.")
public class AuthController {

    private static final int MAX_PAGE_SIZE = 100;

    /** Deterministic order: name ASC, id ASC as tiebreaker. */
    private static final Sort BRANCH_ORDER = Sort.by(Sort.Order.asc("name"), Sort.Order.asc("id"));

    private final AuthService authService;
    private final SelfBranchService selfBranchService;

    public AuthController(AuthService authService, SelfBranchService selfBranchService) {
        this.authService = authService;
        this.selfBranchService = selfBranchService;
    }

    @PostMapping("/login")
    @Operation(summary = "Authenticate with organization code + email + password")
    @ApiResponses({
        @ApiResponse(responseCode = "200", description = "Access + refresh tokens issued", content = @Content),
        @ApiResponse(responseCode = "400", description = "VALIDATION_ERROR / MALFORMED_REQUEST", content = @Content),
        @ApiResponse(responseCode = "401", description = "INVALID_CREDENTIALS (generic for any credential failure)", content = @Content)
    })
    @SecurityRequirements
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
    @SecurityRequirements
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
    @SecurityRequirements
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

    @GetMapping("/me/branches")
    @Operation(
            summary = "Branches reachable by the current identity (paginated; filters status, q)",
            description = "Self-scoped discovery: branches of the user's own organization that the "
                    + "user is an ACTIVE member of, or every branch when the user holds branch.read "
                    + "org-wide. Visibility only; it grants no operational authorization.")
    @ApiResponses({
        @ApiResponse(responseCode = "200", description = "Visible branches (empty page when none)", content = @Content),
        @ApiResponse(responseCode = "400", description = "Invalid paging parameters (VALIDATION_ERROR)", content = @Content),
        @ApiResponse(responseCode = "401", description = "TOKEN_INVALID / TOKEN_EXPIRED / AUTHENTICATION_REQUIRED", content = @Content)
    })
    @SecurityRequirement(name = "bearerAuth")
    public PageResponse<BranchResponse> myBranches(
            @AuthenticationPrincipal Jwt jwt,
            @RequestParam(defaultValue = "0") int page,
            @RequestParam(defaultValue = "20") int size,
            @RequestParam(required = false) BranchStatus status,
            @RequestParam(required = false) String q) {
        validatePaging(page, size);
        Page<Branch> branches =
                selfBranchService.listMyBranches(userIdFrom(jwt), status, q, PageRequest.of(page, size, BRANCH_ORDER));
        return PageResponse.from(branches, BranchResponse::from);
    }

    private static void validatePaging(int page, int size) {
        if (page < 0) {
            throw new BusinessException(HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, "page must be >= 0");
        }
        if (size < 1 || size > MAX_PAGE_SIZE) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST,
                    ErrorCodes.VALIDATION_ERROR,
                    "size must be between 1 and " + MAX_PAGE_SIZE);
        }
    }

    private static java.util.UUID userIdFrom(Jwt jwt) {
        return java.util.UUID.fromString(jwt.getSubject());
    }
}