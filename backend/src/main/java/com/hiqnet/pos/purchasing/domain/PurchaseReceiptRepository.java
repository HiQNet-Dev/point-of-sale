package com.hiqnet.pos.purchasing.domain;

import java.util.List;
import java.util.Optional;
import java.util.UUID;
import org.springframework.data.jpa.repository.JpaRepository;

/**
 * Acceso persistente a recepciones, siempre acotado por organización y sucursal.
 */
public interface PurchaseReceiptRepository extends JpaRepository<PurchaseReceipt, UUID> {

    /**
     * Busca la recepción asociada a una clave de idempotencia dentro de la pareja
     * organización-orden; es el eje del protocolo idempotente.
     *
     * @param organizationId organización propietaria
     * @param purchaseOrderId orden recibida
     * @param idempotencyKey clave normalizada
     * @return recepción confirmada con esa clave, si existe
     */
    Optional<PurchaseReceipt> findByOrganizationIdAndPurchaseOrderIdAndIdempotencyKey(
            UUID organizationId, UUID purchaseOrderId, String idempotencyKey);

    /**
     * Resuelve una recepción dentro de su organización y sucursal.
     *
     * @param id identificador de la recepción
     * @param organizationId organización autorizada
     * @param branchId sucursal de destino de la ruta
     * @return recepción dentro del ámbito solicitado, si existe
     */
    Optional<PurchaseReceipt> findByIdAndOrganizationIdAndBranchId(
            UUID id, UUID organizationId, UUID branchId);

    /**
     * Lista las recepciones de una orden en orden cronológico estable.
     *
     * @param organizationId organización propietaria
     * @param purchaseOrderId orden recibida
     * @return recepciones ordenadas por fecha e identificador
     */
    List<PurchaseReceipt> findByOrganizationIdAndPurchaseOrderIdOrderByReceivedAtAscIdAsc(
            UUID organizationId, UUID purchaseOrderId);

    /**
     * Cuenta las recepciones confirmadas de una orden; se usa para prohibir la
     * cancelación de una orden que ya recibió mercancía.
     *
     * @param organizationId organización propietaria
     * @param purchaseOrderId orden consultada
     * @return número de recepciones confirmadas
     */
    long countByOrganizationIdAndPurchaseOrderId(UUID organizationId, UUID purchaseOrderId);
}
