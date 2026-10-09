package com.hiqnet.pos.audit;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;

import com.hiqnet.pos.branch.domain.Branch;
import com.hiqnet.pos.branch.domain.BranchRepository;
import com.hiqnet.pos.organization.domain.Organization;
import com.hiqnet.pos.organization.domain.OrganizationRepository;
import java.sql.Timestamp;
import java.time.Instant;
import java.util.UUID;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.dao.DataIntegrityViolationException;

/**
 * The append-only guarantee is enforced by PostgreSQL itself, not by JPA or
 * repository discipline: a raw {@code UPDATE}/{@code DELETE} through JDBC is
 * rejected by the BEFORE trigger, and the actor/branch same-organization
 * constraints reject cross-tenant rows even on direct inserts.
 */
class AuditAppendOnlyDbTest extends AbstractAuditIntegrationTest {

    @Autowired
    private OrganizationRepository organizationRepository;

    @Autowired
    private BranchRepository branchRepository;

    @Test
    void updateAndDeleteAreRejectedByTheDatabaseTrigger() {
        RbacFixture fx = fixture();
        UUID eventId = insertAuditEvent(fx.orgId(), null, fx.userId(), "user.updated", "user", fx.userId());

        assertThatThrownBy(() -> jdbcTemplate.update(
                        "update audit_events set action = 'hacked' where id = ?", eventId))
                .isInstanceOf(DataIntegrityViolationException.class);

        assertThatThrownBy(() -> jdbcTemplate.update(
                        "delete from audit_events where id = ?", eventId))
                .isInstanceOf(DataIntegrityViolationException.class);

        // The original row is fully intact.
        assertThat(auditCount(fx.orgId())).isEqualTo(1);
        AuditRow row = latestAudit(fx.orgId());
        assertThat(row.action()).isEqualTo("user.updated");
        assertThat(row.actorUserId()).isEqualTo(fx.userId());
    }

    @Test
    void userActorWithoutUserIdIsRejectedByTheCheckConstraint() {
        RbacFixture fx = fixture();
        assertThatThrownBy(() -> insertAuditEvent(fx.orgId(), null, null, "x.mutated", "x", null))
                .isInstanceOf(DataIntegrityViolationException.class);
        assertThat(auditCount(fx.orgId())).isZero();
    }

    @Test
    void branchOfAnotherOrganizationIsRejectedByTheCompositeFk() {
        RbacFixture fxA = fixture();
        RbacFixture fxB = fixture();
        Organization orgB = organizationRepository.findById(fxB.orgId()).orElseThrow();
        Branch branchB = branchRepository.saveAndFlush(
                Branch.create(orgB, "Branch B", "b-" + UUID.randomUUID().toString().substring(0, 8), null));

        assertThatThrownBy(() -> insertAuditEvent(fxA.orgId(), branchB.getId(), fxA.userId(), "x.mutated", "x", null))
                .isInstanceOf(DataIntegrityViolationException.class);
        assertThat(auditCount(fxA.orgId())).isZero();
    }

    @Test
    void actorOfAnotherOrganizationIsRejectedByTheCompositeFk() {
        RbacFixture fxA = fixture();
        RbacFixture fxB = fixture();

        // Event Organization A with an acting User of Organization B is impossible.
        assertThatThrownBy(() -> insertAuditEvent(fxA.orgId(), null, fxB.userId(), "x.mutated", "x", null))
                .isInstanceOf(DataIntegrityViolationException.class);
        assertThat(auditCount(fxA.orgId())).isZero();
    }

    private UUID insertAuditEvent(
            UUID organizationId,
            UUID branchId,
            UUID actorUserId,
            String action,
            String entityType,
            UUID entityId) {
        UUID id = UUID.randomUUID();
        jdbcTemplate.update(
                """
                insert into audit_events
                    (id, organization_id, branch_id, actor_type, actor_user_id, token_jti,
                     action, entity_type, entity_id, reason, metadata, occurred_at)
                values (?, ?, ?, 'USER', ?, NULL, ?, ?, ?, NULL, NULL, ?)
                """,
                id,
                organizationId,
                branchId,
                actorUserId,
                action,
                entityType,
                entityId,
                Timestamp.from(Instant.now()));
        return id;
    }
}