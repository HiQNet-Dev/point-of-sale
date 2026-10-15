package com.hiqnet.pos.purchasing.application;

import com.hiqnet.pos.organization.domain.Organization;
import com.hiqnet.pos.organization.domain.OrganizationRepository;
import com.hiqnet.pos.purchasing.domain.Supplier;
import com.hiqnet.pos.purchasing.domain.SupplierRepository;
import com.hiqnet.pos.purchasing.domain.SupplierStatus;
import com.hiqnet.pos.shared.audit.AuditMetadata;
import com.hiqnet.pos.shared.audit.AuditPort;
import com.hiqnet.pos.shared.audit.AuditRecord;
import com.hiqnet.pos.shared.error.BusinessException;
import com.hiqnet.pos.shared.error.ErrorCodes;
import jakarta.persistence.criteria.Predicate;
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

/**
 * Casos de uso para la administración organization-scoped de proveedores.
 *
 * <p>La unicidad de código se valida anticipadamente para dar un error de
 * dominio útil, pero PostgreSQL conserva la autoridad final ante carreras de
 * creación concurrente. Toda mutación y su evento de auditoría forman una sola
 * transacción.</p>
 */
@Service
public class SupplierService {

    private final SupplierRepository supplierRepository;
    private final OrganizationRepository organizationRepository;
    private final AuditPort auditPort;

    public SupplierService(
            SupplierRepository supplierRepository,
            OrganizationRepository organizationRepository,
            AuditPort auditPort) {
        this.supplierRepository = supplierRepository;
        this.organizationRepository = organizationRepository;
        this.auditPort = auditPort;
    }

    /**
     * Registra un proveedor activo dentro de una organización.
     *
     * @param organizationId identificador de la organización propietaria
     * @param code código comercial del proveedor
     * @param name nombre comercial obligatorio
     * @param legalName razón social opcional
     * @param taxId identificador fiscal opcional
     * @param contactName contacto principal opcional
     * @param email correo de contacto opcional
     * @param phone teléfono de contacto opcional
     * @param address dirección opcional
     * @return proveedor persistido
     * @throws BusinessException si la organización no existe o el código ya está ocupado
     */
    @Transactional
    public Supplier create(
            UUID organizationId,
            String code,
            String name,
            String legalName,
            String taxId,
            String contactName,
            String email,
            String phone,
            String address) {
        Organization organization = requireOrganization(organizationId);
        String normalizedCode = code == null ? null : code.trim().toLowerCase(Locale.ROOT);
        if (normalizedCode != null && supplierRepository.existsByOrganizationIdAndCode(organizationId, normalizedCode)) {
            throw duplicateCode();
        }
        Supplier saved = supplierRepository.saveAndFlush(Supplier.create(
                organization, normalizedCode, name, legalName, taxId, contactName, email, phone, address));
        auditPort.record(AuditRecord.of(
                organizationId,
                "supplier.created",
                "supplier",
                saved.getId(),
                AuditMetadata.of("code", saved.getCode())));
        return saved;
    }

    /**
     * Obtiene un proveedor sin permitir resolverlo fuera de su organización.
     *
     * @param organizationId identificador de la organización solicitada
     * @param supplierId identificador del proveedor
     * @return proveedor encontrado
     * @throws BusinessException si la organización o el proveedor no existen en el ámbito
     */
    @Transactional(readOnly = true)
    public Supplier get(UUID organizationId, UUID supplierId) {
        requireOrganization(organizationId);
        return supplierRepository.findByIdAndOrganizationId(supplierId, organizationId).orElseThrow(SupplierService::notFound);
    }

    /**
     * Lista proveedores con filtros de estado y búsqueda textual dentro de una organización.
     *
     * @param organizationId identificador de la organización solicitada
     * @param status estado operativo opcional
     * @param q texto opcional que busca en código, nombre, razón social, identificador fiscal o contacto
     * @param pageable página y orden fijados por el controlador
     * @return página de proveedores scoped
     * @throws BusinessException si la organización no existe
     */
    @Transactional(readOnly = true)
    public Page<Supplier> list(UUID organizationId, SupplierStatus status, String q, Pageable pageable) {
        requireOrganization(organizationId);
        return supplierRepository.findAll(specification(organizationId, status, q), pageable);
    }

    /**
     * Actualiza los campos editables de un proveedor.
     *
     * <p>La fila se bloquea antes de comparar {@code expectedVersion}. El lock
     * serializa la edición administrativa y {@code @Version} mantiene la barrera
     * final de persistencia. Una actualización sin cambios es idempotente: no
     * incrementa la versión ni escribe auditoría.</p>
     *
     * @param organizationId identificador de la organización propietaria
     * @param supplierId identificador del proveedor
     * @param command campos editables y versión esperada
     * @return proveedor actualizado o sin cambios
     * @throws BusinessException si el recurso no existe o la versión es obsoleta
     */
    @Transactional
    public Supplier update(UUID organizationId, UUID supplierId, UpdateSupplierCommand command) {
        requireOrganization(organizationId);
        Supplier supplier = supplierRepository
                .findByIdAndOrganizationIdForUpdate(supplierId, organizationId)
                .orElseThrow(SupplierService::notFound);
        if (supplier.getVersion() != command.expectedVersion()) {
            throw concurrentModification();
        }

        List<String> changedFields = new ArrayList<>();
        updateName(supplier, command.name(), changedFields);
        updateLegalName(supplier, command.legalName(), changedFields);
        updateTaxId(supplier, command.taxId(), changedFields);
        updateContactName(supplier, command.contactName(), changedFields);
        updateEmail(supplier, command.email(), changedFields);
        updatePhone(supplier, command.phone(), changedFields);
        updateAddress(supplier, command.address(), changedFields);
        updateStatus(supplier, command.status(), changedFields);

        if (changedFields.isEmpty()) {
            return supplier;
        }
        Supplier saved = supplierRepository.saveAndFlush(supplier);
        Map<String, Object> metadata = new LinkedHashMap<>();
        metadata.put("changedFields", changedFields);
        auditPort.record(AuditRecord.of(
                organizationId, "supplier.updated", "supplier", saved.getId(), AuditMetadata.of(metadata)));
        return saved;
    }

    private static void updateName(Supplier supplier, String value, List<String> changedFields) {
        if (value == null) {
            return;
        }
        String before = supplier.getName();
        supplier.updateName(value);
        if (!supplier.getName().equals(before)) {
            changedFields.add("name");
        }
    }

    private static void updateLegalName(Supplier supplier, String value, List<String> changedFields) {
        if (value == null) {
            return;
        }
        String before = supplier.getLegalName();
        supplier.updateLegalName(value);
        if (!Objects.equals(supplier.getLegalName(), before)) {
            changedFields.add("legalName");
        }
    }

    private static void updateTaxId(Supplier supplier, String value, List<String> changedFields) {
        if (value == null) {
            return;
        }
        String before = supplier.getTaxId();
        supplier.updateTaxId(value);
        if (!Objects.equals(supplier.getTaxId(), before)) {
            changedFields.add("taxId");
        }
    }

    private static void updateContactName(Supplier supplier, String value, List<String> changedFields) {
        if (value == null) {
            return;
        }
        String before = supplier.getContactName();
        supplier.updateContactName(value);
        if (!Objects.equals(supplier.getContactName(), before)) {
            changedFields.add("contactName");
        }
    }

    private static void updateEmail(Supplier supplier, String value, List<String> changedFields) {
        if (value == null) {
            return;
        }
        String before = supplier.getEmail();
        supplier.updateEmail(value);
        if (!Objects.equals(supplier.getEmail(), before)) {
            changedFields.add("email");
        }
    }

    private static void updatePhone(Supplier supplier, String value, List<String> changedFields) {
        if (value == null) {
            return;
        }
        String before = supplier.getPhone();
        supplier.updatePhone(value);
        if (!Objects.equals(supplier.getPhone(), before)) {
            changedFields.add("phone");
        }
    }

    private static void updateAddress(Supplier supplier, String value, List<String> changedFields) {
        if (value == null) {
            return;
        }
        String before = supplier.getAddress();
        supplier.updateAddress(value);
        if (!Objects.equals(supplier.getAddress(), before)) {
            changedFields.add("address");
        }
    }

    private static void updateStatus(Supplier supplier, SupplierStatus value, List<String> changedFields) {
        if (value == null) {
            return;
        }
        SupplierStatus before = supplier.getStatus();
        supplier.updateStatus(value);
        if (supplier.getStatus() != before) {
            changedFields.add("status");
        }
    }

    private static Specification<Supplier> specification(UUID organizationId, SupplierStatus status, String q) {
        return (root, query, criteriaBuilder) -> {
            List<Predicate> predicates = new ArrayList<>();
            predicates.add(criteriaBuilder.equal(root.get("organization").get("id"), organizationId));
            if (status != null) {
                predicates.add(criteriaBuilder.equal(root.get("status"), status));
            }
            if (q != null && !q.isBlank()) {
                String pattern = "%" + q.trim().toLowerCase(Locale.ROOT) + "%";
                predicates.add(criteriaBuilder.or(
                        criteriaBuilder.like(criteriaBuilder.lower(root.get("code")), pattern),
                        criteriaBuilder.like(criteriaBuilder.lower(root.get("name")), pattern),
                        criteriaBuilder.like(criteriaBuilder.lower(root.get("legalName")), pattern),
                        criteriaBuilder.like(criteriaBuilder.lower(root.get("taxId")), pattern),
                        criteriaBuilder.like(criteriaBuilder.lower(root.get("contactName")), pattern)));
            }
            return criteriaBuilder.and(predicates.toArray(Predicate[]::new));
        };
    }

    private Organization requireOrganization(UUID organizationId) {
        return organizationRepository.findById(organizationId).orElseThrow(() -> new BusinessException(
                HttpStatus.NOT_FOUND, ErrorCodes.ORGANIZATION_NOT_FOUND, "Organization not found"));
    }

    private static BusinessException notFound() {
        return new BusinessException(HttpStatus.NOT_FOUND, ErrorCodes.SUPPLIER_NOT_FOUND, "Supplier not found");
    }

    private static BusinessException duplicateCode() {
        return new BusinessException(
                HttpStatus.CONFLICT,
                ErrorCodes.SUPPLIER_CODE_ALREADY_EXISTS,
                "A supplier with this code already exists in the organization");
    }

    private static BusinessException concurrentModification() {
        return new BusinessException(
                HttpStatus.CONFLICT,
                ErrorCodes.CONCURRENT_MODIFICATION,
                "The resource was modified concurrently; retry with fresh data");
    }
}
