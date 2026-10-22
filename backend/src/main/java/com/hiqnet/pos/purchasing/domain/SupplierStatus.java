package com.hiqnet.pos.purchasing.domain;

/**
 * Estado operativo de un proveedor.
 *
 * <p>Un proveedor inactivo se conserva para mantener la trazabilidad histórica
 * de futuras órdenes de compra; no existe eliminación física.</p>
 */
public enum SupplierStatus {
    ACTIVE,
    INACTIVE
}
