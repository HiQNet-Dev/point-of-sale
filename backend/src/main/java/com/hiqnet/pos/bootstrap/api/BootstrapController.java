package com.hiqnet.pos.bootstrap.api;

import com.hiqnet.pos.bootstrap.application.BootstrapCommand;
import com.hiqnet.pos.bootstrap.application.BootstrapResult;
import com.hiqnet.pos.bootstrap.application.BootstrapService;
import com.hiqnet.pos.shared.config.BootstrapTokenGuard;
import io.swagger.v3.oas.annotations.Operation;
import io.swagger.v3.oas.annotations.enums.SecuritySchemeIn;
import io.swagger.v3.oas.annotations.enums.SecuritySchemeType;
import io.swagger.v3.oas.annotations.media.Content;
import io.swagger.v3.oas.annotations.responses.ApiResponse;
import io.swagger.v3.oas.annotations.responses.ApiResponses;
import io.swagger.v3.oas.annotations.security.SecurityRequirement;
import io.swagger.v3.oas.annotations.security.SecurityScheme;
import jakarta.validation.Valid;
import org.springframework.http.HttpStatus;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.RequestBody;
import org.springframework.web.bind.annotation.RequestHeader;
import org.springframework.web.bind.annotation.ResponseStatus;
import org.springframework.web.bind.annotation.RestController;

/**
 * One-shot first-run provisioning endpoint. It is NOT a regular administrative
 * API and requires NO bearer token; the deployment secret travels exclusively in
 * the {@code X-Bootstrap-Token} header, is verified by
 * {@link BootstrapTokenFilter} BEFORE the body is read, and is re-verified here
 * as defence in depth. After a successful provisioning the endpoint stays
 * closed forever by the database singleton state (and, operationally, by
 * removing {@code BOOTSTRAP_SECRET}).
 */
@RestController
@SecurityScheme(
        name = "bootstrapToken",
        type = SecuritySchemeType.APIKEY,
        in = SecuritySchemeIn.HEADER,
        paramName = "X-Bootstrap-Token",
        description = "One-shot deployment secret (Base64, >= 256 bits). Only for the first provisioning; never document a real value.")
public class BootstrapController {

    private final BootstrapService bootstrapService;
    private final BootstrapTokenGuard tokenGuard;

    public BootstrapController(BootstrapService bootstrapService, BootstrapTokenGuard tokenGuard) {
        this.bootstrapService = bootstrapService;
        this.tokenGuard = tokenGuard;
    }

    @PostMapping("/api/v1/bootstrap")
    @ResponseStatus(HttpStatus.CREATED)
    @Operation(
            summary = "One-shot installation provisioning (secure admin bootstrap)",
            description = "Gated by the X-Bootstrap-Token deployment secret. Creates the first "
                    + "Organization, the first administrator User, the 'administrator' Role "
                    + "(a normal role granted every current permission) and the organization-wide "
                    + "assignment. Returns NO tokens: authenticate afterwards via POST /auth/login.")
    @ApiResponses({
        @ApiResponse(responseCode = "201", description = "Provisioned"),
        @ApiResponse(responseCode = "400", description = "Validation error (VALIDATION_ERROR, INVALID_CODE/TIMEZONE/CURRENCY/LOCALE, WEAK_PASSWORD)", content = @Content),
        @ApiResponse(responseCode = "401", description = "Missing/invalid X-Bootstrap-Token or bootstrap disabled (BOOTSTRAP_INVALID_TOKEN)", content = @Content),
        @ApiResponse(responseCode = "409", description = "Already completed (BOOTSTRAP_ALREADY_COMPLETED) or installation is not pristine (BOOTSTRAP_NOT_ALLOWED)", content = @Content)
    })
    @SecurityRequirement(name = "bootstrapToken")
    public BootstrapResponse bootstrap(
            @RequestHeader(value = BootstrapTokenGuard.HEADER, required = false) String bootstrapToken,
            @Valid @RequestBody BootstrapRequest request) {
        // Defence in depth: even though the filter already validated the token
        // before the body was parsed, an HTTP invocation must never be able to
        // reach provisioning without the secret (fail closed).
        tokenGuard.verify(bootstrapToken);
        BootstrapResult result = bootstrapService.bootstrap(toCommand(request));
        return BootstrapResponse.from(result);
    }

    private static BootstrapCommand toCommand(BootstrapRequest request) {
        BootstrapRequest.OrganizationSpec organization = request.organization();
        BootstrapRequest.AdministratorSpec administrator = request.administrator();
        return new BootstrapCommand(
                organization.name(),
                organization.code(),
                organization.timezone(),
                organization.currency(),
                organization.locale(),
                administrator.email(),
                administrator.password());
    }
}