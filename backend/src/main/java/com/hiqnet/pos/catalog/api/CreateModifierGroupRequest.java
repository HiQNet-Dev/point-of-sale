package com.hiqnet.pos.catalog.api;

import jakarta.validation.constraints.Min;
import jakarta.validation.constraints.NotBlank;
import jakarta.validation.constraints.Pattern;
import jakarta.validation.constraints.Size;

/**
 * Modifier group creation payload. Status starts ACTIVE; it is not
 * client-settable. Selection bounds are optional and default to 0/1 (a plain
 * single-choice group).
 */
public record CreateModifierGroupRequest(
        @NotBlank
                @Size(max = 64)
                @Pattern(regexp = "^[a-z0-9]+(-[a-z0-9]+)*$", message = "must be a lowercase slug")
                String code,
        @NotBlank @Size(max = 120) String name,
        @Min(0) Integer minSelections,
        @Min(1) Integer maxSelections) {
}
