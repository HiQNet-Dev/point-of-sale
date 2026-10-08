package com.hiqnet.pos.branch.api;

import com.hiqnet.pos.branch.domain.BranchStatus;
import com.hiqnet.pos.shared.api.PatchField;
import io.swagger.v3.oas.annotations.media.Schema;
import jakarta.validation.constraints.Min;
import jakarta.validation.constraints.NotNull;
import jakarta.validation.constraints.Size;

/**
 * Partial update: only fields present are applied; {@code code} is immutable.
 * {@code expectedVersion} is required (the version the client read).
 * {@code timezone} follows the explicit tri-state semantics of {@link PatchField}.
 */
public record UpdateBranchRequest(
        @Schema(description = "Version the client read; must match the current version.", example = "3")
        @NotNull
                @Min(0)
                Long expectedVersion,
        @Size(max = 120) String name,
        @Schema(
                implementation = String.class,
                nullable = true,
                description = "Set an explicit timezone override; null clears the override and "
                        + "inherits the organization timezone; omit the field to keep the current value.")
        PatchField timezone,
        BranchStatus status) {
}