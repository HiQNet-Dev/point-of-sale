package com.hiqnet.pos.purchasing.application;

import com.hiqnet.pos.branch.domain.Branch;
import com.hiqnet.pos.branch.domain.BranchRepository;
import com.hiqnet.pos.branch.domain.BranchStatus;
import com.hiqnet.pos.inventory.application.InventoryItemUnitGuard;
import com.hiqnet.pos.inventory.domain.InventoryBaseUnit;
import com.hiqnet.pos.inventory.domain.InventoryItem;
import com.hiqnet.pos.inventory.domain.InventoryItemRepository;
import com.hiqnet.pos.inventory.domain.InventoryItemStatus;
import com.hiqnet.pos.organization.domain.Organization;
import com.hiqnet.pos.organization.domain.OrganizationRepository;
import com.hiqnet.pos.purchasing.domain.PurchaseOrder;
import com.hiqnet.pos.purchasing.domain.PurchaseOrderLine;
import com.hiqnet.pos.purchasing.domain.PurchaseOrderLineRepository;
import com.hiqnet.pos.purchasing.domain.PurchaseOrderRepository;
import com.hiqnet.pos.purchasing.domain.PurchaseOrderStatus;
import com.hiqnet.pos.purchasing.domain.Supplier;
import com.hiqnet.pos.purchasing.domain.SupplierRepository;
import com.hiqnet.pos.purchasing.domain.SupplierStatus;
import com.hiqnet.pos.shared.audit.AuditMetadata;
import com.hiqnet.pos.shared.audit.AuditPort;
import com.hiqnet.pos.shared.audit.AuditRecord;
import com.hiqnet.pos.shared.error.BusinessException;
import com.hiqnet.pos.shared.error.ErrorCodes;
import jakarta.persistence.criteria.Predicate;
import java.time.Clock;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Objects;
import java.util.UUID;
import java.util.function.Function;
import java.util.stream.Collectors;
import org.springframework.data.domain.Page;
import org.springframework.data.domain.Pageable;
import org.springframework.data.jpa.domain.Specification;
import org.springframework.http.HttpStatus;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

/**
 * Casos de uso de la administración de órdenes de compra en borrador.
 *
 * <p>Una orden es un agregado cuyo punto de sincronización es la fila de
 * cabecera: toda mutación de cabecera o de partidas toma primero el bloqueo
 * pesimista de la cabecera ({@code SELECT ... FOR UPDATE}), valida la
 * {@code expectedVersion} recibida y deja el agregado sucio para que
 * {@code @Version} incremente la versión al hacer flush. Así dos escrituras
 * concurrentes sobre la misma orden se serializan (la segunda observa la versión
 * nueva y responde {@code 409 CONCURRENT_MODIFICATION}) y no se pierde ninguna
 * actualización. El orden de adquisición de bloqueos es siempre
 * {@code PurchaseOrder → Supplier → InventoryItem → PurchaseOrderLine → (futuro)
 * BranchInventory}, igual que la convención de Sales.</p>
 *
 * <p>La emisión ({@code issue}) y la cancelación ({@code cancel}) son
 * transiciones privilegiadas que reutilizan el mismo bloqueo de cabecera y la
 * misma validación de {@code expectedVersion}: al emitir se valida que la orden
 * tenga partidas, que el proveedor y los artículos sigan activos y que la unidad
 * base de cada artículo no haya cambiado desde la captura; al cancelar se
 * conserva el historial de emisión. Ninguna transición escribe movimientos de
 * inventario.</p>
 */
@Service
public class PurchaseOrderService {

    private final PurchaseOrderRepository purchaseOrderRepository;
    private final PurchaseOrderLineRepository purchaseOrderLineRepository;
    private final OrganizationRepository organizationRepository;
    private final BranchRepository branchRepository;
    private final SupplierRepository supplierRepository;
    private final InventoryItemRepository inventoryItemRepository;
    private final InventoryItemUnitGuard inventoryItemUnitGuard;
    private final AuditPort auditPort;
    private final Clock clock;

    public PurchaseOrderService(
            PurchaseOrderRepository purchaseOrderRepository,
            PurchaseOrderLineRepository purchaseOrderLineRepository,
            OrganizationRepository organizationRepository,
            BranchRepository branchRepository,
            SupplierRepository supplierRepository,
            InventoryItemRepository inventoryItemRepository,
            InventoryItemUnitGuard inventoryItemUnitGuard,
            AuditPort auditPort,
            Clock clock) {
        this.purchaseOrderRepository = purchaseOrderRepository;
        this.purchaseOrderLineRepository = purchaseOrderLineRepository;
        this.organizationRepository = organizationRepository;
        this.branchRepository = branchRepository;
        this.supplierRepository = supplierRepository;
        this.inventoryItemRepository = inventoryItemRepository;
        this.inventoryItemUnitGuard = inventoryItemUnitGuard;
        this.auditPort = auditPort;
        this.clock = clock;
    }

    /**
     * Crea una orden en borrador con la moneda de la organización y los snapshots del proveedor.
     *
     * @param organizationId organización propietaria
     * @param branchId sucursal de destino, dentro de la organización
     * @param code código comercial del borrador
     * @param supplierId proveedor activo de la misma organización
     * @param actorUserId usuario autenticado que crea la orden
     * @return orden creada con su detalle (sin partidas)
     * @throws BusinessException si la organización, sucursal o proveedor no existen en el ámbito,
     *         la sucursal o el proveedor están inactivos, o el código ya está ocupado
     */
    @Transactional
    public PurchaseOrderDetail create(
            UUID organizationId, UUID branchId, String code, UUID supplierId, UUID actorUserId) {
        Organization organization = requireOrganization(organizationId);
        Branch branch = requireBranch(organizationId, branchId);
        requireActiveBranch(branch);
        Supplier supplier = requireSupplier(organizationId, supplierId);
        requireActiveSupplier(supplier);

        String normalizedCode = code == null ? null : code.trim().toLowerCase(Locale.ROOT);
        if (normalizedCode != null
                && purchaseOrderRepository.existsByOrganizationIdAndCode(organizationId, normalizedCode)) {
            throw duplicateCode();
        }
        PurchaseOrder order = PurchaseOrder.create(
                organization, branchId, supplier, normalizedCode, organization.getCurrency(), actorUserId);
        PurchaseOrder saved = purchaseOrderRepository.saveAndFlush(order);
        auditPort.record(AuditRecord.ofBranch(
                organizationId,
                branchId,
                "purchase_order.created",
                "purchase_order",
                saved.getId(),
                AuditMetadata.of("code", saved.getCode())));
        return detail(saved);
    }

    /**
     * Obtiene una orden con sus partidas dentro del ámbito organización-sucursal.
     *
     * @param organizationId organización solicitada
     * @param branchId sucursal de destino de la ruta
     * @param purchaseOrderId identificador de la orden
     * @return orden con sus partidas
     * @throws BusinessException si la organización, la sucursal o la orden no existen en el ámbito
     */
    @Transactional(readOnly = true)
    public PurchaseOrderDetail get(UUID organizationId, UUID branchId, UUID purchaseOrderId) {
        requireOrganization(organizationId);
        requireBranch(organizationId, branchId);
        PurchaseOrder order = requireOrder(organizationId, branchId, purchaseOrderId);
        return detail(order);
    }

    /**
     * Lista órdenes de la sucursal con filtros opcionales y paginación.
     *
     * @param organizationId organización solicitada
     * @param branchId sucursal de destino de la ruta
     * @param status filtro de estado opcional
     * @param supplierId filtro de proveedor opcional
     * @param q texto opcional que busca en el código comercial
     * @param pageable página y orden fijados por el controlador
     * @return página de órdenes scoped
     * @throws BusinessException si la organización o la sucursal no existen en el ámbito
     */
    @Transactional(readOnly = true)
    public Page<PurchaseOrder> list(
            UUID organizationId,
            UUID branchId,
            PurchaseOrderStatus status,
            UUID supplierId,
            String q,
            Pageable pageable) {
        requireOrganization(organizationId);
        requireBranch(organizationId, branchId);
        return purchaseOrderRepository.findAll(
                specification(organizationId, branchId, status, supplierId, q), pageable);
    }

    /**
     * Actualiza la cabecera editable de un borrador.
     *
     * <p>Una actualización sin cambios reales es idempotente: no incrementa la
     * versión ni escribe auditoría.</p>
     *
     * @param organizationId organización propietaria
     * @param branchId sucursal de destino de la ruta
     * @param purchaseOrderId identificador de la orden
     * @param command nuevo proveedor opcional y versión esperada
     * @return orden actualizada
     * @throws BusinessException si la orden no existe, no está en DRAFT, la versión es obsoleta,
     *         el proveedor no existe en la organización o está inactivo
     */
    @Transactional
    public PurchaseOrderDetail update(
            UUID organizationId,
            UUID branchId,
            UUID purchaseOrderId,
            UpdatePurchaseOrderCommand command) {
        PurchaseOrder order = lockOrder(organizationId, branchId, purchaseOrderId);
        order.requireDraft();
        requireExpectedVersion(order, command.expectedVersion());

        List<String> changedFields = new ArrayList<>();
        if (command.supplierId() != null
                && !Objects.equals(order.getSupplier().getId(), command.supplierId())) {
            Supplier supplier = requireSupplier(organizationId, command.supplierId());
            requireActiveSupplier(supplier);
            order.changeSupplier(supplier, clock.instant());
            changedFields.add("supplierId");
        }
        if (changedFields.isEmpty()) {
            return detail(order);
        }
        PurchaseOrder saved = purchaseOrderRepository.saveAndFlush(order);
        auditPort.record(AuditRecord.ofBranch(
                organizationId,
                branchId,
                "purchase_order.updated",
                "purchase_order",
                saved.getId(),
                AuditMetadata.of("changedFields", changedFields)));
        return detail(saved);
    }

    /**
     * Agrega una partida a un borrador, capturando los snapshots del artículo.
     *
     * @param organizationId organización propietaria
     * @param branchId sucursal de destino de la ruta
     * @param purchaseOrderId identificador de la orden
     * @param command artículo, cantidad, costo opcional y versión esperada
     * @return orden con la nueva partida
     * @throws BusinessException si la orden no existe, no está en DRAFT, la versión es obsoleta,
     *         el artículo no existe en la organización, está inactivo o ya tiene una partida en la orden
     */
    @Transactional
    public PurchaseOrderDetail addLine(
            UUID organizationId,
            UUID branchId,
            UUID purchaseOrderId,
            CreatePurchaseOrderLineCommand command) {
        PurchaseOrder order = lockOrder(organizationId, branchId, purchaseOrderId);
        order.requireDraft();
        requireExpectedVersion(order, command.expectedVersion());

        InventoryItem item = inventoryItemUnitGuard
                .lockAndMarkReferenced(organizationId, List.of(command.inventoryItemId()))
                .get(command.inventoryItemId());
        requireActiveInventoryItem(item);
        if (purchaseOrderLineRepository.existsByPurchaseOrderIdAndInventoryItemId(
                purchaseOrderId, item.getId())) {
            throw duplicateLine();
        }
        int lineNumber = purchaseOrderLineRepository.findMaxLineNumber(purchaseOrderId) + 1;
        PurchaseOrderLine line = PurchaseOrderLine.create(
                organizationId, purchaseOrderId, item, lineNumber, command.quantity(), command.unitCost());
        PurchaseOrderLine savedLine = purchaseOrderLineRepository.saveAndFlush(line);

        order.registerLineMutation(clock.instant());
        PurchaseOrder saved = purchaseOrderRepository.saveAndFlush(order);
        Map<String, Object> metadata = new LinkedHashMap<>();
        metadata.put("lineNumber", savedLine.getLineNumber());
        metadata.put("inventoryItemId", savedLine.getInventoryItemId().toString());
        metadata.put("quantity", savedLine.getQuantity().toPlainString());
        auditPort.record(AuditRecord.ofBranch(
                organizationId, branchId, "purchase_order.line.added", "purchase_order", saved.getId(),
                AuditMetadata.of(metadata)));
        return detail(saved);
    }

    /**
     * Actualiza la cantidad y/o el costo unitario de una partida de un borrador.
     *
     * @param organizationId organización propietaria
     * @param branchId sucursal de destino de la ruta
     * @param purchaseOrderId identificador de la orden
     * @param purchaseOrderLineId identificador de la partida
     * @param command cantidad y costo opcionales y versión esperada
     * @return orden con la partida actualizada
     * @throws BusinessException si la orden o la partida no existen, la orden no está en DRAFT,
     *         la versión es obsoleta o la cantidad/costo son inválidos
     */
    @Transactional
    public PurchaseOrderDetail updateLine(
            UUID organizationId,
            UUID branchId,
            UUID purchaseOrderId,
            UUID purchaseOrderLineId,
            UpdatePurchaseOrderLineCommand command) {
        PurchaseOrder order = lockOrder(organizationId, branchId, purchaseOrderId);
        order.requireDraft();
        requireExpectedVersion(order, command.expectedVersion());
        PurchaseOrderLine line = requireLine(purchaseOrderId, purchaseOrderLineId);

        List<String> changedFields = new ArrayList<>();
        if (command.quantity() != null && command.quantity().compareTo(line.getQuantity()) != 0) {
            line.updateQuantity(command.quantity());
            changedFields.add("quantity");
        }
        if (command.unitCostSpecified() && !Objects.equals(command.unitCost(), line.getUnitCost())) {
            line.updateUnitCost(command.unitCost());
            changedFields.add("unitCost");
        }
        if (changedFields.isEmpty()) {
            return detail(order);
        }
        purchaseOrderLineRepository.saveAndFlush(line);
        order.registerLineMutation(clock.instant());
        PurchaseOrder saved = purchaseOrderRepository.saveAndFlush(order);
        Map<String, Object> metadata = new LinkedHashMap<>();
        metadata.put("lineNumber", line.getLineNumber());
        metadata.put("changedFields", changedFields);
        auditPort.record(AuditRecord.ofBranch(
                organizationId, branchId, "purchase_order.line.updated", "purchase_order", saved.getId(),
                AuditMetadata.of(metadata)));
        return detail(saved);
    }

    /**
     * Elimina físicamente una partida de un borrador.
     *
     * <p>Solo se permite mientras la orden está en DRAFT: nada la referencia
     * todavía. La eliminación queda registrada en auditoría.</p>
     *
     * @param organizationId organización propietaria
     * @param branchId sucursal de destino de la ruta
     * @param purchaseOrderId identificador de la orden
     * @param purchaseOrderLineId identificador de la partida
     * @param expectedVersion versión del agregado leída por el cliente
     * @return orden sin la partida
     * @throws BusinessException si la orden o la partida no existen, la orden no está en DRAFT
     *         o la versión es obsoleta
     */
    @Transactional
    public PurchaseOrderDetail removeLine(
            UUID organizationId,
            UUID branchId,
            UUID purchaseOrderId,
            UUID purchaseOrderLineId,
            long expectedVersion) {
        PurchaseOrder order = lockOrder(organizationId, branchId, purchaseOrderId);
        order.requireDraft();
        requireExpectedVersion(order, expectedVersion);
        PurchaseOrderLine line = requireLine(purchaseOrderId, purchaseOrderLineId);

        int lineNumber = line.getLineNumber();
        String inventoryItemId = line.getInventoryItemId().toString();
        purchaseOrderLineRepository.delete(line);
        purchaseOrderLineRepository.flush();

        order.registerLineMutation(clock.instant());
        PurchaseOrder saved = purchaseOrderRepository.saveAndFlush(order);
        Map<String, Object> metadata = new LinkedHashMap<>();
        metadata.put("lineNumber", lineNumber);
        metadata.put("inventoryItemId", inventoryItemId);
        auditPort.record(AuditRecord.ofBranch(
                organizationId, branchId, "purchase_order.line.removed", "purchase_order", saved.getId(),
                AuditMetadata.of(metadata)));
        return detail(saved);
    }

    /**
     * Emite una orden de compra en borrador (transición {@code DRAFT → ORDERED}).
     *
     * <p>Reutiliza el bloqueo pesimista scoped de la cabecera y la validación de
     * {@code expectedVersion}. Antes de emitir exige al menos una partida, un
     * proveedor y artículos activos, y que la unidad base actual de cada artículo
     * coincida con el snapshot capturado en la partida. Emitir no modifica
     * existencias.</p>
     *
     * <p>La validación es una transición atómica frente a los cambios
     * administrativos concurrentes: tras bloquear la orden se bloquea el proveedor
     * y luego los artículos en orden determinista por {@code id}
     * ({@code PurchaseOrder → Supplier → InventoryItem}). Como
     * {@code SupplierService.update} y {@code InventoryItemService.update} toman
     * esos mismos bloqueos de fila, o la emisión observa el estado ya desactivado
     * y falla, o emite y confirma antes de que la desactivación prosiga; nunca
     * emite sobre un estado obsoleto del Persistence Context.</p>
     *
     * @param organizationId organización propietaria
     * @param branchId sucursal de destino de la ruta
     * @param purchaseOrderId identificador de la orden
     * @param expectedVersion versión del agregado leída por el cliente
     * @param actorUserId usuario autenticado que emite la orden
     * @return orden emitida con sus partidas
     * @throws BusinessException si la orden no existe, no está en DRAFT, la versión es obsoleta,
     *         está vacía, el proveedor o un artículo están inactivos, o la unidad base cambió
     */
    @Transactional
    public PurchaseOrderDetail issue(
            UUID organizationId, UUID branchId, UUID purchaseOrderId, long expectedVersion, UUID actorUserId) {
        PurchaseOrder order = lockOrder(organizationId, branchId, purchaseOrderId);
        order.requireDraft();
        requireExpectedVersion(order, expectedVersion);

        List<PurchaseOrderLine> lines =
                purchaseOrderLineRepository.findByPurchaseOrderIdOrderByLineNumberAsc(purchaseOrderId);
        if (lines.isEmpty()) {
            throw new BusinessException(
                    HttpStatus.CONFLICT,
                    ErrorCodes.PURCHASE_ORDER_EMPTY,
                    "The purchase order cannot be issued without lines");
        }
        lockActiveSupplier(organizationId, order.getSupplier().getId());
        lockIssuableItems(organizationId, lines);

        order.issue(actorUserId, clock.instant());
        PurchaseOrder saved = purchaseOrderRepository.saveAndFlush(order);
        Map<String, Object> metadata = new LinkedHashMap<>();
        metadata.put("toStatus", saved.getStatus().name());
        metadata.put("lineCount", lines.size());
        auditPort.record(AuditRecord.ofBranch(
                organizationId, branchId, "purchase_order.issued", "purchase_order", saved.getId(),
                AuditMetadata.of(metadata)));
        return detail(saved);
    }

    /**
     * Cancela una orden (transición {@code DRAFT|ORDERED → CANCELLED}).
     *
     * <p>Reutiliza el bloqueo pesimista scoped de la cabecera y la validación de
     * {@code expectedVersion}. La cancelación es irreversible y conserva el
     * historial de emisión cuando la orden ya estaba emitida. Cancelar no modifica
     * existencias.</p>
     *
     * @param organizationId organización propietaria
     * @param branchId sucursal de destino de la ruta
     * @param purchaseOrderId identificador de la orden
     * @param expectedVersion versión del agregado leída por el cliente
     * @param actorUserId usuario autenticado que cancela la orden
     * @return orden cancelada con sus partidas
     * @throws BusinessException si la orden no existe, la versión es obsoleta o ya estaba cancelada
     */
    @Transactional
    public PurchaseOrderDetail cancel(
            UUID organizationId, UUID branchId, UUID purchaseOrderId, long expectedVersion, UUID actorUserId) {
        PurchaseOrder order = lockOrder(organizationId, branchId, purchaseOrderId);
        requireExpectedVersion(order, expectedVersion);

        String previousStatus = order.getStatus().name();
        order.cancel(actorUserId, clock.instant());
        PurchaseOrder saved = purchaseOrderRepository.saveAndFlush(order);
        Map<String, Object> metadata = new LinkedHashMap<>();
        metadata.put("fromStatus", previousStatus);
        metadata.put("toStatus", saved.getStatus().name());
        auditPort.record(AuditRecord.ofBranch(
                organizationId, branchId, "purchase_order.cancelled", "purchase_order", saved.getId(),
                AuditMetadata.of(metadata)));
        return detail(saved);
    }

    /**
     * Bloquea el proveedor de la orden y verifica su estado vigente.
     *
     * <p>El bloqueo pesimista de la fila serializa la emisión con la desactivación
     * administrativa ({@code SupplierService.update}) y el estado se lee de forma
     * escalar y nativa bajo ese bloqueo. Es necesario porque la orden ya cargó el
     * proveedor de forma {@code EAGER}: una consulta de entidad podría devolver esa
     * instancia administrada sin reflejar la desactivación ya confirmada, mientras
     * que la proyección escalar lee el estado vigente bajo el bloqueo.</p>
     *
     * @param organizationId organización propietaria
     * @param supplierId proveedor de la orden
     * @throws BusinessException si el proveedor no existe en el ámbito o ya no está activo
     */
    private void lockActiveSupplier(UUID organizationId, UUID supplierId) {
        String status = supplierRepository.lockStatusByIdAndOrganizationId(supplierId, organizationId);
        if (status == null) {
            throw supplierNotFound();
        }
        if (!SupplierStatus.ACTIVE.name().equals(status)) {
            throw supplierInactive();
        }
    }

    /**
     * Bloquea en orden determinista por {@code id} los artículos de las partidas y
     * verifica que sigan activos y que su unidad base coincida con el snapshot.
     *
     * <p>El bloqueo pesimista serializa la emisión con la desactivación y con el
     * cambio de unidad administrativos ({@code InventoryItemService.update}), de
     * modo que el estado observado es el vigente al confirmar. El orden ascendente
     * por {@code id} mantiene el orden global de bloqueos y evita interbloqueos con
     * otros escritores de dependencias.</p>
     *
     * <p>El estado y la unidad base se leen como proyección escalar nativa bajo el
     * bloqueo, no desde una consulta de entidad: la emisión puede haber cargado ya
     * el artículo en el Persistence Context y una consulta de entidad devolvería
     * esa instancia administrada sin reflejar la desactivación confirmada.</p>
     *
     * @param organizationId organización propietaria
     * @param lines partidas de la orden
     * @throws BusinessException si un artículo no existe, está inactivo o su unidad base cambió
     */
    private void lockIssuableItems(UUID organizationId, List<PurchaseOrderLine> lines) {
        List<UUID> itemIds = lines.stream()
                .map(PurchaseOrderLine::getInventoryItemId)
                .distinct()
                .sorted()
                .toList();
        Map<UUID, InventoryItemRepository.IssuableItemState> items = inventoryItemRepository
                .lockIssuableStatesByIdIn(organizationId, itemIds)
                .stream()
                .collect(Collectors.toMap(
                        InventoryItemRepository.IssuableItemState::getId, Function.identity()));
        for (PurchaseOrderLine line : lines) {
            InventoryItemRepository.IssuableItemState item = items.get(line.getInventoryItemId());
            if (item == null) {
                throw new BusinessException(
                        HttpStatus.NOT_FOUND, ErrorCodes.INVENTORY_ITEM_NOT_FOUND, "Inventory item not found");
            }
            if (!InventoryItemStatus.ACTIVE.name().equals(item.getStatus())) {
                throw new BusinessException(
                        HttpStatus.CONFLICT, ErrorCodes.INVENTORY_ITEM_INACTIVE, "The inventory item is inactive");
            }
            if (line.getUnitSnapshot() != InventoryBaseUnit.valueOf(item.getBaseUnit())) {
                throw new BusinessException(
                        HttpStatus.CONFLICT,
                        ErrorCodes.PURCHASE_ORDER_UNIT_CHANGED,
                        "The inventory item base unit changed since the line was captured");
            }
        }
    }

    private PurchaseOrderDetail detail(PurchaseOrder order) {
        return new PurchaseOrderDetail(
                order, purchaseOrderLineRepository.findByPurchaseOrderIdOrderByLineNumberAsc(order.getId()));
    }

    private static Specification<PurchaseOrder> specification(
            UUID organizationId, UUID branchId, PurchaseOrderStatus status, UUID supplierId, String q) {
        return (root, query, criteriaBuilder) -> {
            List<Predicate> predicates = new ArrayList<>();
            predicates.add(criteriaBuilder.equal(root.get("organization").get("id"), organizationId));
            predicates.add(criteriaBuilder.equal(root.get("branchId"), branchId));
            if (status != null) {
                predicates.add(criteriaBuilder.equal(root.get("status"), status));
            }
            if (supplierId != null) {
                predicates.add(criteriaBuilder.equal(root.get("supplier").get("id"), supplierId));
            }
            if (q != null && !q.isBlank()) {
                String pattern = "%" + q.trim().toLowerCase(Locale.ROOT) + "%";
                predicates.add(criteriaBuilder.like(criteriaBuilder.lower(root.get("code")), pattern));
            }
            return criteriaBuilder.and(predicates.toArray(Predicate[]::new));
        };
    }

    private PurchaseOrder lockOrder(UUID organizationId, UUID branchId, UUID purchaseOrderId) {
        requireOrganization(organizationId);
        requireBranch(organizationId, branchId);
        return purchaseOrderRepository
                .findByIdAndOrganizationIdAndBranchIdForUpdate(purchaseOrderId, organizationId, branchId)
                .orElseThrow(PurchaseOrderService::notFound);
    }

    private PurchaseOrder requireOrder(UUID organizationId, UUID branchId, UUID purchaseOrderId) {
        return purchaseOrderRepository
                .findByIdAndOrganizationIdAndBranchId(purchaseOrderId, organizationId, branchId)
                .orElseThrow(PurchaseOrderService::notFound);
    }

    private PurchaseOrderLine requireLine(UUID purchaseOrderId, UUID purchaseOrderLineId) {
        return purchaseOrderLineRepository
                .findByIdAndPurchaseOrderId(purchaseOrderLineId, purchaseOrderId)
                .orElseThrow(() -> new BusinessException(
                        HttpStatus.NOT_FOUND,
                        ErrorCodes.PURCHASE_ORDER_LINE_NOT_FOUND,
                        "Purchase order line not found"));
    }

    private Organization requireOrganization(UUID organizationId) {
        return organizationRepository.findById(organizationId).orElseThrow(() -> new BusinessException(
                HttpStatus.NOT_FOUND, ErrorCodes.ORGANIZATION_NOT_FOUND, "Organization not found"));
    }

    private Branch requireBranch(UUID organizationId, UUID branchId) {
        return branchRepository.findByIdAndOrganizationId(branchId, organizationId).orElseThrow(() -> new BusinessException(
                HttpStatus.NOT_FOUND, ErrorCodes.BRANCH_NOT_FOUND, "Branch not found"));
    }

    private static void requireActiveBranch(Branch branch) {
        if (branch.getStatus() != BranchStatus.ACTIVE) {
            throw new BusinessException(HttpStatus.CONFLICT, ErrorCodes.BRANCH_INACTIVE, "The branch is inactive");
        }
    }

    private Supplier requireSupplier(UUID organizationId, UUID supplierId) {
        return supplierRepository.findByIdAndOrganizationId(supplierId, organizationId).orElseThrow(() -> new BusinessException(
                HttpStatus.NOT_FOUND, ErrorCodes.SUPPLIER_NOT_FOUND, "Supplier not found"));
    }

    private static void requireActiveSupplier(Supplier supplier) {
        if (supplier.getStatus() != SupplierStatus.ACTIVE) {
            throw supplierInactive();
        }
    }

    private static void requireActiveInventoryItem(InventoryItem item) {
        if (item.getStatus() != InventoryItemStatus.ACTIVE) {
            throw new BusinessException(
                    HttpStatus.CONFLICT, ErrorCodes.INVENTORY_ITEM_INACTIVE, "The inventory item is inactive");
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

    private static BusinessException notFound() {
        return new BusinessException(
                HttpStatus.NOT_FOUND, ErrorCodes.PURCHASE_ORDER_NOT_FOUND, "Purchase order not found");
    }

    private static BusinessException supplierNotFound() {
        return new BusinessException(HttpStatus.NOT_FOUND, ErrorCodes.SUPPLIER_NOT_FOUND, "Supplier not found");
    }

    private static BusinessException supplierInactive() {
        return new BusinessException(HttpStatus.CONFLICT, ErrorCodes.SUPPLIER_INACTIVE, "The supplier is inactive");
    }

    private static BusinessException duplicateCode() {
        return new BusinessException(
                HttpStatus.CONFLICT,
                ErrorCodes.PURCHASE_ORDER_CODE_ALREADY_EXISTS,
                "A purchase order with this code already exists in the organization");
    }

    private static BusinessException duplicateLine() {
        return new BusinessException(
                HttpStatus.CONFLICT,
                ErrorCodes.PURCHASE_ORDER_LINE_DUPLICATE_ITEM,
                "The purchase order already has a line for this inventory item");
    }
}
