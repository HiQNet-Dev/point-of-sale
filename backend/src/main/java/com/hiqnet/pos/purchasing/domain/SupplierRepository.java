package com.hiqnet.pos.purchasing.domain;

import jakarta.persistence.LockModeType;
import java.util.Optional;
import java.util.UUID;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.data.jpa.repository.JpaSpecificationExecutor;
import org.springframework.data.jpa.repository.Lock;
import org.springframework.data.jpa.repository.Query;
import org.springframework.data.repository.query.Param;

/**
 * Acceso persistente a proveedores, siempre acotado por organización.
 */
public interface SupplierRepository extends JpaRepository<Supplier, UUID>, JpaSpecificationExecutor<Supplier> {

    /**
     * Busca un proveedor sin permitir resolver un identificador fuera de su organización.
     *
     * @param id identificador del proveedor
     * @param organizationId identificador de la organización autorizada
     * @return proveedor dentro del ámbito solicitado, si existe
     */
    Optional<Supplier> findByIdAndOrganizationId(UUID id, UUID organizationId);

    /**
     * Bloquea la fila del proveedor scoped para serializar una mutación administrativa.
     *
     * @param id identificador del proveedor
     * @param organizationId identificador de la organización autorizada
     * @return proveedor bloqueado dentro del ámbito solicitado, si existe
     */
    @Lock(LockModeType.PESSIMISTIC_WRITE)
    @Query("select supplier from Supplier supplier where supplier.id = :id and supplier.organization.id = :organizationId")
    Optional<Supplier> findByIdAndOrganizationIdForUpdate(
            @Param("id") UUID id, @Param("organizationId") UUID organizationId);

    /**
     * Indica si el código comercial ya está ocupado dentro de la organización.
     *
     * @param organizationId identificador de la organización
     * @param code código comercial normalizado
     * @return {@code true} cuando existe un proveedor con el código
     */
    boolean existsByOrganizationIdAndCode(UUID organizationId, String code);
}
