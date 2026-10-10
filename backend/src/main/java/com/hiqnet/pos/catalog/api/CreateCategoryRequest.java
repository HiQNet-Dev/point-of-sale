package com.hiqnet.pos.catalog.api;

import jakarta.validation.constraints.NotBlank;
import jakarta.validation.constraints.Pattern;
import jakarta.validation.constraints.Size;

/** Category creation payload. Status starts ACTIVE; it is not client-settable. */
public record CreateCategoryRequest(
        @NotBlank
                @Size(max = 64)
                @Pattern(regexp = "^[a-z0-9]+(-[a-z0-9]+)*$", message = "must be a lowercase slug")
                String code,
        @NotBlank @Size(max = 120) String name) {
}
