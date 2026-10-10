package com.hiqnet.pos.catalog.api;

import jakarta.validation.constraints.DecimalMin;
import jakarta.validation.constraints.Digits;
import jakarta.validation.constraints.NotBlank;
import jakarta.validation.constraints.NotNull;
import jakarta.validation.constraints.Pattern;
import jakarta.validation.constraints.Size;
import java.math.BigDecimal;
import java.util.UUID;

public record CreateProductRequest(
        @NotBlank @Size(max = 64) @Pattern(regexp = "^[A-Za-z0-9._-]{1,64}$") String sku,
        @Size(max = 64) String barcode,
        @NotBlank @Size(max = 120) String name,
        @Size(max = 1000) String description,
        UUID categoryId,
        @NotNull @DecimalMin(value = "0") @Digits(integer = 15, fraction = 4) BigDecimal unitPrice) {
}
