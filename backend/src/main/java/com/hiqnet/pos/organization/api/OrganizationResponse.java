package com.hiqnet.pos.organization.api;

import com.hiqnet.pos.organization.domain.InventoryNegativeStockPolicy;
import com.hiqnet.pos.organization.domain.Organization;
import com.hiqnet.pos.organization.domain.OrganizationStatus;
import java.time.Instant;
import java.util.UUID;

public record OrganizationResponse(
        UUID id,
        String name,
        String code,
        String timezone,
        String currency,
        String locale,
        OrganizationStatus status,
        InventoryNegativeStockPolicy inventoryNegativeStockPolicy,
        long version,
        Instant createdAt,
        Instant updatedAt) {

    public static OrganizationResponse from(Organization organization) {
        return new OrganizationResponse(
                organization.getId(),
                organization.getName(),
                organization.getCode(),
                organization.getTimezone(),
                organization.getCurrency(),
                organization.getLocale(),
                organization.getStatus(),
                organization.getInventoryNegativeStockPolicy(),
                organization.getVersion(),
                organization.getCreatedAt(),
                organization.getUpdatedAt());
    }
}