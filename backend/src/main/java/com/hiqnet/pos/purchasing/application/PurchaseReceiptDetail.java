package com.hiqnet.pos.purchasing.application;

import com.hiqnet.pos.purchasing.domain.PurchaseOrder;
import com.hiqnet.pos.purchasing.domain.PurchaseReceipt;
import com.hiqnet.pos.purchasing.domain.PurchaseReceiptLine;
import java.util.List;

/**
 * Vista de aplicación de una recepción con sus líneas y la orden afectada.
 *
 * @param receipt cabecera de la recepción
 * @param lines líneas de la recepción ordenadas por partida
 * @param purchaseOrder orden tras aplicar la recepción, con su estado y versión vigentes
 */
public record PurchaseReceiptDetail(
        PurchaseReceipt receipt, List<PurchaseReceiptLine> lines, PurchaseOrder purchaseOrder) {
}
