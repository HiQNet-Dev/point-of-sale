package com.hiqnet.pos.inventory.stock.api;
import com.hiqnet.pos.inventory.stock.application.InventoryQuantity;
import com.hiqnet.pos.inventory.stock.domain.InventoryMovementRow;
import com.hiqnet.pos.inventory.stock.domain.InventoryMovementType;
import java.time.Instant;
import java.util.UUID;
public record InventoryMovementResponse(UUID id,InventoryMovementType type,String quantityDelta,String resultingQuantity,
        String reason,UUID actorUserId,UUID saleId,Instant occurredAt) {
    public static InventoryMovementResponse from(InventoryMovementRow row) {
        return new InventoryMovementResponse(row.id(),row.type(),InventoryQuantity.format(row.quantityDelta()),InventoryQuantity.format(row.resultingQuantity()),row.reason(),row.actorUserId(),row.saleId(),row.occurredAt());
    }
}
