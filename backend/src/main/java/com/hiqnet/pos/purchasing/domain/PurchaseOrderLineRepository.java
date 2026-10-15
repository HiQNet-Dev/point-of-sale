package com.hiqnet.pos.purchasing.domain;

import java.util.List;
import java.util.Optional;
import java.util.UUID;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.data.jpa.repository.Query;
import org.springframework.data.repository.query.Param;

/**
 * Acceso persistente a las partidas de una orden de compra.
 *
 * <p>No se expone eliminación por identificador sin orden: toda resolución exige
 * el identificador de la orden, de modo que una partida nunca se alcanza fuera
 * de su agregado.</p>
 */
public interface PurchaseOrderLineRepository extends JpaRepository<PurchaseOrderLine, UUID> {

    /**
     * Lista las partidas de una orden en el orden de línea estable asignado por el servidor.
     *
     * @param purchaseOrderId identificador de la orden
     * @return partidas ordenadas por número de línea
     */
    List<PurchaseOrderLine> findByPurchaseOrderIdOrderByLineNumberAsc(UUID purchaseOrderId);

    /**
     * Resuelve una partida dentro de su orden.
     *
     * @param id identificador de la partida
     * @param purchaseOrderId orden autorizada
     * @return partida dentro del agregado solicitado, si existe
     */
    Optional<PurchaseOrderLine> findByIdAndPurchaseOrderId(UUID id, UUID purchaseOrderId);

    /**
     * Indica si la orden ya contiene una partida del mismo artículo.
     *
     * @param purchaseOrderId identificador de la orden
     * @param inventoryItemId identificador del artículo de inventario
     * @return {@code true} cuando la partida duplicada ya existe
     */
    boolean existsByPurchaseOrderIdAndInventoryItemId(UUID purchaseOrderId, UUID inventoryItemId);

    /**
     * Obtiene el mayor número de línea de la orden, o {@code 0} cuando está vacía.
     *
     * <p>El servidor asigna {@code max + 1} como siguiente número de línea. La
     * unicidad real la garantiza el constraint {@code (purchase_order_id,
     * line_number)}: el cálculo se ejecuta con el bloqueo de cabecera tomado, por
     * lo que dos altas concurrentes no pueden colisionar.</p>
     *
     * @param purchaseOrderId identificador de la orden
     * @return mayor número de línea existente, o cero
     */
    @Query("select coalesce(max(line.lineNumber), 0) from PurchaseOrderLine line "
            + "where line.purchaseOrderId = :purchaseOrderId")
    int findMaxLineNumber(@Param("purchaseOrderId") UUID purchaseOrderId);
}
