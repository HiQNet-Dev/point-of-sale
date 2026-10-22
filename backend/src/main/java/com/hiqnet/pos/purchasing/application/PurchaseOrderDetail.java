package com.hiqnet.pos.purchasing.application;

import com.hiqnet.pos.purchasing.domain.PurchaseOrder;
import com.hiqnet.pos.purchasing.domain.PurchaseOrderLine;
import java.util.List;

/**
 * Vista de aplicación de una orden con sus partidas.
 *
 * @param order cabecera del agregado
 * @param lines partidas ordenadas por número de línea
 */
public record PurchaseOrderDetail(PurchaseOrder order, List<PurchaseOrderLine> lines) {
}
