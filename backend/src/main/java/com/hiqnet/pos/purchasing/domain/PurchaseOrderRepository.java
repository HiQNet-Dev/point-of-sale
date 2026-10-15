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
 * Acceso persistente a órdenes de compra, siempre acotado por organización y
 * sucursal.
 */
public interface PurchaseOrderRepository
        extends JpaRepository<PurchaseOrder, UUID>, JpaSpecificationExecutor<PurchaseOrder> {

    /**
     * Resuelve una orden dentro de su organización y sucursal, impidiendo que un
     * identificador ajeno se lea o se modifique (aislamiento multiempresa).
     *
     * @param id identificador de la orden
     * @param organizationId organización autorizada
     * @param branchId sucursal de destino de la ruta
     * @return orden dentro del ámbito solicitado, si existe
     */
    Optional<PurchaseOrder> findByIdAndOrganizationIdAndBranchId(UUID id, UUID organizationId, UUID branchId);

    /**
     * Bloquea la cabecera del agregado para serializar todas sus mutaciones.
     *
     * <p>El bloqueo pesimista sobre la fila de cabecera es el punto de
     * sincronización de cabecera y partidas: cualquier operación que modifique el
     * agregado toma primero este bloqueo, de modo que dos escrituras concurrentes
     * sobre la misma orden se serializan y la segunda observa la versión ya
     * incrementada por la primera.</p>
     *
     * @param id identificador de la orden
     * @param organizationId organización autorizada
     * @param branchId sucursal de destino de la ruta
     * @return orden bloqueada dentro del ámbito solicitado, si existe
     */
    @Lock(LockModeType.PESSIMISTIC_WRITE)
    @Query("select purchaseOrder from PurchaseOrder purchaseOrder "
            + "where purchaseOrder.id = :id and purchaseOrder.organization.id = :organizationId "
            + "and purchaseOrder.branchId = :branchId")
    Optional<PurchaseOrder> findByIdAndOrganizationIdAndBranchIdForUpdate(
            @Param("id") UUID id,
            @Param("organizationId") UUID organizationId,
            @Param("branchId") UUID branchId);

    /**
     * Indica si el código comercial ya está ocupado dentro de la organización.
     *
     * @param organizationId organización de la orden
     * @param code código comercial normalizado
     * @return {@code true} cuando existe una orden con ese código
     */
    boolean existsByOrganizationIdAndCode(UUID organizationId, String code);
}
