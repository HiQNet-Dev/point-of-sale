package com.hiqnet.pos.inventory.stock.domain;

import java.math.BigDecimal;
import java.util.UUID;
import org.springframework.data.domain.Page;
import org.springframework.data.domain.Pageable;

/** Append/read-only persistence boundary; deliberately exposes no update or delete operation. */
public interface InventoryMovementRepository {

    void insert(
            UUID id,
            UUID organizationId,
            UUID branchId,
            UUID inventoryItemId,
            InventoryMovementType type,
            BigDecimal delta,
            BigDecimal resultingQuantity,
            String reason,
            UUID actorUserId);

    boolean existsForBranchItem(UUID organizationId, UUID branchId, UUID inventoryItemId);

    Page<InventoryMovementRow> findHistory(
            UUID organizationId, UUID branchId, UUID inventoryItemId, Pageable pageable);
}
