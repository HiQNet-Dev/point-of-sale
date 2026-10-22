package com.hiqnet.pos.purchasing.application;

import java.util.UUID;

/**
 * Datos de una actualización parcial de la cabecera de una orden.
 *
 * <p>{@code supplierId} {@code null} significa que el proveedor no se modifica.
 * El estado y la moneda no viajan aquí: el estado solo cambia por transiciones
 * de dominio y la moneda la determina el servidor.</p>
 *
 * @param expectedVersion versión leída por el cliente
 * @param supplierId nuevo proveedor opcional, de la misma organización
 */
public record UpdatePurchaseOrderCommand(long expectedVersion, UUID supplierId) {
}
