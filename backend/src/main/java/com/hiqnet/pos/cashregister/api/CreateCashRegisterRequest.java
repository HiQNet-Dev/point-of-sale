package com.hiqnet.pos.cashregister.api;

import jakarta.validation.constraints.NotBlank;
import jakarta.validation.constraints.Pattern;
import jakarta.validation.constraints.Size;

/**
 * Cash register creation payload. Status starts ACTIVE and is not
 * client-settable; the organization and branch come from the request path.
 */
public record CreateCashRegisterRequest(
        @NotBlank
                @Size(max = 64)
                @Pattern(regexp = "^[a-z0-9]+(-[a-z0-9]+)*$", message = "must be a lowercase slug")
                String code,
        @NotBlank @Size(max = 120) String name) {
}
