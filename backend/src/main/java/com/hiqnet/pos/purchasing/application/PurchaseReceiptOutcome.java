package com.hiqnet.pos.purchasing.application;

/**
 * Resultado de registrar una recepción.
 *
 * @param detail recepción confirmada con su detalle
 * @param replayed {@code true} cuando el resultado proviene de un reintento
 *        idempotente con la misma clave y el mismo contenido (no hubo efectos
 *        nuevos); {@code false} cuando la recepción se creó en esta llamada
 */
public record PurchaseReceiptOutcome(PurchaseReceiptDetail detail, boolean replayed) {
}
