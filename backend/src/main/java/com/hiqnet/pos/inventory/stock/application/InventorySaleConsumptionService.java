package com.hiqnet.pos.inventory.stock.application;

import com.hiqnet.pos.inventory.application.InventoryItemUnitGuard;
import com.hiqnet.pos.inventory.stock.domain.BranchInventory;
import com.hiqnet.pos.inventory.stock.domain.BranchInventoryRepository;
import com.hiqnet.pos.inventory.stock.domain.InventoryMovementInsert;
import com.hiqnet.pos.inventory.stock.domain.InventoryMovementRepository;
import com.hiqnet.pos.inventory.stock.domain.InventoryMovementType;
import com.hiqnet.pos.organization.domain.InventoryNegativeStockPolicy;
import com.hiqnet.pos.shared.error.BusinessException;
import com.hiqnet.pos.shared.error.ErrorCodes;
import java.math.BigDecimal;
import java.util.ArrayList;
import java.util.Collection;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.UUID;
import org.springframework.http.HttpStatus;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

/**
 * Atomic branch Inventory consumption for one completed checkout. Runs inside
 * the caller's transaction so Sale completion, payments, stock and movements
 * commit or roll back together.
 *
 * Lock order is authoritative: the CashShift lock is already held by the caller,
 * these balances are locked here (deterministically ordered by Inventory item),
 * and only afterwards does the caller claim the optimistic Sale aggregate.
 * Balances are ensured and locked in bulk; shortages are evaluated for every
 * item before any balance is mutated, so STRICT consumption is all-or-nothing.
 */
@Service
public class InventorySaleConsumptionService {

    private final BranchInventoryRepository branchInventoryRepository;
    private final InventoryMovementRepository movementRepository;
    private final InventoryItemUnitGuard inventoryItemUnitGuard;

    public InventorySaleConsumptionService(
            BranchInventoryRepository branchInventoryRepository,
            InventoryMovementRepository movementRepository,
            InventoryItemUnitGuard inventoryItemUnitGuard) {
        this.branchInventoryRepository = branchInventoryRepository;
        this.movementRepository = movementRepository;
        this.inventoryItemUnitGuard = inventoryItemUnitGuard;
    }

    @Transactional
    public void consume(
            UUID organizationId,
            UUID branchId,
            UUID saleId,
            UUID actorUserId,
            InventoryNegativeStockPolicy policy,
            List<InventoryConsumptionRequirement> requirements) {
        if (requirements.isEmpty()) {
            return;
        }
        Map<UUID, InventoryConsumptionRequirement> byItem = new LinkedHashMap<>();
        for (InventoryConsumptionRequirement requirement : requirements) {
            byItem.put(requirement.inventoryItemId(), requirement);
        }
        Collection<UUID> itemIds = byItem.keySet();

        // Bloqueo de artículos antes de los balances: orden global
        // InventoryItem -> BranchInventory, coherente con las mutaciones manuales
        // de stock, para evitar deadlocks con el FK de inventory_movements.
        inventoryItemUnitGuard.lockAndMarkReferenced(organizationId, itemIds);

        branchInventoryRepository.ensureBalances(organizationId, branchId, joinIds(itemIds));
        List<BranchInventory> locked = branchInventoryRepository.lockBalances(organizationId, branchId, itemIds);
        if (locked.size() != itemIds.size()) {
            throw new IllegalStateException("Branch inventory rows could not be locked for consumption");
        }

        // Evaluate every shortage before mutating anything: STRICT is all-or-nothing.
        if (policy == InventoryNegativeStockPolicy.STRICT) {
            List<String> shortages = new ArrayList<>();
            for (BranchInventory balance : locked) {
                InventoryConsumptionRequirement requirement = byItem.get(balance.getInventoryItemId());
                if (balance.getQuantityOnHand().compareTo(requirement.requiredQuantity()) < 0) {
                    shortages.add("code=" + requirement.inventoryItemCodeSnapshot()
                            + " required=" + InventoryQuantity.format(requirement.requiredQuantity())
                            + " available=" + InventoryQuantity.format(balance.getQuantityOnHand()));
                }
            }
            if (!shortages.isEmpty()) {
                throw new BusinessException(
                        HttpStatus.CONFLICT,
                        ErrorCodes.INSUFFICIENT_INVENTORY,
                        "Insufficient inventory to complete the sale",
                        shortages);
            }
        }

        // Apply in the canonical locked order so movement rows are deterministic.
        List<InventoryMovementInsert> movements = new ArrayList<>(locked.size());
        for (BranchInventory balance : locked) {
            InventoryConsumptionRequirement requirement = byItem.get(balance.getInventoryItemId());
            BigDecimal delta = requirement.requiredQuantity().negate();
            BigDecimal resulting = balance.getQuantityOnHand().add(delta);
            InventoryQuantity.validateRange(resulting, "resulting quantity");
            balance.applyMovement(resulting);
            movements.add(new InventoryMovementInsert(
                    UUID.randomUUID(),
                    organizationId,
                    branchId,
                    balance.getInventoryItemId(),
                    InventoryMovementType.SALE_CONSUMPTION,
                    delta,
                    resulting,
                    null,
                    actorUserId,
                    saleId));
        }
        branchInventoryRepository.saveAll(locked);
        branchInventoryRepository.flush();
        movementRepository.insertBatch(movements);
    }

    private static String joinIds(Collection<UUID> itemIds) {
        StringBuilder builder = new StringBuilder();
        for (UUID id : itemIds) {
            if (builder.length() > 0) {
                builder.append(',');
            }
            builder.append(id);
        }
        return builder.toString();
    }
}
