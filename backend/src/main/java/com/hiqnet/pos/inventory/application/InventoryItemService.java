package com.hiqnet.pos.inventory.application;

import com.hiqnet.pos.inventory.domain.InventoryBaseUnit;
import com.hiqnet.pos.inventory.domain.InventoryItem;
import com.hiqnet.pos.inventory.domain.InventoryItemRepository;
import com.hiqnet.pos.inventory.domain.InventoryItemStatus;
import com.hiqnet.pos.organization.domain.Organization;
import com.hiqnet.pos.organization.domain.OrganizationRepository;
import com.hiqnet.pos.shared.audit.AuditMetadata;
import com.hiqnet.pos.shared.audit.AuditPort;
import com.hiqnet.pos.shared.audit.AuditRecord;
import com.hiqnet.pos.shared.error.BusinessException;
import com.hiqnet.pos.shared.error.ErrorCodes;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Objects;
import java.util.UUID;
import org.springframework.data.domain.Page;
import org.springframework.data.domain.Pageable;
import org.springframework.data.jpa.domain.Specification;
import org.springframework.http.HttpStatus;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

@Service
public class InventoryItemService {

    private final InventoryItemRepository inventoryItemRepository;
    private final OrganizationRepository organizationRepository;
    private final AuditPort auditPort;

    public InventoryItemService(
            InventoryItemRepository inventoryItemRepository,
            OrganizationRepository organizationRepository,
            AuditPort auditPort) {
        this.inventoryItemRepository = inventoryItemRepository;
        this.organizationRepository = organizationRepository;
        this.auditPort = auditPort;
    }

    @Transactional
    public InventoryItem create(
            UUID organizationId,
            String code,
            String name,
            String description,
            InventoryBaseUnit baseUnit,
            Boolean fractionalUnitAllowed) {
        Organization organization = requireOrganization(organizationId);
        String normalizedCode = code == null ? null : code.trim().toLowerCase(Locale.ROOT);
        if (normalizedCode != null
                && inventoryItemRepository.existsByOrganizationIdAndCode(organizationId, normalizedCode)) {
            throw duplicateCode();
        }
        InventoryItem item = InventoryItem.create(
                organization,
                normalizedCode,
                name,
                description,
                baseUnit,
                Boolean.TRUE.equals(fractionalUnitAllowed));
        InventoryItem saved = inventoryItemRepository.saveAndFlush(item);
        auditPort.record(AuditRecord.of(
                organizationId,
                "inventory.item.created",
                "inventory_item",
                saved.getId(),
                AuditMetadata.of("code", saved.getCode())));
        return saved;
    }

    @Transactional(readOnly = true)
    public InventoryItem get(UUID organizationId, UUID inventoryItemId) {
        requireOrganization(organizationId);
        return inventoryItemRepository.findByIdAndOrganizationId(inventoryItemId, organizationId)
                .orElseThrow(() -> itemNotFound());
    }

    @Transactional(readOnly = true)
    public Page<InventoryItem> list(
            UUID organizationId,
            String q,
            InventoryItemStatus status,
            InventoryBaseUnit baseUnit,
            Pageable pageable) {
        requireOrganization(organizationId);
        return inventoryItemRepository.findAll(specification(organizationId, q, status, baseUnit), pageable);
    }

    @Transactional
    public InventoryItem update(UUID organizationId, UUID inventoryItemId, UpdateInventoryItemCommand command) {
        requireOrganization(organizationId);
        InventoryItem item = inventoryItemRepository.findByIdAndOrganizationIdForUpdate(inventoryItemId, organizationId)
                .orElseThrow(() -> itemNotFound());
        if (item.getVersion() != command.expectedVersion()) {
            throw new BusinessException(
                    HttpStatus.CONFLICT,
                    ErrorCodes.CONCURRENT_MODIFICATION,
                    "The resource was modified concurrently; retry with fresh data");
        }

        // El marcador se lee antes de mutar la entidad y con una proyección escalar
        // (que no usa el Persistence Context, por si la unidad ya se marcó en esta
        // misma transacción). Leerlo aquí, con la entidad aún limpia, evita el
        // auto-flush intermedio que provocaría un incremento doble de @Version.
        boolean unitLocked = command.baseUnit() != null
                && command.baseUnit() != item.getBaseUnit()
                && Boolean.TRUE.equals(
                        inventoryItemRepository.findUnitLockedFlag(inventoryItemId, organizationId));

        List<String> changedFields = new ArrayList<>();
        if (command.name() != null) {
            String before = item.getName();
            item.updateName(command.name());
            if (!item.getName().equals(before)) {
                changedFields.add("name");
            }
        }
        if (command.description() != null) {
            String before = item.getDescription();
            item.updateDescription(command.description());
            if (!Objects.equals(item.getDescription(), before)) {
                changedFields.add("description");
            }
        }
        if (command.baseUnit() != null && command.baseUnit() != item.getBaseUnit()) {
            if (unitLocked) {
                throw new BusinessException(
                        HttpStatus.CONFLICT,
                        ErrorCodes.INVENTORY_ITEM_UNIT_LOCKED,
                        "The base unit cannot be changed because the item has dependent inventory facts");
            }
            item.updateBaseUnit(command.baseUnit());
            changedFields.add("baseUnit");
        }
        if (command.fractionalUnitAllowed() != null
                && command.fractionalUnitAllowed() != item.isFractionalUnitAllowed()) {
            item.updateFractionalUnitAllowed(command.fractionalUnitAllowed());
            changedFields.add("fractionalUnitAllowed");
        }
        if (command.status() != null && command.status() != item.getStatus()) {
            item.updateStatus(command.status());
            changedFields.add("status");
        }
        if (changedFields.isEmpty()) {
            return item;
        }

        InventoryItem saved = inventoryItemRepository.saveAndFlush(item);
        Map<String, Object> metadata = new LinkedHashMap<>();
        metadata.put("changedFields", changedFields);
        auditPort.record(AuditRecord.of(
                organizationId,
                "inventory.item.updated",
                "inventory_item",
                saved.getId(),
                AuditMetadata.of(metadata)));
        return saved;
    }

    private Organization requireOrganization(UUID organizationId) {
        return organizationRepository.findById(organizationId)
                .orElseThrow(() -> new BusinessException(
                        HttpStatus.NOT_FOUND, ErrorCodes.ORGANIZATION_NOT_FOUND, "Organization not found"));
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

    private static BusinessException itemNotFound() {
        return new BusinessException(
                HttpStatus.NOT_FOUND, ErrorCodes.INVENTORY_ITEM_NOT_FOUND, "Inventory item not found");
    }

    private static BusinessException duplicateCode() {
        return new BusinessException(
                HttpStatus.CONFLICT,
                ErrorCodes.INVENTORY_ITEM_CODE_ALREADY_EXISTS,
                "An inventory item with this code already exists in the organization");
    }
}
