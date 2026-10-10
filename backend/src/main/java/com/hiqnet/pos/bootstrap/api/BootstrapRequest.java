package com.hiqnet.pos.bootstrap.api;

import io.swagger.v3.oas.annotations.media.Schema;
import jakarta.validation.Valid;
import jakarta.validation.constraints.NotBlank;
import jakarta.validation.constraints.NotNull;
import jakarta.validation.constraints.Pattern;
import jakarta.validation.constraints.Size;

/**
 * Bootstrap provisioning request. The client decides ONLY the organization
 * shape and the administrator email/password. Accepting {@code role},
 * {@code permissions}, {@code status}, {@code organizationId} or
 * {@code userId} from the client is impossible (mass assignment is a
 * security risk); the service creates the role and grants the WHOLE current
 * permission catalog.
 *
 * <p>Validation mirrors the established domain rules (organization slug /
 * timezone / currency / locale); the authoritative password policy is
 * {@code PasswordPolicy}, applied by {@code UserService} (no second policy
 * lives here).</p>
 */
public record BootstrapRequest(
        @NotNull @Valid OrganizationSpec organization,
        @NotNull @Valid AdministratorSpec administrator) {

    @Schema(description = "First organization of the installation.")
    public record OrganizationSpec(
            @NotBlank @Size(max = 120) String name,
            @NotBlank @Size(max = 64) @Pattern(
                    regexp = "^[a-z0-9]+(-[a-z0-9]+)*$",
                    message = "must be a lowercase slug")
                    String code,
            @NotBlank @Size(max = 64) String timezone,
            @NotBlank @Size(min = 3, max = 3) String currency,
            @NotBlank @Size(max = 20) String locale) {}

    @Schema(description = "Initial administrator of the installation.")
    public record AdministratorSpec(
            @Schema(description = "Administrator email; normalized to trim+lowercase.", example = "admin@example.com")
            @NotBlank
            @Size(max = 254)
            String email,
            @Schema(description = "Initial password (hashed by the shared Policy/Encoder; never stored, logged or returned).")
            @NotBlank
            String password) {}
}