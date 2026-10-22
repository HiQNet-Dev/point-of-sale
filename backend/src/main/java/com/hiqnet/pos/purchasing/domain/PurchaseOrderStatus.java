package com.hiqnet.pos.purchasing.domain;

/**
 * Estado operativo de una orden de compra.
 *
 * <p>El ciclo completo es {@code DRAFT → ORDERED → PARTIALLY_RECEIVED →
 * RECEIVED} (con {@code CANCELLED} alcanzable desde {@code DRAFT} u
 * {@code ORDERED}). Una orden se emite ({@code issue}, solo desde {@code DRAFT}),
 * recibe mercancía por recepciones ({@code ORDERED → PARTIALLY_RECEIVED →
 * RECEIVED}) y se cancela ({@code cancel}, desde {@code DRAFT} u {@code ORDERED}).
 * El estado nunca se acepta desde el cliente ni se modifica por {@code PATCH};
 * solo lo gobiernan las transiciones del agregado.</p>
 */
public enum PurchaseOrderStatus {
    DRAFT,
    ORDERED,
    PARTIALLY_RECEIVED,
    RECEIVED,
    CANCELLED,

    ;

    /** Indica si la orden puede recibir mercancía en este estado. */
    public boolean isReceivable() {
        return this == ORDERED || this == PARTIALLY_RECEIVED;
    }
}
