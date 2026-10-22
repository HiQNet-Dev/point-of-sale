package com.hiqnet.pos.purchasing.application;

import com.hiqnet.pos.purchasing.domain.SupplierStatus;

/**
 * Datos de una actualización parcial de proveedor.
 *
 * <p>Los valores {@code null} representan campos no enviados y no producen
 * cambios. Para los textos opcionales, una cadena en blanco elimina el valor
 * persistido.</p>
 */
public record UpdateSupplierCommand(
        long expectedVersion,
        String name,
        String legalName,
        String taxId,
        String contactName,
        String email,
        String phone,
        String address,
        SupplierStatus status) {
}
