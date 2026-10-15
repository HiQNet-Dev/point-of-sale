package com.hiqnet.pos.inventory.stock.application;

import com.hiqnet.pos.branch.domain.Branch;
import com.hiqnet.pos.branch.domain.BranchRepository;
import com.hiqnet.pos.branch.domain.BranchStatus;
import com.hiqnet.pos.inventory.application.InventoryItemUnitGuard;
import com.hiqnet.pos.inventory.domain.InventoryBaseUnit;
import com.hiqnet.pos.inventory.domain.InventoryItem;
import com.hiqnet.pos.inventory.domain.InventoryItemRepository;
import com.hiqnet.pos.inventory.domain.InventoryItemStatus;
import com.hiqnet.pos.inventory.stock.api.InventoryStockMutationResponse;
import com.hiqnet.pos.inventory.stock.api.InventoryStockResponse;
import com.hiqnet.pos.inventory.stock.domain.BranchInventory;
import com.hiqnet.pos.inventory.stock.domain.BranchInventoryRepository;
import com.hiqnet.pos.inventory.stock.domain.InventoryMovementRepository;
import com.hiqnet.pos.inventory.stock.domain.InventoryMovementRow;
import com.hiqnet.pos.inventory.stock.domain.InventoryMovementType;
import com.hiqnet.pos.shared.audit.AuditMetadata;
import com.hiqnet.pos.shared.audit.AuditPort;
import com.hiqnet.pos.shared.audit.AuditRecord;
import com.hiqnet.pos.shared.error.BusinessException;
import com.hiqnet.pos.shared.error.ErrorCodes;
import java.math.BigDecimal;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.UUID;
import org.springframework.data.domain.Page;
import org.springframework.data.domain.PageImpl;
import org.springframework.data.domain.Pageable;
import org.springframework.data.jpa.domain.Specification;
import org.springframework.http.HttpStatus;
import org.springframework.jdbc.core.JdbcTemplate;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

@Service
public class InventoryStockService {

    private final BranchRepository branchRepository;
    private final InventoryItemRepository inventoryItemRepository;
    private final InventoryItemUnitGuard inventoryItemUnitGuard;
    private final BranchInventoryRepository branchInventoryRepository;
    private final InventoryMovementRepository movementRepository;
    private final JdbcTemplate jdbcTemplate;
    private final AuditPort auditPort;

    public InventoryStockService(
            BranchRepository branchRepository,
            InventoryItemRepository inventoryItemRepository,
            InventoryItemUnitGuard inventoryItemUnitGuard,
            BranchInventoryRepository branchInventoryRepository,
            InventoryMovementRepository movementRepository,
            JdbcTemplate jdbcTemplate,
            AuditPort auditPort) {
        this.branchRepository = branchRepository;
        this.inventoryItemRepository = inventoryItemRepository;
        this.inventoryItemUnitGuard = inventoryItemUnitGuard;
        this.branchInventoryRepository = branchInventoryRepository;
        this.movementRepository = movementRepository;
        this.jdbcTemplate = jdbcTemplate;
        this.auditPort = auditPort;
    }

    @Transactional(readOnly = true)
    public Page<InventoryStockResponse> list(
            UUID organizationId,
            UUID branchId,
            String q,
            InventoryItemStatus status,
            InventoryBaseUnit baseUnit,
            Pageable pageable) {
        requireBranch(organizationId, branchId);
        Page<InventoryItem> itemPage = inventoryItemRepository.findAll(
                specification(organizationId, q, status, baseUnit), pageable);
        if (itemPage.isEmpty()) {
            return new PageImpl<>(List.of(), pageable, itemPage.getTotalElements());
        }

        List<UUID> itemIds = itemPage.getContent().stream().map(InventoryItem::getId).toList();
        Map<UUID, BranchInventory> balanceByItem = new LinkedHashMap<>();
        branchInventoryRepository.findAllByBranchIdAndInventoryItemIdIn(branchId, itemIds)
                .forEach(balance -> balanceByItem.put(balance.getInventoryItemId(), balance));
        return itemPage.map(item -> InventoryStockResponse.from(item, balanceByItem.get(item.getId())));
    }

    @Transactional(readOnly = true)
    public InventoryStockResponse get(UUID organizationId, UUID branchId, UUID inventoryItemId) {
        requireBranch(organizationId, branchId);
        InventoryItem item = requireItem(organizationId, inventoryItemId);
        BranchInventory balance = branchInventoryRepository
                .findByOrganizationIdAndBranchIdAndInventoryItemId(organizationId, branchId, inventoryItemId)
                .orElse(null);
        return InventoryStockResponse.from(item, balance);
    }

    @Transactional(readOnly = true)
    public Page<InventoryMovementRow> history(
            UUID organizationId, UUID branchId, UUID inventoryItemId, Pageable pageable) {
        requireBranch(organizationId, branchId);
        requireItem(organizationId, inventoryItemId);
        return movementRepository.findHistory(organizationId, branchId, inventoryItemId, pageable);
    }

    @Transactional
    public InventoryStockMutationResponse open(
            UUID organizationId,
            UUID branchId,
            UUID inventoryItemId,
            UUID actorUserId,
            long expectedVersion,
            String quantityText,
            String reasonText) {
        BigDecimal quantity = InventoryQuantity.parse(quantityText, "quantity");
        if (quantity.signum() < 0) {
            throw invalidQuantity("Opening quantity must be zero or greater");
        }
        return mutate(
                organizationId,
                branchId,
                inventoryItemId,
                actorUserId,
                expectedVersion,
                InventoryMovementType.OPENING,
                quantity,
                optionalReason(reasonText));
    }

    @Transactional
    public InventoryStockMutationResponse adjust(
            UUID organizationId,
            UUID branchId,
            UUID inventoryItemId,
            UUID actorUserId,
            long expectedVersion,
            String deltaText,
            String reasonText) {
        BigDecimal delta = InventoryQuantity.parse(deltaText, "quantityDelta");
        if (delta.signum() == 0) {
            throw invalidQuantity("Adjustment quantityDelta must not be zero");
        }
        return mutate(
                organizationId,
                branchId,
                inventoryItemId,
                actorUserId,
                expectedVersion,
                InventoryMovementType.ADJUSTMENT,
                delta,
                requiredReason(reasonText));
    }

    private InventoryStockMutationResponse mutate(
            UUID organizationId,
            UUID branchId,
            UUID inventoryItemId,
            UUID actorUserId,
            long expectedVersion,
            InventoryMovementType type,
            BigDecimal delta,
            String reason) {
        Branch branch = requireBranch(organizationId, branchId);
        if (branch.getStatus() != BranchStatus.ACTIVE) {
            throw new BusinessException(
                    HttpStatus.CONFLICT, ErrorCodes.BRANCH_INACTIVE, "Branch must be active for inventory mutations");
        }

        InventoryItem item = inventoryItemUnitGuard
                .lockAndMarkReferenced(organizationId, List.of(inventoryItemId))
                .get(inventoryItemId);
        if (item.getStatus() != InventoryItemStatus.ACTIVE) {
            throw new BusinessException(
                    HttpStatus.CONFLICT, ErrorCodes.INVENTORY_ITEM_INACTIVE, "Inventory item must be active for stock mutations");
        }

        branchInventoryRepository.ensureBalanceRow(UUID.randomUUID(), organizationId, branchId, inventoryItemId);
        BranchInventory balance = branchInventoryRepository
                .lockBalance(organizationId, branchId, inventoryItemId)
                .orElseThrow(() -> new IllegalStateException("Inventory balance row could not be initialized"));
        if (balance.getVersion() != expectedVersion) {
            throw new BusinessException(
                    HttpStatus.CONFLICT, ErrorCodes.CONCURRENT_MODIFICATION, "Stock changed; retry with fresh data");
        }

        if (type == InventoryMovementType.OPENING && hasOpening(organizationId, branchId, inventoryItemId)) {
            throw new BusinessException(
                    HttpStatus.CONFLICT, ErrorCodes.INVENTORY_OPENING_ALREADY_EXISTS, "Opening stock already exists");
        }
        if (type == InventoryMovementType.OPENING
                && movementRepository.existsForBranchItem(organizationId, branchId, inventoryItemId)) {
            throw new BusinessException(
                    HttpStatus.CONFLICT,
                    ErrorCodes.INVENTORY_HISTORY_ALREADY_EXISTS,
                    "Opening stock is only allowed before any stock movement");
        }

        BigDecimal resultingQuantity = balance.getQuantityOnHand().add(delta);
        InventoryQuantity.validateRange(resultingQuantity, "resulting quantity");
        validateUnit(item, resultingQuantity);

        balance.applyMovement(resultingQuantity);
        BranchInventory savedBalance = branchInventoryRepository.saveAndFlush(balance);
        UUID movementId = UUID.randomUUID();
        movementRepository.insert(
                movementId,
                organizationId,
                branchId,
                inventoryItemId,
                type,
                delta,
                resultingQuantity,
                reason,
                actorUserId);
        recordAudit(
                organizationId,
                branchId,
                inventoryItemId,
                movementId,
                type,
                delta,
                resultingQuantity,
                reason);
        return InventoryStockMutationResponse.from(inventoryItemId, movementId, type, delta, savedBalance);
    }

    private void recordAudit(
            UUID organizationId,
            UUID branchId,
            UUID inventoryItemId,
            UUID movementId,
            InventoryMovementType type,
            BigDecimal delta,
            BigDecimal resultingQuantity,
            String reason) {
        Map<String, Object> metadata = new LinkedHashMap<>();
        metadata.put("branchId", branchId.toString());
        metadata.put("inventoryItemId", inventoryItemId.toString());
        metadata.put("movementId", movementId.toString());
        metadata.put("quantityDelta", InventoryQuantity.format(delta));
        metadata.put("resultingQuantity", InventoryQuantity.format(resultingQuantity));
        if (reason != null) {
            metadata.put("reason", reason);
        }
        String action = type == InventoryMovementType.OPENING
                ? "inventory.stock.opened"
                : "inventory.stock.adjusted";
        auditPort.record(new AuditRecord(
                organizationId, branchId, action, "inventory_stock", movementId, reason, AuditMetadata.of(metadata)));
    }

    private boolean hasOpening(UUID organizationId, UUID branchId, UUID inventoryItemId) {
        Boolean exists = jdbcTemplate.queryForObject(
                "select exists(select 1 from inventory_movements where organization_id=? and branch_id=? and inventory_item_id=? and type='OPENING')",
                Boolean.class,
                organizationId,
                branchId,
                inventoryItemId);
        return Boolean.TRUE.equals(exists);
    }

    private Branch requireBranch(UUID organizationId, UUID branchId) {
        return branchRepository.findByIdAndOrganizationId(branchId, organizationId)
                .orElseThrow(() -> new BusinessException(
                        HttpStatus.NOT_FOUND, ErrorCodes.BRANCH_NOT_FOUND, "Branch not found"));
    }

    private InventoryItem requireItem(UUID organizationId, UUID inventoryItemId) {
        return inventoryItemRepository.findByIdAndOrganizationId(inventoryItemId, organizationId)
                .orElseThrow(() -> new BusinessException(
                        HttpStatus.NOT_FOUND, ErrorCodes.INVENTORY_ITEM_NOT_FOUND, "Inventory item not found"));
    }

    private static void validateUnit(InventoryItem item, BigDecimal quantity) {
        if (item.getBaseUnit() == InventoryBaseUnit.UNIT
                && !item.isFractionalUnitAllowed()
                && quantity.stripTrailingZeros().scale() > 0) {
            throw invalidQuantity("Fractional stock is not allowed for this UNIT inventory item");
        }
    }

    private static String requiredReason(String text) {
        if (text == null || text.trim().isEmpty()) {
            throw validation("reason is required");
        }
        String reason = text.trim();
        if (reason.length() > 255) {
            throw validation("reason must not exceed 255 characters");
        }
        return reason;
    }

    private static String optionalReason(String text) {
        if (text == null || text.trim().isEmpty()) {
            return null;
        }
        String reason = text.trim();
        if (reason.length() > 255) {
            throw validation("reason must not exceed 255 characters");
        }
        return reason;
    }

    private static BusinessException validation(String message) {
        return new BusinessException(HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, message);
    }

    private static BusinessException invalidQuantity(String message) {
        return new BusinessException(HttpStatus.BAD_REQUEST, ErrorCodes.INVALID_INVENTORY_QUANTITY, message);
    }

    private static Specification<InventoryItem> specification(
            UUID organizationId, String q, InventoryItemStatus status, InventoryBaseUnit baseUnit) {
        return (root, query, criteriaBuilder) -> {
            List<jakarta.persistence.criteria.Predicate> predicates = new ArrayList<>();
            predicates.add(criteriaBuilder.equal(root.get("organization").get("id"), organizationId));
            if (status != null) {
                predicates.add(criteriaBuilder.equal(root.get("status"), status));
            }
            if (baseUnit != null) {
                predicates.add(criteriaBuilder.equal(root.get("baseUnit"), baseUnit));
            }
            if (q != null && !q.trim().isEmpty()) {
                String pattern = "%" + q.trim().toLowerCase(Locale.ROOT) + "%";
                predicates.add(criteriaBuilder.or(
                        criteriaBuilder.like(criteriaBuilder.lower(root.get("code")), pattern),
                        criteriaBuilder.like(criteriaBuilder.lower(root.get("name")), pattern)));
            }
            return criteriaBuilder.and(predicates.toArray(new jakarta.persistence.criteria.Predicate[0]));
        };
    }
}
