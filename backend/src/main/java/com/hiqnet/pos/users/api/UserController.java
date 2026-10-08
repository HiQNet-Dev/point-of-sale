package com.hiqnet.pos.users.api;

import com.hiqnet.pos.shared.api.PageResponse;
import com.hiqnet.pos.shared.error.BusinessException;
import com.hiqnet.pos.shared.error.ErrorCodes;
import com.hiqnet.pos.users.application.UpdateUserCommand;
import com.hiqnet.pos.users.application.UserService;
import com.hiqnet.pos.users.domain.User;
import com.hiqnet.pos.users.domain.UserStatus;
import io.swagger.v3.oas.annotations.Operation;
import io.swagger.v3.oas.annotations.media.Content;
import io.swagger.v3.oas.annotations.responses.ApiResponse;
import io.swagger.v3.oas.annotations.responses.ApiResponses;
import jakarta.validation.Valid;
import java.util.UUID;
import org.springframework.data.domain.Page;
import org.springframework.data.domain.PageRequest;
import org.springframework.data.domain.Sort;
import org.springframework.http.HttpStatus;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.PatchMapping;
import org.springframework.web.bind.annotation.PathVariable;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.RequestBody;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RequestParam;
import org.springframework.web.bind.annotation.ResponseStatus;
import org.springframework.web.bind.annotation.RestController;

/**
 * Organization-scoped user administration.
 *
 * <p>These endpoints are intentionally NOT reachable in the production security
 * chain yet: {@code SecurityConfig} keeps {@code anyRequest().denyAll()} until
 * Roles/Permissions (RBAC) exist, so a valid access token alone does not grant
 * user administration. The endpoints exist and are exercised only through the
 * test-only chain, exactly like {@code organization}/{@code branch}.</p>
 */
@RestController
@RequestMapping("/api/v1/organizations/{organizationId}/users")
public class UserController {

    private static final int MAX_PAGE_SIZE = 100;

    /** Deterministic order: email ASC, id ASC as tiebreaker. */
    private static final Sort ORDER = Sort.by(Sort.Order.asc("email"), Sort.Order.asc("id"));

    private final UserService userService;

    public UserController(UserService userService) {
        this.userService = userService;
    }

    @PostMapping
    @ResponseStatus(HttpStatus.CREATED)
    @Operation(summary = "Create a user inside an organization (initial password is hashed, never returned)")
    @ApiResponses({
        @ApiResponse(responseCode = "201", description = "Created", content = @Content),
        @ApiResponse(responseCode = "400", description = "Invalid email or weak password (VALIDATION_ERROR, WEAK_PASSWORD)", content = @Content),
        @ApiResponse(responseCode = "404", description = "Organization not found (ORGANIZATION_NOT_FOUND)", content = @Content),
        @ApiResponse(responseCode = "409", description = "Inactive organization or duplicate email (ORGANIZATION_INACTIVE, USER_EMAIL_ALREADY_EXISTS)", content = @Content)
    })
    public UserResponse create(
            @PathVariable UUID organizationId, @Valid @RequestBody CreateUserRequest request) {
        return UserResponse.from(userService.create(organizationId, request.email(), request.password()));
    }

    @GetMapping
    @Operation(summary = "List users of an organization (paginated, ordered by email)")
    @ApiResponses({
        @ApiResponse(responseCode = "200", description = "OK", content = @Content),
        @ApiResponse(responseCode = "400", description = "Invalid paging parameters (VALIDATION_ERROR)", content = @Content),
        @ApiResponse(responseCode = "404", description = "Organization not found (ORGANIZATION_NOT_FOUND)", content = @Content)
    })
    public PageResponse<UserResponse> list(
            @PathVariable UUID organizationId,
            @RequestParam(defaultValue = "0") int page,
            @RequestParam(defaultValue = "20") int size,
            @RequestParam(required = false) UserStatus status) {
        validatePaging(page, size);
        Page<User> users = userService.list(organizationId, status, PageRequest.of(page, size, ORDER));
        return PageResponse.from(users, UserResponse::from);
    }

    @GetMapping("/{userId}")
    @Operation(summary = "Get a user by id (scoped to the organization)")
    @ApiResponses({
        @ApiResponse(responseCode = "200", description = "OK", content = @Content),
        @ApiResponse(responseCode = "404", description = "Organization or user not found (ORGANIZATION_NOT_FOUND, USER_NOT_FOUND)", content = @Content)
    })
    public UserResponse get(@PathVariable UUID organizationId, @PathVariable UUID userId) {
        return UserResponse.from(userService.get(organizationId, userId));
    }

    @PatchMapping("/{userId}")
    @Operation(summary = "Partially update a user (email and/or status)")
    @ApiResponses({
        @ApiResponse(responseCode = "200", description = "Updated", content = @Content),
        @ApiResponse(responseCode = "400", description = "Malformed email (VALIDATION_ERROR)", content = @Content),
        @ApiResponse(responseCode = "404", description = "Organization or user not found (ORGANIZATION_NOT_FOUND, USER_NOT_FOUND)", content = @Content),
        @ApiResponse(responseCode = "409", description = "Duplicate email or concurrent modification (USER_EMAIL_ALREADY_EXISTS, CONCURRENT_MODIFICATION)", content = @Content)
    })
    public UserResponse update(
            @PathVariable UUID organizationId,
            @PathVariable UUID userId,
            @Valid @RequestBody UpdateUserRequest request) {
        return UserResponse.from(userService.update(
                organizationId, userId, new UpdateUserCommand(request.email(), request.status())));
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
}
