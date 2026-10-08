package com.hiqnet.pos.rbac.api;

import io.swagger.v3.oas.annotations.media.Schema;
import jakarta.validation.constraints.NotBlank;
import jakarta.validation.constraints.Pattern;
import jakarta.validation.constraints.Size;

/** Create payload. {@code code} must be a normalized slug; never derived from name. */
public record CreateRoleRequest(
        @Schema(description = "Normalized immutable slug (trim+lowercase).", example = "store-manager")
        @NotBlank
                @Size(max = 64)
                @Pattern(regexp = "^[a-z0-9]+(-[a-z0-9]+)*$", message = "must be a lowercase slug")
                String code,
        @Schema(description = "Editable display name.", example = "Store Manager")
        @NotBlank
                @Size(max = 120)
                String name) {
}