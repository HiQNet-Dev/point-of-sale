package com.hiqnet.pos.purchasing.application;

import com.hiqnet.pos.branch.domain.Branch;
import com.hiqnet.pos.branch.domain.BranchRepository;
import com.hiqnet.pos.branch.domain.BranchStatus;
import com.hiqnet.pos.inventory.application.InventoryItemUnitGuard;
import com.hiqnet.pos.inventory.domain.InventoryBaseUnit;
import com.hiqnet.pos.inventory.domain.InventoryItem;
import com.hiqnet.pos.inventory.domain.InventoryItemStatus;
import com.hiqnet.pos.inventory.stock.application.InventoryQuantity;
import com.hiqnet.pos.inventory.stock.domain.BranchInventory;
import com.hiqnet.pos.inventory.stock.domain.BranchInventoryRepository;
import com.hiqnet.pos.inventory.stock.domain.InventoryMovementInsert;
import com.hiqnet.pos.inventory.stock.domain.InventoryMovementRepository;
import com.hiqnet.pos.inventory.stock.domain.InventoryMovementType;
import com.hiqnet.pos.purchasing.domain.PurchaseOrder;
import com.hiqnet.pos.purchasing.domain.PurchaseOrderLine;
import com.hiqnet.pos.purchasing.domain.PurchaseOrderLineRepository;
import com.hiqnet.pos.purchasing.domain.PurchaseOrderRepository;
import com.hiqnet.pos.purchasing.domain.PurchaseOrderStatus;
import com.hiqnet.pos.purchasing.domain.PurchaseReceipt;
import com.hiqnet.pos.purchasing.domain.PurchaseReceiptLine;
import com.hiqnet.pos.purchasing.domain.PurchaseReceiptLineRepository;
import com.hiqnet.pos.purchasing.domain.PurchaseReceiptRepository;
import com.hiqnet.pos.shared.audit.AuditMetadata;
import com.hiqnet.pos.shared.audit.AuditPort;
import com.hiqnet.pos.shared.audit.AuditRecord;
import com.hiqnet.pos.shared.error.BusinessException;
import com.hiqnet.pos.shared.error.ErrorCodes;
import java.math.BigDecimal;
import java.math.RoundingMode;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.time.Clock;
import java.time.Instant;
import java.util.ArrayList;
import java.util.Collection;
import java.util.Comparator;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.UUID;
import java.util.function.Function;
import java.util.stream.Collectors;
import org.springframework.http.HttpStatus;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

/**
 * Casos de uso de recepción de mercancía de una orden de compra.
 *
 * <p>Registrar una recepción confirma stock real en una única transacción: valida
 * autenticación/autorización en el controlador, resuelve idempotencia, valida el
 * estado y la versión de la orden, rechaza sobrerrecepción, crea balances
 * faltantes, escribe movimientos {@code PURCHASE_RECEIPT} append-only, persiste
 * cabecera y líneas de recepción, actualiza el estado de la orden y audita. Todo
 * confirma o se revierte junto.</p>
 *
 * <p>Orden global de bloqueos: {@code PurchaseOrder → InventoryItem →
 * BranchInventory}. La orden se bloquea primero para serializar recepción,
 * cancelación y emisión; los artículos se bloquean en orden ascendente por
 * {@code id} mediante {@link InventoryItemUnitGuard}; los balances se bloquean
 * por artículo ascendente. La comprobación de idempotencia ocurre bajo el bloqueo
 * de la orden, de modo que dos reintentos simultáneos se serializan y solo uno
 * registra stock y auditoría.</p>
 */
@Service
public class PurchaseReceiptService {

    private static final int MAX_IDEMPOTENCY_KEY = 128;

    private final PurchaseOrderRepository purchaseOrderRepository;
    private final PurchaseOrderLineRepository purchaseOrderLineRepository;
    private final PurchaseReceiptRepository purchaseReceiptRepository;
    private final PurchaseReceiptLineRepository purchaseReceiptLineRepository;
    private final BranchRepository branchRepository;
    private final InventoryItemUnitGuard inventoryItemUnitGuard;
    private final BranchInventoryRepository branchInventoryRepository;
    private final InventoryMovementRepository movementRepository;
    private final AuditPort auditPort;
    private final Clock clock;

    public PurchaseReceiptService(
            PurchaseOrderRepository purchaseOrderRepository,
            PurchaseOrderLineRepository purchaseOrderLineRepository,
            PurchaseReceiptRepository purchaseReceiptRepository,
            PurchaseReceiptLineRepository purchaseReceiptLineRepository,
            BranchRepository branchRepository,
            InventoryItemUnitGuard inventoryItemUnitGuard,
            BranchInventoryRepository branchInventoryRepository,
            InventoryMovementRepository movementRepository,
            AuditPort auditPort,
            Clock clock) {
        this.purchaseOrderRepository = purchaseOrderRepository;
        this.purchaseOrderLineRepository = purchaseOrderLineRepository;
        this.purchaseReceiptRepository = purchaseReceiptRepository;
        this.purchaseReceiptLineRepository = purchaseReceiptLineRepository;
        this.branchRepository = branchRepository;
        this.inventoryItemUnitGuard = inventoryItemUnitGuard;
        this.branchInventoryRepository = branchInventoryRepository;
        this.movementRepository = movementRepository;
        this.auditPort = auditPort;
        this.clock = clock;
    }

    /**
     * Registra la recepción (parcial o completa) de una orden emitida.
     *
     * @param organizationId organización propietaria
     * @param branchId sucursal de destino de la ruta
     * @param purchaseOrderId orden recibida
     * @param idempotencyKey clave de idempotencia provista por el cliente
     * @param expectedVersion versión de la orden leída por el cliente (solo solicitudes nuevas)
     * @param actorUserId actor autenticado que confirma la recepción
     * @param lines cantidades recibidas por partida
     * @return recepción confirmada (o el resultado original en un reintento idéntico)
     * @throws BusinessException si el ámbito, el estado, la versión, las cantidades o la clave son inválidos
     */
    @Transactional
    public PurchaseReceiptOutcome post(
            UUID organizationId,
            UUID branchId,
            UUID purchaseOrderId,
            String idempotencyKey,
            long expectedVersion,
            UUID actorUserId,
            List<PurchaseReceiptLineCommand> lines) {
        String key = requireIdempotencyKey(idempotencyKey);
        PurchaseOrder order = purchaseOrderRepository
                .findByIdAndOrganizationIdAndBranchIdForUpdate(purchaseOrderId, organizationId, branchId)
                .orElseThrow(PurchaseReceiptService::orderNotFound);

        Optional<PurchaseReceipt> existing = purchaseReceiptRepository
                .findByOrganizationIdAndPurchaseOrderIdAndIdempotencyKey(organizationId, purchaseOrderId, key);
        if (existing.isPresent()) {
            return replay(existing.get(), order, lines);
        }

        requireReceivable(order);
        requireExpectedVersion(order, expectedVersion);
        requireActiveBranch(organizationId, branchId);

        List<PurchaseOrderLine> orderLines =
                purchaseOrderLineRepository.findByPurchaseOrderIdOrderByLineNumberAsc(purchaseOrderId);
        Map<UUID, PurchaseOrderLine> orderLineById = orderLines.stream()
                .collect(Collectors.toMap(PurchaseOrderLine::getId, Function.identity()));
        Map<UUID, BigDecimal> requested = validateRequestedLines(lines, orderLineById);
        String fingerprint = fingerprint(requested);

        Map<UUID, BigDecimal> alreadyReceived = receivedByLine(organizationId, purchaseOrderId);
        rejectOverReceipt(requested, alreadyReceived, orderLineById);

        List<UUID> itemIds = requested.keySet().stream()
                .map(id -> orderLineById.get(id).getInventoryItemId())
                .distinct()
                .sorted()
                .toList();
        Map<UUID, InventoryItem> items = inventoryItemUnitGuard.lockAndMarkReferenced(organizationId, itemIds);
        validateItems(orderLineById, requested.keySet(), items);

        Instant now = clock.instant();
        PurchaseReceipt receipt = purchaseReceiptRepository.saveAndFlush(PurchaseReceipt.create(
                organizationId, branchId, purchaseOrderId, key, fingerprint, actorUserId, now));

        List<PurchaseReceiptLine> receiptLines = new ArrayList<>(requested.size());
        for (UUID lineId : sortedLineIds(requested)) {
            PurchaseOrderLine line = orderLineById.get(lineId);
            receiptLines.add(PurchaseReceiptLine.create(
                    organizationId,
                    branchId,
                    receipt.getId(),
                    purchaseOrderId,
                    line.getId(),
                    line.getInventoryItemId(),
                    requested.get(lineId)));
        }
        purchaseReceiptLineRepository.saveAllAndFlush(receiptLines);

        applyStock(organizationId, branchId, receiptLines, items, actorUserId);

        PurchaseOrderStatus newStatus = nextStatus(orderLines, alreadyReceived, requested);
        order.applyReceipt(newStatus, now);
        purchaseOrderRepository.saveAndFlush(order);

        recordAudit(organizationId, branchId, order.getId(), receipt.getId(), receiptLines.size());
        return new PurchaseReceiptOutcome(new PurchaseReceiptDetail(receipt, receiptLines, order), false);
    }

    /**
     * Obtiene una recepción con sus líneas dentro del ámbito organización-sucursal.
     *
     * @param organizationId organización solicitada
     * @param branchId sucursal de destino de la ruta
     * @param purchaseOrderId orden de la recepción
     * @param purchaseReceiptId recepción solicitada
     * @return recepción con sus líneas y la orden vigente
     */
    @Transactional(readOnly = true)
    public PurchaseReceiptDetail get(
            UUID organizationId, UUID branchId, UUID purchaseOrderId, UUID purchaseReceiptId) {
        PurchaseOrder order = requireOrder(organizationId, branchId, purchaseOrderId);
        PurchaseReceipt receipt = purchaseReceiptRepository
                .findByIdAndOrganizationIdAndBranchId(purchaseReceiptId, organizationId, branchId)
                .filter(candidate -> candidate.getPurchaseOrderId().equals(purchaseOrderId))
                .orElseThrow(PurchaseReceiptService::receiptNotFound);
        return new PurchaseReceiptDetail(receipt, receiptLines(receipt.getId()), order);
    }

    /**
     * Lista las recepciones confirmadas de una orden.
     *
     * @param organizationId organización solicitada
     * @param branchId sucursal de destino de la ruta
     * @param purchaseOrderId orden consultada
     * @return recepciones ordenadas cronológicamente
     */
    @Transactional(readOnly = true)
    public List<PurchaseReceipt> list(UUID organizationId, UUID branchId, UUID purchaseOrderId) {
        requireOrder(organizationId, branchId, purchaseOrderId);
        return purchaseReceiptRepository.findByOrganizationIdAndPurchaseOrderIdOrderByReceivedAtAscIdAsc(
                organizationId, purchaseOrderId);
    }

    /**
     * Calcula cantidades solicitadas, recibidas y pendientes por partida.
     *
     * @param organizationId organización solicitada
     * @param branchId sucursal de destino de la ruta
     * @param purchaseOrderId orden consultada
     * @return estado de recepción por partida
     */
    @Transactional(readOnly = true)
    public PurchaseOrderReceiptStatus receiptStatus(UUID organizationId, UUID branchId, UUID purchaseOrderId) {
        PurchaseOrder order = requireOrder(organizationId, branchId, purchaseOrderId);
        List<PurchaseOrderLine> lines =
                purchaseOrderLineRepository.findByPurchaseOrderIdOrderByLineNumberAsc(purchaseOrderId);
        Map<UUID, BigDecimal> received = receivedByLine(organizationId, purchaseOrderId);
        List<PurchaseOrderReceiptStatus.LineStatus> statuses = new ArrayList<>(lines.size());
        for (PurchaseOrderLine line : lines) {
            BigDecimal lineReceived = received.getOrDefault(line.getId(), BigDecimal.ZERO.setScale(6));
            statuses.add(new PurchaseOrderReceiptStatus.LineStatus(
                    line, lineReceived, line.getQuantity().subtract(lineReceived)));
        }
        return new PurchaseOrderReceiptStatus(order, statuses);
    }

    private PurchaseReceiptOutcome replay(
            PurchaseReceipt receipt, PurchaseOrder order, List<PurchaseReceiptLineCommand> lines) {
        Map<UUID, BigDecimal> requested = new LinkedHashMap<>();
        if (lines != null) {
            for (PurchaseReceiptLineCommand line : lines) {
                if (line != null && line.purchaseOrderLineId() != null && line.quantity() != null) {
                    requested.putIfAbsent(line.purchaseOrderLineId(), line.quantity());
                }
            }
        }
        if (!fingerprint(requested).equals(receipt.getPayloadFingerprint())) {
            throw new BusinessException(
                    HttpStatus.CONFLICT,
                    ErrorCodes.PURCHASE_RECEIPT_IDEMPOTENCY_CONFLICT,
                    "The idempotency key was already used with a different payload");
        }
        return new PurchaseReceiptOutcome(
                new PurchaseReceiptDetail(receipt, receiptLines(receipt.getId()), order), true);
    }

    private void applyStock(
            UUID organizationId,
            UUID branchId,
            List<PurchaseReceiptLine> receiptLines,
            Map<UUID, InventoryItem> items,
            UUID actorUserId) {
        List<UUID> itemIds =
                receiptLines.stream().map(PurchaseReceiptLine::getInventoryItemId).distinct().sorted().toList();
        branchInventoryRepository.ensureBalances(organizationId, branchId, joinIds(itemIds));
        List<BranchInventory> balances = branchInventoryRepository.lockBalances(organizationId, branchId, itemIds);
        if (balances.size() != itemIds.size()) {
            throw new IllegalStateException("Branch inventory rows could not be locked for receipt");
        }
        Map<UUID, BranchInventory> balanceByItem = new LinkedHashMap<>();
        balances.forEach(balance -> balanceByItem.put(balance.getInventoryItemId(), balance));

        List<InventoryMovementInsert> movements = new ArrayList<>(receiptLines.size());
        for (PurchaseReceiptLine receiptLine : receiptLines) {
            BranchInventory balance = balanceByItem.get(receiptLine.getInventoryItemId());
            BigDecimal resulting = balance.getQuantityOnHand().add(receiptLine.getQuantityReceived());
            InventoryQuantity.validateRange(resulting, "resulting quantity");
            validateUnit(items.get(receiptLine.getInventoryItemId()), resulting);
            balance.applyMovement(resulting);
            movements.add(new InventoryMovementInsert(
                    UUID.randomUUID(),
                    organizationId,
                    branchId,
                    receiptLine.getInventoryItemId(),
                    InventoryMovementType.PURCHASE_RECEIPT,
                    receiptLine.getQuantityReceived(),
                    resulting,
                    null,
                    actorUserId,
                    null,
                    receiptLine.getId()));
        }
        branchInventoryRepository.saveAllAndFlush(balances);
        movementRepository.insertBatch(movements);
    }

    private Map<UUID, BigDecimal> validateRequestedLines(
            List<PurchaseReceiptLineCommand> lines, Map<UUID, PurchaseOrderLine> orderLineById) {
        if (lines == null || lines.isEmpty()) {
            throw validation("lines must contain at least one receipt line");
        }
        Map<UUID, BigDecimal> requested = new LinkedHashMap<>();
        for (PurchaseReceiptLineCommand line : lines) {
            if (line == null || line.purchaseOrderLineId() == null) {
                throw validation("purchaseOrderLineId is required");
            }
            PurchaseOrderLine orderLine = orderLineById.get(line.purchaseOrderLineId());
            if (orderLine == null) {
                throw new BusinessException(
                        HttpStatus.NOT_FOUND,
                        ErrorCodes.PURCHASE_ORDER_LINE_NOT_FOUND,
                        "Purchase order line not found in this order");
            }
            if (line.quantity() == null || line.quantity().signum() <= 0) {
                throw validation("quantity must be greater than zero");
            }
            if (requested.putIfAbsent(line.purchaseOrderLineId(), line.quantity()) != null) {
                throw validation("duplicate purchaseOrderLineId in receipt lines");
            }
        }
        return requested;
    }

    private void rejectOverReceipt(
            Map<UUID, BigDecimal> requested,
            Map<UUID, BigDecimal> alreadyReceived,
            Map<UUID, PurchaseOrderLine> orderLineById) {
        List<String> violations = new ArrayList<>();
        for (Map.Entry<UUID, BigDecimal> entry : requested.entrySet()) {
            PurchaseOrderLine line = orderLineById.get(entry.getKey());
            BigDecimal previouslyReceived = alreadyReceived.getOrDefault(entry.getKey(), BigDecimal.ZERO.setScale(6));
            BigDecimal total = previouslyReceived.add(entry.getValue());
            if (total.compareTo(line.getQuantity()) > 0) {
                violations.add("lineNumber=" + line.getLineNumber()
                        + " ordered=" + InventoryQuantity.format(line.getQuantity())
                        + " received=" + InventoryQuantity.format(previouslyReceived)
                        + " attempted=" + InventoryQuantity.format(entry.getValue()));
            }
        }
        if (!violations.isEmpty()) {
            throw new BusinessException(
                    HttpStatus.CONFLICT,
                    ErrorCodes.PURCHASE_RECEIPT_EXCEEDS_ORDERED,
                    "The receipt would exceed the ordered quantity",
                    violations);
        }
    }

    private void validateItems(
            Map<UUID, PurchaseOrderLine> orderLineById, Collection<UUID> requestedLineIds, Map<UUID, InventoryItem> items) {
        Collection<UUID> itemIds = new LinkedHashSet<>();
        for (UUID lineId : requestedLineIds) {
            itemIds.add(orderLineById.get(lineId).getInventoryItemId());
        }
        for (UUID itemId : itemIds) {
            InventoryItem item = items.get(itemId);
            if (item == null) {
                throw new BusinessException(
                        HttpStatus.NOT_FOUND, ErrorCodes.INVENTORY_ITEM_NOT_FOUND, "Inventory item not found");
            }
            if (item.getStatus() != InventoryItemStatus.ACTIVE) {
                throw new BusinessException(
                        HttpStatus.CONFLICT, ErrorCodes.INVENTORY_ITEM_INACTIVE, "The inventory item is inactive");
            }
            for (UUID lineId : requestedLineIds) {
                PurchaseOrderLine line = orderLineById.get(lineId);
                if (line.getInventoryItemId().equals(itemId) && line.getUnitSnapshot() != item.getBaseUnit()) {
                    throw new BusinessException(
                            HttpStatus.CONFLICT,
                            ErrorCodes.PURCHASE_ORDER_UNIT_CHANGED,
                            "The inventory item base unit changed since the line was captured");
                }
            }
        }
    }

    private PurchaseOrderStatus nextStatus(
            List<PurchaseOrderLine> orderLines,
            Map<UUID, BigDecimal> alreadyReceived,
            Map<UUID, BigDecimal> requested) {
        for (PurchaseOrderLine line : orderLines) {
            BigDecimal total = alreadyReceived.getOrDefault(line.getId(), BigDecimal.ZERO.setScale(6))
                    .add(requested.getOrDefault(line.getId(), BigDecimal.ZERO.setScale(6)));
            if (total.compareTo(line.getQuantity()) < 0) {
                return PurchaseOrderStatus.PARTIALLY_RECEIVED;
            }
        }
        return PurchaseOrderStatus.RECEIVED;
    }

    private Map<UUID, BigDecimal> receivedByLine(UUID organizationId, UUID purchaseOrderId) {
        Map<UUID, BigDecimal> received = new LinkedHashMap<>();
        for (PurchaseReceiptLineRepository.LineReceived row :
                purchaseReceiptLineRepository.sumReceivedByOrder(organizationId, purchaseOrderId)) {
            received.put(row.getLineId(), row.getReceived());
        }
        return received;
    }

    private List<PurchaseReceiptLine> receiptLines(UUID purchaseReceiptId) {
        return purchaseReceiptLineRepository.findByPurchaseReceiptIdOrderByPurchaseOrderLineIdAsc(purchaseReceiptId);
    }

    private void recordAudit(
            UUID organizationId, UUID branchId, UUID orderId, UUID receiptId, int lineCount) {
        Map<String, Object> metadata = new LinkedHashMap<>();
        metadata.put("purchaseOrderId", orderId.toString());
        metadata.put("purchaseReceiptId", receiptId.toString());
        metadata.put("branchId", branchId.toString());
        metadata.put("lineCount", lineCount);
        auditPort.record(AuditRecord.ofBranch(
                organizationId,
                branchId,
                "purchase_receipt.posted",
                "purchase_receipt",
                receiptId,
                AuditMetadata.of(metadata)));
    }

    private PurchaseOrder requireOrder(UUID organizationId, UUID branchId, UUID purchaseOrderId) {
        return purchaseOrderRepository
                .findByIdAndOrganizationIdAndBranchId(purchaseOrderId, organizationId, branchId)
                .orElseThrow(PurchaseReceiptService::orderNotFound);
    }

    private void requireActiveBranch(UUID organizationId, UUID branchId) {
        Branch branch = branchRepository.findByIdAndOrganizationId(branchId, organizationId)
                .orElseThrow(() -> new BusinessException(
                        HttpStatus.NOT_FOUND, ErrorCodes.BRANCH_NOT_FOUND, "Branch not found"));
        if (branch.getStatus() != BranchStatus.ACTIVE) {
            throw new BusinessException(
                    HttpStatus.CONFLICT, ErrorCodes.BRANCH_INACTIVE, "Branch must be active to receive stock");
        }
    }

    private static void requireReceivable(PurchaseOrder order) {
        switch (order.getStatus()) {
            case ORDERED, PARTIALLY_RECEIVED -> {
                // Receivable.
            }
            case DRAFT -> throw new BusinessException(
                    HttpStatus.CONFLICT,
                    ErrorCodes.PURCHASE_ORDER_NOT_ISSUED,
                    "The purchase order must be issued before it can receive goods");
            case RECEIVED -> throw new BusinessException(
                    HttpStatus.CONFLICT,
                    ErrorCodes.PURCHASE_ORDER_ALREADY_RECEIVED,
                    "The purchase order is already fully received");
            case CANCELLED -> throw new BusinessException(
                    HttpStatus.CONFLICT,
                    ErrorCodes.PURCHASE_ORDER_ALREADY_CANCELLED,
                    "The purchase order is cancelled");
        }
    }

    private static void requireExpectedVersion(PurchaseOrder order, long expectedVersion) {
        if (order.getVersion() != expectedVersion) {
            throw new BusinessException(
                    HttpStatus.CONFLICT,
                    ErrorCodes.CONCURRENT_MODIFICATION,
                    "The purchase order was modified concurrently; retry with fresh data");
        }
    }

    private static String requireIdempotencyKey(String value) {
        if (value == null || value.isBlank()) {
            throw validation("idempotencyKey is required");
        }
        String key = value.trim();
        if (key.length() > MAX_IDEMPOTENCY_KEY) {
            throw validation("idempotencyKey must not exceed " + MAX_IDEMPOTENCY_KEY + " characters");
        }
        return key;
    }

    private static void validateUnit(InventoryItem item, BigDecimal quantity) {
        if (item.getBaseUnit() == InventoryBaseUnit.UNIT
                && !item.isFractionalUnitAllowed()
                && quantity.stripTrailingZeros().scale() > 0) {
            throw new BusinessException(
                    HttpStatus.CONFLICT,
                    ErrorCodes.INVALID_INVENTORY_QUANTITY,
                    "Fractional stock is not allowed for this UNIT inventory item");
        }
    }

    private static List<UUID> sortedLineIds(Map<UUID, BigDecimal> requested) {
        return requested.keySet().stream().sorted(Comparator.naturalOrder()).toList();
    }

    private static String fingerprint(Map<UUID, BigDecimal> requested) {
        String canonical = requested.entrySet().stream()
                .sorted(Map.Entry.comparingByKey())
                .map(entry -> entry.getKey() + ":" + entry.getValue().setScale(6, RoundingMode.UNNECESSARY).toPlainString())
                .collect(Collectors.joining("|"));
        return sha256(canonical);
    }

    private static String sha256(String value) {
        try {
            byte[] digest = MessageDigest.getInstance("SHA-256").digest(value.getBytes(StandardCharsets.UTF_8));
            StringBuilder hex = new StringBuilder(digest.length * 2);
            for (byte octet : digest) {
                hex.append(Character.forDigit((octet >> 4) & 0xF, 16));
                hex.append(Character.forDigit(octet & 0xF, 16));
            }
            return hex.toString();
        } catch (NoSuchAlgorithmException exception) {
            throw new IllegalStateException("SHA-256 is not available", exception);
        }
    }

    private static String joinIds(Collection<UUID> ids) {
        StringBuilder builder = new StringBuilder();
        for (UUID id : ids) {
            if (builder.length() > 0) {
                builder.append(',');
            }
            builder.append(id);
        }
        return builder.toString();
    }

    private static BusinessException validation(String message) {
        return new BusinessException(HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, message);
    }

    private static BusinessException orderNotFound() {
        return new BusinessException(
                HttpStatus.NOT_FOUND, ErrorCodes.PURCHASE_ORDER_NOT_FOUND, "Purchase order not found");
    }

    private static BusinessException receiptNotFound() {
        return new BusinessException(
                HttpStatus.NOT_FOUND, ErrorCodes.PURCHASE_RECEIPT_NOT_FOUND, "Purchase receipt not found");
    }
}
