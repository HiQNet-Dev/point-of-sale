package com.hiqnet.pos.catalog.api;

import com.hiqnet.pos.catalog.application.ModifierGroupService;
import com.hiqnet.pos.catalog.application.UpdateModifierGroupCommand;
import com.hiqnet.pos.catalog.domain.ModifierGroup;
import com.hiqnet.pos.catalog.domain.ModifierGroupStatus;
import com.hiqnet.pos.shared.api.PageResponse;
import com.hiqnet.pos.shared.error.BusinessException;
import com.hiqnet.pos.shared.error.ErrorCodes;
import io.swagger.v3.oas.annotations.Operation;
import io.swagger.v3.oas.annotations.media.Content;
import io.swagger.v3.oas.annotations.responses.ApiResponse;
import io.swagger.v3.oas.annotations.responses.ApiResponses;
import io.swagger.v3.oas.annotations.tags.Tag;
import jakarta.validation.Valid;
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
import org.springframework.web.bind.annotation.RequestBody;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RequestParam;
import org.springframework.web.bind.annotation.ResponseStatus;
import org.springframework.web.bind.annotation.RestController;

/**
 * Organization-scoped modifier-group administration. The modifier catalog
 * belongs to the organization, so every endpoint authorizes with
 * organization-scope {@code hasPermission} (never {@code hasBranchPermission}).
 * There is no DELETE: a group is retired via status INACTIVE.
 */
@RestController
@RequestMapping("/api/v1/organizations/{organizationId}/modifier-groups")
@Tag(name = "Modifiers", description = "Grupos y opciones de modificadores de producto.")
public class ModifierGroupController {

    private static final int MAX_PAGE_SIZE = 100;

    /** Deterministic order: name ASC, id ASC as tiebreaker. */
    private static final Sort ORDER = Sort.by(Sort.Order.asc("name"), Sort.Order.asc("id"));

    private final ModifierGroupService modifierGroupService;

    public ModifierGroupController(ModifierGroupService modifierGroupService) {
        this.modifierGroupService = modifierGroupService;
    }

    @PostMapping
    @ResponseStatus(HttpStatus.CREATED)
    @Operation(summary = "Create a modifier group inside an organization")
    @ApiResponses({
        @ApiResponse(responseCode = "201", description = "Created", content = @Content),
        @ApiResponse(responseCode = "400", description = "Validation (VALIDATION_ERROR, INVALID_CODE)", content = @Content),
        @ApiResponse(responseCode = "404", description = "Organization not found (ORGANIZATION_NOT_FOUND)", content = @Content),
        @ApiResponse(responseCode = "409", description = "Duplicate code (MODIFIER_GROUP_CODE_ALREADY_EXISTS)", content = @Content),
        @ApiResponse(responseCode = "403", description = "Forbidden (modifier.group.create)", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasPermission(authentication, #organizationId, 'modifier.group.create')")
    public ModifierGroupResponse create(
            @PathVariable UUID organizationId, @Valid @RequestBody CreateModifierGroupRequest request) {
        int minSelections = request.minSelections() == null ? 0 : request.minSelections();
        int maxSelections = request.maxSelections() == null ? 1 : request.maxSelections();
        return ModifierGroupResponse.from(
                modifierGroupService.create(organizationId, request.code(), request.name(), minSelections, maxSelections));
    }

    @GetMapping
    @Operation(summary = "List modifier groups of an organization (paginated; filters status, q)")
    @ApiResponses({
        @ApiResponse(responseCode = "200", description = "OK", content = @Content),
        @ApiResponse(responseCode = "400", description = "Invalid paging parameters (VALIDATION_ERROR)", content = @Content),
        @ApiResponse(responseCode = "404", description = "Organization not found (ORGANIZATION_NOT_FOUND)", content = @Content),
        @ApiResponse(responseCode = "403", description = "Forbidden (modifier.group.read)", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasPermission(authentication, #organizationId, 'modifier.group.read')")
    public PageResponse<ModifierGroupResponse> list(
            @PathVariable UUID organizationId,
            @RequestParam(defaultValue = "0") int page,
            @RequestParam(defaultValue = "20") int size,
            @RequestParam(required = false) ModifierGroupStatus status,
            @RequestParam(required = false) String q) {
        validatePaging(page, size);
        Page<ModifierGroup> groups =
                modifierGroupService.list(organizationId, status, q, PageRequest.of(page, size, ORDER));
        return PageResponse.from(groups, ModifierGroupResponse::from);
    }

    @GetMapping("/{groupId}")
    @Operation(summary = "Get a modifier group by id (scoped to the organization)")
    @ApiResponses({
        @ApiResponse(responseCode = "200", description = "OK", content = @Content),
        @ApiResponse(responseCode = "404", description = "Organization or modifier group not found (ORGANIZATION_NOT_FOUND, MODIFIER_GROUP_NOT_FOUND)", content = @Content),
        @ApiResponse(responseCode = "403", description = "Forbidden (modifier.group.read)", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasPermission(authentication, #organizationId, 'modifier.group.read')")
    public ModifierGroupResponse get(@PathVariable UUID organizationId, @PathVariable UUID groupId) {
        return ModifierGroupResponse.from(modifierGroupService.get(organizationId, groupId));
    }

    @PatchMapping("/{groupId}")
    @Operation(summary = "Partially update a modifier group (code immutable; name, selections, status)")
    @ApiResponses({
        @ApiResponse(responseCode = "200", description = "Updated", content = @Content),
        @ApiResponse(responseCode = "400", description = "Validation (VALIDATION_ERROR)", content = @Content),
        @ApiResponse(responseCode = "404", description = "Organization or modifier group not found (ORGANIZATION_NOT_FOUND, MODIFIER_GROUP_NOT_FOUND)", content = @Content),
        @ApiResponse(responseCode = "409", description = "Concurrent modification (CONCURRENT_MODIFICATION)", content = @Content),
        @ApiResponse(responseCode = "403", description = "Forbidden (modifier.group.update)", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasPermission(authentication, #organizationId, 'modifier.group.update')")
    public ModifierGroupResponse update(
            @PathVariable UUID organizationId,
            @PathVariable UUID groupId,
            @Valid @RequestBody UpdateModifierGroupRequest request) {
        return ModifierGroupResponse.from(modifierGroupService.update(
                organizationId,
                groupId,
                new UpdateModifierGroupCommand(
                        request.expectedVersion(),
                        request.name(),
                        request.minSelections(),
                        request.maxSelections(),
                        request.status())));
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
