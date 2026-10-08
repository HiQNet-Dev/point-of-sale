package com.hiqnet.pos.branch.api;

import jakarta.validation.constraints.NotBlank;
import jakarta.validation.constraints.Pattern;
import jakarta.validation.constraints.Size;

/** timezone is optional: null means "inherit organization timezone". */
public record CreateBranchRequest(
        @NotBlank
                @Size(max = 120)
                String name,
        @NotBlank
                @Size(max = 64)
                @Pattern(regexp = "^[a-z0-9]+(-[a-z0-9]+)*$", message = "must be a lowercase slug")
                String code,
        @Size(max = 64) String timezone) {
}