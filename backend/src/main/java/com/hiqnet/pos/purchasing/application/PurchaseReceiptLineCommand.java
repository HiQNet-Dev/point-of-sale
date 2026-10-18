package com.hiqnet.pos.purchasing.application;

import java.math.BigDecimal;
import java.util.UUID;

/**
 * Línea solicitada en una recepción.
 *
 * @param purchaseOrderLineId partida de la orden que se recibe
 * @param quantity cantidad recibida, decimal exacto positivo en la unidad base de la partida
 */
public record PurchaseReceiptLineCommand(UUID purchaseOrderLineId, BigDecimal quantity) {
}
