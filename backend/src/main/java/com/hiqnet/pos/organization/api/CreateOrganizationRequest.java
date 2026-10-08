package com.hiqnet.pos.organization.api;

import jakarta.validation.constraints.NotBlank;
import jakarta.validation.constraints.Pattern;
import jakarta.validation.constraints.Size;

public record CreateOrganizationRequest(
        @NotBlank
                @Size(max = 120)
                String name,
        @NotBlank
                @Size(max = 64)
                @Pattern(regexp = "^[a-z0-9]+(-[a-z0-9]+)*$", message = "must be a lowercase slug")
                String code,
        @NotBlank
                @Size(max = 64)
                String timezone,
        @NotBlank
                @Size(min = 3, max = 3)
                String currency,
        @NotBlank
                @Size(max = 20)
                String locale) {
}