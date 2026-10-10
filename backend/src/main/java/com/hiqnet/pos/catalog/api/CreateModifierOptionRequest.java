package com.hiqnet.pos.catalog.api;

import jakarta.validation.constraints.Digits;
import jakarta.validation.constraints.Min;
import jakarta.validation.constraints.NotBlank;
import jakarta.validation.constraints.NotNull;
import jakarta.validation.constraints.Pattern;
import jakarta.validation.constraints.Size;
import java.math.BigDecimal;

/**
 * Modifier option creation payload. Status starts ACTIVE; it is not
 * client-settable. {@code priceDelta} is signed (negative allowed).
 */
public record CreateModifierOptionRequest(
        @NotBlank
                @Size(max = 64)
                @Pattern(regexp = "^[a-z0-9]+(-[a-z0-9]+)*$", message = "must be a lowercase slug")
                String code,
        @NotBlank @Size(max = 120) String name,
        @NotNull @Digits(integer = 15, fraction = 4) BigDecimal priceDelta,
        @Min(0) Integer displayOrder) {
}
