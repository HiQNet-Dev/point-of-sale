package com.hiqnet.pos.purchasing.domain;

/**
 * Estado operativo de una orden de compra.
 *
 * <p>El ciclo completo es {@code DRAFT → ORDERED → RECEIVED} (con
 * {@code PARTIALLY_RECEIVED} y {@code CANCELLED} como estados intermedios). Hoy
 * se persisten {@link #DRAFT}, {@link #ORDERED} y {@link #CANCELLED}: una orden
 * se emite ({@code issue}, solo desde {@code DRAFT}) y se cancela ({@code cancel},
 * desde {@code DRAFT} u {@code ORDERED}). Los estados {@code PARTIALLY_RECEIVED}
 * y {@code RECEIVED} llegan con la recepción de mercancía. El estado nunca se
 * acepta desde el cliente ni se modifica por {@code PATCH}.</p>
 */
public enum PurchaseOrderStatus {
    DRAFT,
    ORDERED,
    CANCELLED
}
