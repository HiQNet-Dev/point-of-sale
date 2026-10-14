package com.hiqnet.pos.inventory.stock.api;
import com.hiqnet.pos.inventory.stock.application.InventoryQuantity;
import com.hiqnet.pos.inventory.stock.domain.BranchInventory;
import com.hiqnet.pos.inventory.stock.domain.InventoryMovementType;
import io.swagger.v3.oas.annotations.media.Schema;
import java.math.BigDecimal;
import java.util.UUID;
public record InventoryStockMutationResponse(UUID inventoryItemId,UUID movementId,InventoryMovementType type,
        @Schema(example = "-3.500000", description = "Delta aplicado como string decimal con signo (hasta 6 decimales).") String quantityDelta,
        @Schema(example = "96.500000", description = "Existencia resultante como string decimal (hasta 6 decimales).") String quantityOnHand,long version) {
    public static InventoryStockMutationResponse from(UUID item,UUID movement,InventoryMovementType type,BigDecimal delta,BranchInventory balance) {
        return new InventoryStockMutationResponse(item,movement,type,InventoryQuantity.format(delta),InventoryQuantity.format(balance.getQuantityOnHand()),balance.getVersion());
    }
}
