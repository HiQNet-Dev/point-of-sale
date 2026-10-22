package com.hiqnet.pos.catalog.api;

import com.hiqnet.pos.catalog.application.ModifierOptionService;
import com.hiqnet.pos.catalog.application.UpdateModifierOptionCommand;
import com.hiqnet.pos.catalog.domain.ModifierOption;
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
 * Organization-scoped modifier-option administration, nested under a modifier
 * group. Every endpoint authorizes with organization-scope
 * {@code hasPermission} (never {@code hasBranchPermission}). There is no DELETE:
 * an option is retired via status INACTIVE.
 */
@RestController
@RequestMapping("/api/v1/organizations/{organizationId}/modifier-groups/{groupId}/options")
@Tag(name = "Modifiers", description = "Grupos y opciones de modificadores de producto.")
public class ModifierOptionController {

    private static final int MAX_PAGE_SIZE = 100;

    /** Deterministic order: displayOrder ASC, id ASC as tiebreaker. */
    private static final Sort ORDER = Sort.by(Sort.Order.asc("displayOrder"), Sort.Order.asc("id"));

    private final ModifierOptionService modifierOptionService;

    public ModifierOptionController(ModifierOptionService modifierOptionService) {
        this.modifierOptionService = modifierOptionService;
    }

    @PostMapping
    @ResponseStatus(HttpStatus.CREATED)
    @Operation(summary = "Create a modifier option inside a modifier group")
    @ApiResponses({
        @ApiResponse(responseCode = "201", description = "Created", content = @Content),
        @ApiResponse(responseCode = "400", description = "Validation (VALIDATION_ERROR, INVALID_CODE)", content = @Content),
        @ApiResponse(responseCode = "404", description = "Organization or modifier group not found (ORGANIZATION_NOT_FOUND, MODIFIER_GROUP_NOT_FOUND)", content = @Content),
        @ApiResponse(responseCode = "409", description = "Duplicate code (MODIFIER_OPTION_CODE_ALREADY_EXISTS)", content = @Content),
        @ApiResponse(responseCode = "403", description = "Forbidden (modifier.option.create)", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasPermission(authentication, #organizationId, 'modifier.option.create')")
    public ModifierOptionResponse create(
            @PathVariable UUID organizationId,
            @PathVariable UUID groupId,
            @Valid @RequestBody CreateModifierOptionRequest request) {
        int displayOrder = request.displayOrder() == null ? 0 : request.displayOrder();
        return ModifierOptionResponse.from(modifierOptionService.create(
                organizationId, groupId, request.code(), request.name(), request.priceDelta(), displayOrder));
    }

    @GetMapping
    @Operation(summary = "List options of a modifier group (paginated, ordered by displayOrder)")
    @ApiResponses({
        @ApiResponse(responseCode = "200", description = "OK", content = @Content),
        @ApiResponse(responseCode = "400", description = "Invalid paging parameters (VALIDATION_ERROR)", content = @Content),
        @ApiResponse(responseCode = "404", description = "Organization or modifier group not found (ORGANIZATION_NOT_FOUND, MODIFIER_GROUP_NOT_FOUND)", content = @Content),
        @ApiResponse(responseCode = "403", description = "Forbidden (modifier.option.read)", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasPermission(authentication, #organizationId, 'modifier.option.read')")
    public PageResponse<ModifierOptionResponse> list(
            @PathVariable UUID organizationId,
            @PathVariable UUID groupId,
            @RequestParam(defaultValue = "0") int page,
            @RequestParam(defaultValue = "20") int size) {
        validatePaging(page, size);
        Page<ModifierOption> options =
                modifierOptionService.list(organizationId, groupId, PageRequest.of(page, size, ORDER));
        return PageResponse.from(options, ModifierOptionResponse::from);
    }

    @GetMapping("/{optionId}")
    @Operation(summary = "Get a modifier option by id (scoped to the organization and group)")
    @ApiResponses({
        @ApiResponse(responseCode = "200", description = "OK", content = @Content),
        @ApiResponse(responseCode = "404", description = "Organization, modifier group or option not found (ORGANIZATION_NOT_FOUND, MODIFIER_GROUP_NOT_FOUND, MODIFIER_OPTION_NOT_FOUND)", content = @Content),
        @ApiResponse(responseCode = "403", description = "Forbidden (modifier.option.read)", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasPermission(authentication, #organizationId, 'modifier.option.read')")
    public ModifierOptionResponse get(
            @PathVariable UUID organizationId, @PathVariable UUID groupId, @PathVariable UUID optionId) {
        return ModifierOptionResponse.from(modifierOptionService.get(organizationId, groupId, optionId));
    }

    @PatchMapping("/{optionId}")
    @Operation(summary = "Partially update a modifier option (code immutable; name, priceDelta, displayOrder, status)")
    @ApiResponses({
        @ApiResponse(responseCode = "200", description = "Updated", content = @Content),
        @ApiResponse(responseCode = "400", description = "Validation (VALIDATION_ERROR)", content = @Content),
        @ApiResponse(responseCode = "404", description = "Organization, modifier group or option not found (ORGANIZATION_NOT_FOUND, MODIFIER_GROUP_NOT_FOUND, MODIFIER_OPTION_NOT_FOUND)", content = @Content),
        @ApiResponse(responseCode = "409", description = "Concurrent modification (CONCURRENT_MODIFICATION)", content = @Content),
        @ApiResponse(responseCode = "403", description = "Forbidden (modifier.option.update)", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasPermission(authentication, #organizationId, 'modifier.option.update')")
    public ModifierOptionResponse update(
            @PathVariable UUID organizationId,
            @PathVariable UUID groupId,
            @PathVariable UUID optionId,
            @Valid @RequestBody UpdateModifierOptionRequest request) {
        return ModifierOptionResponse.from(modifierOptionService.update(
                organizationId,
                groupId,
                optionId,
                new UpdateModifierOptionCommand(
                        request.expectedVersion(),
                        request.name(),
                        request.priceDelta(),
                        request.displayOrder(),
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
