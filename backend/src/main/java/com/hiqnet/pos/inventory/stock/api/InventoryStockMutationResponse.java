package com.hiqnet.pos.inventory.stock.api;
import com.hiqnet.pos.inventory.stock.application.InventoryQuantity;
import com.hiqnet.pos.inventory.stock.domain.BranchInventory;
import com.hiqnet.pos.inventory.stock.domain.InventoryMovementType;
import java.math.BigDecimal;
import java.util.UUID;
public record InventoryStockMutationResponse(UUID inventoryItemId,UUID movementId,InventoryMovementType type,
        String quantityDelta,String quantityOnHand,long version) {
    public static InventoryStockMutationResponse from(UUID item,UUID movement,InventoryMovementType type,BigDecimal delta,BranchInventory balance) {
        return new InventoryStockMutationResponse(item,movement,type,InventoryQuantity.format(delta),InventoryQuantity.format(balance.getQuantityOnHand()),balance.getVersion());
    }
}
