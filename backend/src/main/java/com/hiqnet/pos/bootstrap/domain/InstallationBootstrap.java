package com.hiqnet.pos.bootstrap.domain;

import jakarta.persistence.Column;
import jakarta.persistence.Entity;
import jakarta.persistence.Id;
import jakarta.persistence.Table;
import java.time.Instant;
import java.util.UUID;

/**
 * The single installation row ({@code id = 1}) that records one-shot bootstrap
 * state. It is technical provisioning state, NOT a credential: the bootstrap
 * secret never reaches the database. The row is either PENDING (everything
 * {@code null}) or COMPLETED (everything set) — the database enforces both the
 * singleton and the completeness invariants.
 */
@Entity
@Table(name = "installation_bootstrap")
public class InstallationBootstrap {

    /** The only valid id (CHECK-enforced singleton). */
    public static final short SINGLETON_ID = 1;

    @Id
    @Column(name = "id", nullable = false)
    private Short id;

    @Column(name = "completed_at")
    private Instant completedAt;

    @Column(name = "organization_id")
    private UUID organizationId;

    @Column(name = "administrator_user_id")
    private UUID administratorUserId;

    @Column(name = "administrator_role_id")
    private UUID administratorRoleId;

    protected InstallationBootstrap() {
    }

    public boolean isCompleted() {
        return completedAt != null;
    }

    /**
     * Marks provisioning complete, recording the identities created. Values are
     * never partially set: the entity is only ever brought to the COMPLETED
     * state, and the database CHECK rejects anything partial.
     */
    public void complete(
            UUID organizationId, UUID administratorUserId, UUID administratorRoleId, Instant completedAt) {
        this.organizationId = organizationId;
        this.administratorUserId = administratorUserId;
        this.administratorRoleId = administratorRoleId;
        this.completedAt = completedAt;
    }

    public Short getId() {
        return id;
    }

    public Instant getCompletedAt() {
        return completedAt;
    }

    public UUID getOrganizationId() {
        return organizationId;
    }

    public UUID getAdministratorUserId() {
        return administratorUserId;
    }

    public UUID getAdministratorRoleId() {
        return administratorRoleId;
    }
}
