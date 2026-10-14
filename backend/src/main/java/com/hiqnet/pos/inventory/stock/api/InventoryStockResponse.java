package com.hiqnet.pos.inventory.stock.api;
import com.hiqnet.pos.inventory.domain.InventoryBaseUnit;
import com.hiqnet.pos.inventory.domain.InventoryItem;
import com.hiqnet.pos.inventory.stock.application.InventoryQuantity;
import com.hiqnet.pos.inventory.stock.domain.BranchInventory;
import io.swagger.v3.oas.annotations.media.Schema;
import java.math.BigDecimal;
import java.util.UUID;
public record InventoryStockResponse(UUID inventoryItemId,String code,String name,InventoryBaseUnit baseUnit,
        boolean fractionalUnitAllowed,String status,
        @Schema(example = "100.000000", description = "Existencia actual como string decimal (hasta 6 decimales).") String quantityOnHand,long version) {
    public static InventoryStockResponse from(InventoryItem item,BigDecimal quantity,long version) {
        return new InventoryStockResponse(item.getId(),item.getCode(),item.getName(),item.getBaseUnit(),item.isFractionalUnitAllowed(),item.getStatus().name(),InventoryQuantity.format(quantity),version);
    }
    public static InventoryStockResponse from(InventoryItem item,BranchInventory balance) {
        return from(item,balance==null?BigDecimal.ZERO:balance.getQuantityOnHand(),balance==null?0:balance.getVersion());
    }
}
