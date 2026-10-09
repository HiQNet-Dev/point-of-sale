package com.hiqnet.pos.membership.domain;

import jakarta.persistence.Column;
import jakarta.persistence.Entity;
import jakarta.persistence.Id;
import jakarta.persistence.IdClass;
import jakarta.persistence.PrePersist;
import jakarta.persistence.Table;
import java.time.Instant;
import java.util.UUID;

/**
 * Branch-scoped role assignment of a {@link BranchMembership}. Roles are the
 * SAME organization-scoped definitions used by {@code user_organization_roles}
 * (there is no separate BranchRole model). Composite FKs in PostgreSQL already
 * guarantee membership.organization_id == role.organization_id, so a role of
 * another organization can never be attached to a membership.
 */
@Entity
@Table(name = "branch_membership_roles")
@IdClass(BranchMembershipRoleId.class)
public class BranchMembershipRole {

    @Id
    @Column(name = "organization_id", nullable = false)
    private UUID organizationId;

    @Id
    @Column(name = "membership_id", nullable = false)
    private UUID membershipId;

    @Id
    @Column(name = "role_id", nullable = false)
    private UUID roleId;

    @Column(name = "created_at", nullable = false, updatable = false)
    private Instant createdAt;

    protected BranchMembershipRole() {
    }

    private BranchMembershipRole(UUID organizationId, UUID membershipId, UUID roleId) {
        this.organizationId = organizationId;
        this.membershipId = membershipId;
        this.roleId = roleId;
    }

    public static BranchMembershipRole of(UUID organizationId, UUID membershipId, UUID roleId) {
        if (organizationId == null || membershipId == null || roleId == null) {
            throw new IllegalArgumentException("organizationId, membershipId and roleId are required");
        }
        return new BranchMembershipRole(organizationId, membershipId, roleId);
    }

    public UUID getOrganizationId() {
        return organizationId;
    }

    public UUID getMembershipId() {
        return membershipId;
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