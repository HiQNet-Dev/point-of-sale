package com.hiqnet.pos.purchasing.application;

import java.math.BigDecimal;
import java.util.UUID;

/**
 * Datos para agregar una partida a un borrador.
 *
 * @param expectedVersion versión del agregado leída por el cliente
 * @param inventoryItemId artículo de inventario de la misma organización
 * @param quantity cantidad solicitada, decimal exacto positivo
 * @param unitCost costo unitario pactado opcional; {@code null} cuando no se informa
 */
public record CreatePurchaseOrderLineCommand(
        long expectedVersion, UUID inventoryItemId, BigDecimal quantity, BigDecimal unitCost) {
}
