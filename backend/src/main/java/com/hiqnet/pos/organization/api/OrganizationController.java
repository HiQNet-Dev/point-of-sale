package com.hiqnet.pos.organization.api;

import com.hiqnet.pos.organization.application.OrganizationService;
import com.hiqnet.pos.organization.application.UpdateOrganizationCommand;
import jakarta.validation.Valid;
import java.util.UUID;
import org.springframework.http.HttpStatus;
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
    public OrganizationResponse create(@Valid @RequestBody CreateOrganizationRequest request) {
        return OrganizationResponse.from(organizationService.create(
                request.name(),
                request.code(),
                request.timezone(),
                request.currency(),
                request.locale()));
    }

    @GetMapping("/{organizationId}")
    public OrganizationResponse get(@PathVariable UUID organizationId) {
        return OrganizationResponse.from(organizationService.get(organizationId));
    }

    @PatchMapping("/{organizationId}")
    public OrganizationResponse update(
            @PathVariable UUID organizationId, @Valid @RequestBody UpdateOrganizationRequest request) {
        return OrganizationResponse.from(organizationService.update(
                organizationId,
                new UpdateOrganizationCommand(
                        request.name(),
                        request.timezone(),
                        request.currency(),
                        request.locale(),
                        request.status())));
    }
}