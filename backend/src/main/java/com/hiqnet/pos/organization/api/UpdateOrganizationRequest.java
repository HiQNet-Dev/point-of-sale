package com.hiqnet.pos.organization.api;

import com.hiqnet.pos.organization.domain.OrganizationStatus;
import io.swagger.v3.oas.annotations.media.Schema;
import jakarta.validation.constraints.Min;
import jakarta.validation.constraints.NotNull;
import jakarta.validation.constraints.Size;

/**
 * Partial update: only fields present are applied; {@code code} is immutable.
 * Semantics of timezone/currency/locale are validated by the domain
 * (INVALID_TIMEZONE / INVALID_CURRENCY / INVALID_LOCALE).
 */
public record UpdateOrganizationRequest(
        @Schema(description = "Version the client read; must match the current version.", example = "3")
        @NotNull
                @Min(0)
                Long expectedVersion,
        @Size(max = 120) String name,
        @Size(max = 64) String timezone,
        @Size(min = 3, max = 3) String currency,
        @Size(max = 20) String locale,
        OrganizationStatus status) {
}