package com.hiqnet.pos.branch.api;

import com.hiqnet.pos.branch.application.BranchService;
import com.hiqnet.pos.branch.application.UpdateBranchCommand;
import io.swagger.v3.oas.annotations.Operation;
import io.swagger.v3.oas.annotations.media.Content;
import io.swagger.v3.oas.annotations.responses.ApiResponse;
import io.swagger.v3.oas.annotations.responses.ApiResponses;
import jakarta.validation.Valid;
import java.util.List;
import java.util.UUID;
import org.springframework.http.HttpStatus;
import org.springframework.security.access.prepost.PreAuthorize;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.PatchMapping;
import org.springframework.web.bind.annotation.PathVariable;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.RequestBody;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.ResponseStatus;
import org.springframework.web.bind.annotation.RestController;

@RestController
@RequestMapping("/api/v1/organizations/{organizationId}/branches")
public class BranchController {

    private final BranchService branchService;

    public BranchController(BranchService branchService) {
        this.branchService = branchService;
    }

    @PostMapping
    @ResponseStatus(HttpStatus.CREATED)
    @Operation(summary = "Create a branch inside an organization")
    @ApiResponses({
        @ApiResponse(responseCode = "201", description = "Created"),
        @ApiResponse(responseCode = "400", description = "Validation or malformed request (VALIDATION_ERROR, INVALID_CODE/TIMEZONE)", content = @Content),
        @ApiResponse(responseCode = "404", description = "Organization not found (ORGANIZATION_NOT_FOUND)", content = @Content),
        @ApiResponse(responseCode = "409", description = "Duplicate code (BRANCH_CODE_ALREADY_EXISTS)", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasPermission(authentication, #organizationId, 'branch.create')")
    public BranchResponse create(
            @PathVariable UUID organizationId, @Valid @RequestBody CreateBranchRequest request) {
        return BranchResponse.from(
                branchService.create(organizationId, request.name(), request.code(), request.timezone()));
    }

    @GetMapping
    @PreAuthorize("@authorizationService.hasPermission(authentication, #organizationId, 'branch.read')")
    public List<BranchResponse> list(@PathVariable UUID organizationId) {
        return branchService.list(organizationId).stream().map(BranchResponse::from).toList();
    }

    @GetMapping("/{branchId}")
    @Operation(summary = "Get a branch by id (scoped to the organization)")
    @ApiResponses({
        @ApiResponse(responseCode = "200", description = "OK", content = @Content),
        @ApiResponse(responseCode = "404", description = "Organization or branch not found (ORGANIZATION_NOT_FOUND, BRANCH_NOT_FOUND)", content = @Content),
        @ApiResponse(responseCode = "403", description = "Forbidden (branch.read)", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasPermission(authentication, #organizationId, 'branch.read')")
    public BranchResponse get(@PathVariable UUID organizationId, @PathVariable UUID branchId) {
        return BranchResponse.from(branchService.get(organizationId, branchId));
    }

    @PatchMapping("/{branchId}")
    @Operation(summary = "Partially update a branch")
    @ApiResponses({
        @ApiResponse(responseCode = "200", description = "Updated", content = @Content),
        @ApiResponse(responseCode = "400", description = "Validation or malformed request (VALIDATION_ERROR, INVALID_TIMEZONE)", content = @Content),
        @ApiResponse(responseCode = "404", description = "Organization or branch not found (ORGANIZATION_NOT_FOUND, BRANCH_NOT_FOUND)", content = @Content),
        @ApiResponse(responseCode = "409", description = "Concurrent modification (CONCURRENT_MODIFICATION)", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasPermission(authentication, #organizationId, 'branch.update')")
    public BranchResponse update(
            @PathVariable UUID organizationId,
            @PathVariable UUID branchId,
            @Valid @RequestBody UpdateBranchRequest request) {
        return BranchResponse.from(branchService.update(
                organizationId, branchId, new UpdateBranchCommand(
                        request.expectedVersion(), request.name(), request.timezone(), request.status())));
    }
}