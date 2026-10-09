package com.hiqnet.pos.membership.domain;

import com.hiqnet.pos.shared.error.BusinessException;
import com.hiqnet.pos.shared.error.ErrorCodes;
import jakarta.persistence.Column;
import jakarta.persistence.Entity;
import jakarta.persistence.EnumType;
import jakarta.persistence.Enumerated;
import jakarta.persistence.Id;
import jakarta.persistence.PrePersist;
import jakarta.persistence.PreUpdate;
import jakarta.persistence.Table;
import jakarta.persistence.UniqueConstraint;
import jakarta.persistence.Version;
import java.time.Instant;
import java.util.UUID;
import org.springframework.http.HttpStatus;

/**
 * Explicit link between a user and a branch inside an organization. At most one
 * membership per (organization, branch, user) — enforced by a unique constraint
 * at the database level, not only in Java.
 *
 * <p>A membership is a first-class aggregate: it has its own {@code status}
 * (INACTIVE keeps the configuration but grants nothing), its own optimistic
 * locking {@code version} and its own branch-scoped role assignments
 * ({@link BranchMembershipRole}). The three ids are plain UUIDs (no JPA
 * navigation): every use case resolves them through organization-scoped
 * repositories, mirroring the «roles» aggregate style.</p>
 */
@Entity
@Table(
        name = "branch_memberships",
        uniqueConstraints = @UniqueConstraint(
                name = "branch_memberships_org_branch_user_unique",
                columnNames = {"organization_id", "branch_id", "user_id"}))
public class BranchMembership {

    @Id
    private UUID id;

    @Column(name = "organization_id", nullable = false)
    private UUID organizationId;

    @Column(name = "branch_id", nullable = false)
    private UUID branchId;

    @Column(name = "user_id", nullable = false)
    private UUID userId;

    @Enumerated(EnumType.STRING)
    @Column(nullable = false, length = 16)
    private BranchMembershipStatus status;

    @Version
    @Column(nullable = false)
    private long version;

    @Column(name = "created_at", nullable = false, updatable = false)
    private Instant createdAt;

    @Column(name = "updated_at", nullable = false)
    private Instant updatedAt;

    protected BranchMembership() {
    }

    private BranchMembership(UUID organizationId, UUID branchId, UUID userId) {
        this.id = UUID.randomUUID();
        this.organizationId = organizationId;
        this.branchId = branchId;
        this.userId = userId;
        this.status = BranchMembershipStatus.ACTIVE;
        this.version = 0;
    }

    /**
     * Factory. The caller must have validated that the organization, branch and
     * user exist, all belong to the same organization and are ACTIVE (a new
     * operational assignment is never created for a disabled identity).
     */
    public static BranchMembership create(UUID organizationId, UUID branchId, UUID userId) {
        if (organizationId == null || branchId == null || userId == null) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, "organizationId, branchId and userId are required");
        }
        return new BranchMembership(organizationId, branchId, userId);
    }

    public UUID getId() {
        return id;
    }

    public UUID getOrganizationId() {
        return organizationId;
    }

    public UUID getBranchId() {
        return branchId;
    }

    public UUID getUserId() {
        return userId;
    }

    public BranchMembershipStatus getStatus() {
        return status;
    }

    public long getVersion() {
        return version;
    }

    public Instant getCreatedAt() {
        return createdAt;
    }

    public Instant getUpdatedAt() {
        return updatedAt;
    }

    public void updateStatus(BranchMembershipStatus newStatus) {
        if (newStatus == null) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, "status is required");
        }
        this.status = newStatus;
    }

    @PrePersist
    void onCreate() {
        Instant now = Instant.now();
        createdAt = now;
        updatedAt = now;
    }

    @PreUpdate
    void onUpdate() {
        updatedAt = Instant.now();
    }
}