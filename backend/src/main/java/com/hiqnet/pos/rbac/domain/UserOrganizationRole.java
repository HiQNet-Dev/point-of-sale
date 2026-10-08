package com.hiqnet.pos.rbac.domain;

import jakarta.persistence.Column;
import jakarta.persistence.Entity;
import jakarta.persistence.Id;
import jakarta.persistence.IdClass;
import jakarta.persistence.PrePersist;
import jakarta.persistence.Table;
import java.time.Instant;
import java.util.UUID;

/**
 * Assignment: the user owns this role for the whole organization. Composite
 * FKs in PostgreSQL already guarantee user.organization_id == organization_id
 * == role.organization_id (no cross-tenant assignment possible at the DB).
 */
@Entity
@Table(name = "user_organization_roles")
@IdClass(UserOrganizationRoleId.class)
public class UserOrganizationRole {

    @Id
    @Column(name = "organization_id", nullable = false)
    private UUID organizationId;

    @Id
    @Column(name = "user_id", nullable = false)
    private UUID userId;

    @Id
    @Column(name = "role_id", nullable = false)
    private UUID roleId;

    @Column(name = "created_at", nullable = false, updatable = false)
    private Instant createdAt;

    protected UserOrganizationRole() {
    }

    private UserOrganizationRole(UUID organizationId, UUID userId, UUID roleId) {
        this.organizationId = organizationId;
        this.userId = userId;
        this.roleId = roleId;
    }

    public static UserOrganizationRole of(UUID organizationId, UUID userId, UUID roleId) {
        if (organizationId == null || userId == null || roleId == null) {
            throw new IllegalArgumentException("organizationId, userId and roleId are required");
        }
        return new UserOrganizationRole(organizationId, userId, roleId);
    }

    public UUID getOrganizationId() {
        return organizationId;
    }

    public UUID getUserId() {
        return userId;
    }

    public UUID getRoleId() {
        return roleId;
    }

    public Instant getCreatedAt() {
        return createdAt;
    }

    @PrePersist
    void onCreate() {
        createdAt = Instant.now();
    }
}