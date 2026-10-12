package com.hiqnet.pos.organization.api;

import com.hiqnet.pos.organization.application.OrganizationService;
import com.hiqnet.pos.organization.application.UpdateOrganizationCommand;
import io.swagger.v3.oas.annotations.Operation;
import io.swagger.v3.oas.annotations.media.Content;
import io.swagger.v3.oas.annotations.responses.ApiResponse;
import io.swagger.v3.oas.annotations.responses.ApiResponses;
import jakarta.validation.Valid;
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
@RequestMapping("/api/v1/organizations")
public class OrganizationController {

    private final OrganizationService organizationService;

    public OrganizationController(OrganizationService organizationService) {
        this.organizationService = organizationService;
    }

    @PostMapping
    @ResponseStatus(HttpStatus.CREATED)
    @Operation(summary = "Create an organization")
    @ApiResponses({
        @ApiResponse(responseCode = "201", description = "Created"),
        @ApiResponse(responseCode = "400", description = "Validation or malformed request (VALIDATION_ERROR, INVALID_CODE/TIMEZONE/CURRENCY/LOCALE)", content = @Content),
        @ApiResponse(responseCode = "409", description = "Duplicate code (ORGANIZATION_CODE_ALREADY_EXISTS)", content = @Content)
    })
    public OrganizationResponse create(@Valid @RequestBody CreateOrganizationRequest request) {
        return OrganizationResponse.from(organizationService.create(
                request.name(),
                request.code(),
                request.timezone(),
                request.currency(),
                request.locale()));
    }

    @GetMapping("/{organizationId}")
    @Operation(summary = "Get an organization by id")
    @ApiResponses({
        @ApiResponse(responseCode = "200", description = "OK", content = @Content),
        @ApiResponse(responseCode = "404", description = "Organization not found (ORGANIZATION_NOT_FOUND)", content = @Content),
        @ApiResponse(responseCode = "403", description = "Forbidden (organization.read)", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasPermission(authentication, #organizationId, 'organization.read')")
    public OrganizationResponse get(@PathVariable UUID organizationId) {
        return OrganizationResponse.from(organizationService.get(organizationId));
    }

    @PatchMapping("/{organizationId}")
    @Operation(summary = "Partially update an organization")
    @ApiResponses({
        @ApiResponse(responseCode = "200", description = "Updated", content = @Content),
        @ApiResponse(responseCode = "400", description = "Validation or malformed request (VALIDATION_ERROR, INVALID_CODE/TIMEZONE/CURRENCY/LOCALE)", content = @Content),
        @ApiResponse(responseCode = "404", description = "Organization not found (ORGANIZATION_NOT_FOUND)", content = @Content),
        @ApiResponse(responseCode = "409", description = "Concurrent modification (CONCURRENT_MODIFICATION)", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasPermission(authentication, #organizationId, 'organization.update')")
    public OrganizationResponse update(
            @PathVariable UUID organizationId, @Valid @RequestBody UpdateOrganizationRequest request) {
        return OrganizationResponse.from(organizationService.update(
                organizationId,
                new UpdateOrganizationCommand(
                        request.expectedVersion(),
                        request.name(),
                        request.timezone(),
                        request.currency(),
                        request.locale(),
                        request.status(),
                        request.inventoryNegativeStockPolicy())));
    }
}