package com.hiqnet.pos.inventory.api;

import com.hiqnet.pos.inventory.domain.InventoryBaseUnit;
import com.hiqnet.pos.inventory.domain.InventoryItemStatus;
import io.swagger.v3.oas.annotations.media.Schema;
import jakarta.validation.constraints.Min;
import jakarta.validation.constraints.NotNull;
import jakarta.validation.constraints.Size;

public record UpdateInventoryItemRequest(
        @Schema(description = "Version the client read; must match the current version.", example = "0")
                @NotNull @Min(0) Long expectedVersion,
        @Size(max = 120) String name,
        @Size(max = 1000) String description,
        @Schema(description = "Nueva unidad base. Solo se aplica si el artículo no tiene dependencias "
                        + "(balance de sucursal, movimiento, receta, efecto de modificador o partida de orden de "
                        + "compra); en caso contrario responde 409 INVENTORY_ITEM_UNIT_LOCKED. Enviar la unidad "
                        + "actual es un no-op idempotente.", example = "GRAM")
                InventoryBaseUnit baseUnit,
        Boolean fractionalUnitAllowed,
        InventoryItemStatus status) {
}
