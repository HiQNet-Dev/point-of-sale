package com.hiqnet.pos.sales.api;

import jakarta.validation.constraints.Min;
import jakarta.validation.constraints.NotNull;

public record ExpectedSaleVersionRequest(@NotNull @Min(0) Long expectedVersion) {}
