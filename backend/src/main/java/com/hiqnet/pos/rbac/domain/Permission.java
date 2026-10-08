package com.hiqnet.pos.rbac.domain;

import jakarta.persistence.Column;
import jakarta.persistence.Entity;
import jakarta.persistence.Id;
import jakarta.persistence.Table;

/**
 * One entry of the system-wide permission catalog. Identity is the natural,
 * immutable {@code code} (e.g. {@code branch.create}); the catalog is seeded by
 * Flyway and has NO client mutation API (no create/update/delete endpoints).
 */
@Entity
@Table(name = "permissions")
public class Permission {

    @Id
    @Column(length = 64)
    private String code;

    @Column(nullable = false, length = 255)
    private String description;

    protected Permission() {
    }

    public String getCode() {
        return code;
    }

    public String getDescription() {
        return description;
    }
}