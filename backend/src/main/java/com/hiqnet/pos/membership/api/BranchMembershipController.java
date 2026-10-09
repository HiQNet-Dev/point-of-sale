package com.hiqnet.pos.membership.api;

import com.hiqnet.pos.membership.application.BranchMembershipService;
import com.hiqnet.pos.membership.application.UpdateMembershipCommand;
import com.hiqnet.pos.membership.domain.BranchMembership;
import com.hiqnet.pos.membership.domain.BranchMembershipStatus;
import com.hiqnet.pos.rbac.api.RoleResponse;
import com.hiqnet.pos.shared.api.PageResponse;
import com.hiqnet.pos.shared.error.BusinessException;
import com.hiqnet.pos.shared.error.ErrorCodes;
import io.swagger.v3.oas.annotations.Operation;
import io.swagger.v3.oas.annotations.media.Content;
import io.swagger.v3.oas.annotations.responses.ApiResponse;
import io.swagger.v3.oas.annotations.responses.ApiResponses;
import jakarta.validation.Valid;
import java.util.List;
import java.util.UUID;
import org.springframework.data.domain.Page;
import org.springframework.data.domain.PageRequest;
import org.springframework.data.domain.Sort;
import org.springframework.http.HttpStatus;
import org.springframework.security.access.prepost.PreAuthorize;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.PatchMapping;
import org.springframework.web.bind.annotation.PathVariable;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.PutMapping;
import org.springframework.web.bind.annotation.RequestBody;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RequestParam;
import org.springframework.web.bind.annotation.ResponseStatus;
import org.springframework.web.bind.annotation.RestController;

/**
 * Branch membership administration. All endpoints are authorized with
 * ORGANIZATION-wide permissions ({@code branch.membership.*}) — a branch-scoped
 * role can never manage memberships (anti privilege-escalation). Since one user
 * has at most one membership per branch, the single-member endpoints address
 * the membership by {@code userId} inside the branch path.
 */
@RestController
@RequestMapping("/api/v1/organizations/{organizationId}/branches/{branchId}/memberships")
public class BranchMembershipController {

    private static final int MAX_PAGE_SIZE = 100;

    /** Deterministic order: createdAt ASC, id ASC as tiebreaker. */
    private static final Sort ORDER = Sort.by(Sort.Order.asc("createdAt"), Sort.Order.asc("id"));

    private final BranchMembershipService membershipService;

    public BranchMembershipController(BranchMembershipService membershipService) {
        this.membershipService = membershipService;
    }

    @PostMapping
    @ResponseStatus(HttpStatus.CREATED)
    @Operation(summary = "Create a branch membership (user joins the branch)")
    @ApiResponses({
        @ApiResponse(responseCode = "201", description = "Created", content = @Content),
        @ApiResponse(responseCode = "400", description = "Validation (VALIDATION_ERROR)", content = @Content),
        @ApiResponse(responseCode = "404", description = "Organization, branch or user not found (ORGANIZATION_NOT_FOUND, BRANCH_NOT_FOUND, USER_NOT_FOUND)", content = @Content),
        @ApiResponse(responseCode = "409", description = "Inactive org/branch/user or duplicate (ORGANIZATION_INACTIVE, BRANCH_INACTIVE, USER_INACTIVE, BRANCH_MEMBERSHIP_ALREADY_EXISTS)", content = @Content),
        @ApiResponse(responseCode = "403", description = "Forbidden (branch.membership.create)", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasPermission(authentication, #organizationId, 'branch.membership.create')")
    public MembershipResponse create(
            @PathVariable UUID organizationId,
            @PathVariable UUID branchId,
            @Valid @RequestBody CreateMembershipRequest request) {
        return MembershipResponse.from(
                membershipService.create(organizationId, branchId, request.userId()));
    }

    @GetMapping
    @Operation(summary = "List memberships of a branch (paginated)")
    @ApiResponses({
        @ApiResponse(responseCode = "200", description = "OK", content = @Content),
        @ApiResponse(responseCode = "400", description = "Invalid paging parameters (VALIDATION_ERROR)", content = @Content),
        @ApiResponse(responseCode = "404", description = "Organization or branch not found (ORGANIZATION_NOT_FOUND, BRANCH_NOT_FOUND)", content = @Content),
        @ApiResponse(responseCode = "403", description = "Forbidden (branch.membership.read)", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasPermission(authentication, #organizationId, 'branch.membership.read')")
    public PageResponse<MembershipResponse> list(
            @PathVariable UUID organizationId,
            @PathVariable UUID branchId,
            @RequestParam(defaultValue = "0") int page,
            @RequestParam(defaultValue = "20") int size,
            @RequestParam(required = false) BranchMembershipStatus status) {
        validatePaging(page, size);
        Page<BranchMembership> memberships =
                membershipService.list(organizationId, branchId, status, PageRequest.of(page, size, ORDER));
        return PageResponse.from(memberships, MembershipResponse::from);
    }

    @GetMapping("/{userId}")
    @Operation(summary = "Get the membership of a user in this branch")
    @ApiResponses({
        @ApiResponse(responseCode = "200", description = "OK", content = @Content),
        @ApiResponse(responseCode = "404", description = "Organization, branch or membership not found (ORGANIZATION_NOT_FOUND, BRANCH_NOT_FOUND, BRANCH_MEMBERSHIP_NOT_FOUND)", content = @Content),
        @ApiResponse(responseCode = "403", description = "Forbidden (branch.membership.read)", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasPermission(authentication, #organizationId, 'branch.membership.read')")
    public MembershipResponse get(
            @PathVariable UUID organizationId, @PathVariable UUID branchId, @PathVariable UUID userId) {
        return MembershipResponse.from(membershipService.get(organizationId, branchId, userId));
    }

    @PatchMapping("/{userId}")
    @Operation(summary = "Partially update a membership (status; no DELETE - INACTIVE is the way down)")
    @ApiResponses({
        @ApiResponse(responseCode = "200", description = "Updated", content = @Content),
        @ApiResponse(responseCode = "400", description = "Validation (VALIDATION_ERROR)", content = @Content),
        @ApiResponse(responseCode = "404", description = "Organization, branch or membership not found (ORGANIZATION_NOT_FOUND, BRANCH_NOT_FOUND, BRANCH_MEMBERSHIP_NOT_FOUND)", content = @Content),
        @ApiResponse(responseCode = "409", description = "Concurrent modification (CONCURRENT_MODIFICATION)", content = @Content),
        @ApiResponse(responseCode = "403", description = "Forbidden (branch.membership.update)", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasPermission(authentication, #organizationId, 'branch.membership.update')")
    public MembershipResponse update(
            @PathVariable UUID organizationId,
            @PathVariable UUID branchId,
            @PathVariable UUID userId,
            @Valid @RequestBody UpdateMembershipRequest request) {
        return MembershipResponse.from(membershipService.update(
                organizationId,
                branchId,
                userId,
                new UpdateMembershipCommand(request.expectedVersion(), request.status())));
    }

    @GetMapping("/{userId}/roles")
    @Operation(summary = "List the branch-scoped roles of a membership")
    @ApiResponses({
        @ApiResponse(responseCode = "200", description = "OK", content = @Content),
        @ApiResponse(responseCode = "404", description = "Organization, branch or membership not found (ORGANIZATION_NOT_FOUND, BRANCH_NOT_FOUND, BRANCH_MEMBERSHIP_NOT_FOUND)", content = @Content),
        @ApiResponse(responseCode = "403", description = "Forbidden (branch.membership.read)", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasPermission(authentication, #organizationId, 'branch.membership.read')")
    public List<RoleResponse> listRoles(
            @PathVariable UUID organizationId, @PathVariable UUID branchId, @PathVariable UUID userId) {
        return membershipService.listRoles(organizationId, branchId, userId).stream()
                .map(RoleResponse::from)
                .toList();
    }

    @PutMapping("/{userId}/roles")
    @Operation(summary = "Atomically replace the membership's branch-scoped role set")
    @ApiResponses({
        @ApiResponse(responseCode = "200", description = "Replaced; returns the new set", content = @Content),
        @ApiResponse(responseCode = "400", description = "Duplicate role ids (VALIDATION_ERROR)", content = @Content),
        @ApiResponse(responseCode = "404", description = "Organization, branch, membership or role not found (…, ROLE_NOT_FOUND)", content = @Content),
        @ApiResponse(responseCode = "403", description = "Forbidden (branch.membership.roles.manage)", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasPermission(authentication, #organizationId, 'branch.membership.roles.manage')")
    public List<RoleResponse> replaceRoles(
            @PathVariable UUID organizationId,
            @PathVariable UUID branchId,
            @PathVariable UUID userId,
            @Valid @RequestBody PutMembershipRolesRequest request) {
        membershipService.replaceRoles(organizationId, branchId, userId, request.roleIds());
        return membershipService.listRoles(organizationId, branchId, userId).stream()
                .map(RoleResponse::from)
                .toList();
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