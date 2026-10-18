package com.hiqnet.pos.purchasing.application;

import com.hiqnet.pos.purchasing.domain.PurchaseOrder;
import com.hiqnet.pos.purchasing.domain.PurchaseOrderLine;
import java.math.BigDecimal;
import java.util.List;

/**
 * Estado de recepción de una orden: cantidades solicitadas, recibidas y pendientes.
 *
 * @param order orden consultada
 * @param lines estado por partida en el orden de línea del servidor
 */
public record PurchaseOrderReceiptStatus(PurchaseOrder order, List<LineStatus> lines) {

    /**
     * Estado de recepción de una partida.
     *
     * @param line partida original
     * @param quantityReceived cantidad ya recibida
     * @param quantityPending cantidad pendiente por recibir
     */
    public record LineStatus(PurchaseOrderLine line, BigDecimal quantityReceived, BigDecimal quantityPending) {
    }
}
