package com.hiqnet.pos.purchasing.domain;

import java.math.BigDecimal;
import java.util.Collection;
import java.util.List;
import java.util.UUID;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.data.jpa.repository.Query;
import org.springframework.data.repository.query.Param;

/**
 * Acceso persistente a las líneas de recepción.
 */
public interface PurchaseReceiptLineRepository extends JpaRepository<PurchaseReceiptLine, UUID> {

    /**
     * Lista las líneas de una recepción en orden determinista por partida.
     *
     * @param purchaseReceiptId cabecera de la recepción
     * @return líneas ordenadas por identificador de partida
     */
    List<PurchaseReceiptLine> findByPurchaseReceiptIdOrderByPurchaseOrderLineIdAsc(UUID purchaseReceiptId);

    /**
     * Lista las líneas de varias recepciones, útil para armar representaciones
     * agregadas sin provocar N+1.
     *
     * @param purchaseReceiptIds cabeceras de recepción
     * @return líneas de todas las recepciones indicadas
     */
    List<PurchaseReceiptLine> findByPurchaseReceiptIdInOrderByPurchaseOrderLineIdAsc(
            Collection<UUID> purchaseReceiptIds);

    /**
     * Suma la cantidad ya recibida por partida de una orden. Es la fuente de
     * verdad para calcular cantidades pendientes y rechazar sobrerrecepción.
     *
     * @param organizationId organización propietaria
     * @param purchaseOrderId orden consultada
     * @return cantidades recibidas por partida
     */
    @Query("select line.purchaseOrderLineId as lineId, sum(line.quantityReceived) as received "
            + "from PurchaseReceiptLine line "
            + "where line.organizationId = :organizationId and line.purchaseOrderId = :purchaseOrderId "
            + "group by line.purchaseOrderLineId")
    List<LineReceived> sumReceivedByOrder(
            @Param("organizationId") UUID organizationId, @Param("purchaseOrderId") UUID purchaseOrderId);

    /** Cantidad ya recibida de una partida. */
    interface LineReceived {

        UUID getLineId();

        BigDecimal getReceived();
    }
}
