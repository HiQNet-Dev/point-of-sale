package com.hiqnet.pos.organization.application;

import com.hiqnet.pos.organization.domain.OrganizationStatus;

/** Partial update payload. Null fields are left unchanged; code is immutable. */
public record UpdateOrganizationCommand(
        String name,
        String timezone,
        String currency,
        String locale,
        OrganizationStatus status) {
}