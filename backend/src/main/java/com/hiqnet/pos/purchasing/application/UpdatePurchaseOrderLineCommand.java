package com.hiqnet.pos.purchasing.application;

import java.math.BigDecimal;

/**
 * Datos de una actualización parcial de una partida.
 *
 * <p>{@code quantity} {@code null} significa que la cantidad no se modifica. El
 * costo unitario usa la semántica tri-estado de {@code PatchField}: clave ausente
 * deja el valor intacto, un {@code null} explícito limpia el costo a {@code NULL}
 * y un valor lo asigna (incluido {@code 0}). El artículo referenciado y sus
 * snapshots son inmutables: para cambiar de artículo se elimina la partida y se
 * agrega otra.</p>
 *
 * @param expectedVersion versión del agregado leída por el cliente
 * @param quantity nueva cantidad opcional, decimal exacto positivo; {@code null} no cambia
 * @param unitCost nuevo costo unitario opcional, no negativo; {@code null} limpia el costo
 * @param unitCostSpecified {@code true} cuando el cuerpo incluyó {@code unitCost} (aunque fuera
 *        {@code null}), de modo que {@code null} limpia en vez de omitir
 */
public record UpdatePurchaseOrderLineCommand(
        long expectedVersion, BigDecimal quantity, BigDecimal unitCost, boolean unitCostSpecified) {
}
