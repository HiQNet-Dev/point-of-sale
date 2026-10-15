package com.hiqnet.pos.purchasing.domain;

/**
 * Estado operativo de una orden de compra.
 *
 * <p>El ciclo completo es {@code DRAFT → ORDERED → RECEIVED} (con
 * {@code PARTIALLY_RECEIVED} y {@code CANCELLED} como estados intermedios), pero
 * esta fundación solo persiste {@link #DRAFT}: las transiciones {@code issue} y
 * {@code cancel} y los estados derivados de la recepción de mercancía se
 * habilitan en incrementos posteriores. El estado nunca se acepta desde el
 * cliente.</p>
 */
public enum PurchaseOrderStatus {
    DRAFT,
    ORDERED,
    CANCELLED
}
