package com.hiqnet.pos.audit;

import static org.assertj.core.api.Assertions.assertThat;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.get;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.jsonPath;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import com.hiqnet.pos.branch.domain.Branch;
import com.hiqnet.pos.branch.domain.BranchRepository;
import com.hiqnet.pos.membership.domain.BranchMembership;
import com.hiqnet.pos.membership.domain.BranchMembershipRepository;
import com.hiqnet.pos.membership.domain.BranchMembershipRole;
import com.hiqnet.pos.membership.domain.BranchMembershipRoleRepository;
import com.hiqnet.pos.organization.domain.Organization;
import com.hiqnet.pos.organization.domain.OrganizationRepository;
import com.hiqnet.pos.rbac.domain.Role;
import com.hiqnet.pos.rbac.domain.RoleRepository;
import com.hiqnet.pos.users.domain.User;
import java.sql.Timestamp;
import java.time.Instant;
import java.util.UUID;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.http.MediaType;

/**
 * READ side of the audit log: access control (audit.read organization-wide
 * ONLY), cross-organization isolation, filters, time semantics, deterministic
 * ordering and pagination — and the explicit proof that a branch-scoped role
 * containing audit.read NEVER grants access.
 */
class AuditReadApiTest extends AbstractAuditIntegrationTest {

    private static final String INSERT_SQL =
            """
            insert into audit_events
                (id, organization_id, branch_id, actor_type, actor_user_id, token_jti,
                 action, entity_type, entity_id, reason, metadata, occurred_at)
            values (?, ?, ?, 'USER', ?, ?, ?, ?, ?, NULL, ?::jsonb, ?)
            """;

    @Autowired
    private OrganizationRepository organizationRepository;

    @Autowired
    private BranchRepository branchRepository;

    @Autowired
    private RoleRepository roleRepository;

    @Autowired
    private BranchMembershipRepository membershipRepository;

    @Autowired
    private BranchMembershipRoleRepository membershipRoleRepository;

    @Test
    void auditReadWithoutTokenIsUnauthorized() throws Exception {
        mockMvc.perform(get("/api/v1/organizations/" + UUID.randomUUID() + "/audit-events"))
                .andExpect(status().isUnauthorized())
                .andExpect(jsonPath("$.code").value("AUTHENTICATION_REQUIRED"));
    }

    @Test
    void auditReadWithoutPermissionIsForbidden() throws Exception {
        RbacFixture fx = fixture();
        User restricted =
                createUserForOrganization(fx.orgId().toString(), uniqueEmail("audit-ro@"), PASSWORD);
        assignRoleTo(fx, restricted.getId(), "user.read");
        String token = postLoginExpectingToken(fx.orgCode(), emailFor(restricted), PASSWORD);

        mockMvc.perform(get(auditUrl(fx.orgId())).header("Authorization", bearer(token)))
                .andExpect(status().isForbidden())
                .andExpect(jsonPath("$.code").value("FORBIDDEN"));
    }

    @Test
    void branchScopedAuditReadNeverGrantsAccess() throws Exception {
        RbacFixture fx = fixture();
        Organization organization = organizationRepository.findById(fx.orgId()).orElseThrow();
        Branch branch = branchRepository.saveAndFlush(
                Branch.create(organization, "Branch Audit", "audit-suc", null));
        Role role = roleRepository.saveAndFlush(Role.create(fx.orgId(), "branch-auditor", "Branch Auditor"));
        grantPermissionsToRole(role.getId(), "audit.read");
        BranchMembership membership = membershipRepository.saveAndFlush(
                BranchMembership.create(fx.orgId(), branch.getId(), fx.userId()));
        membershipRoleRepository.saveAndFlush(
                BranchMembershipRole.of(fx.orgId(), membership.getId(), role.getId()));
        String token = login(fx); // ONLY the branch-scoped grant

        mockMvc.perform(get(auditUrl(fx.orgId())).header("Authorization", bearer(token)))
                .andExpect(status().isForbidden())
                .andExpect(jsonPath("$.code").value("FORBIDDEN"));
    }

    @Test
    void organizationWideAuditReadListsEventsNewestFirst() throws Exception {
        RbacFixture fx = fixture();
        Instant t0 = Instant.parse("2026-10-01T10:00:00Z");
        insertEvent(fx.orgId(), null, fx.userId(), "organization.updated", "organization", fx.orgId(), t0);
        insertEvent(fx.orgId(), null, fx.userId(), "user.created", "user", UUID.randomUUID(), t0.plusSeconds(10));
        insertEvent(fx.orgId(), null, fx.userId(), "role.created", "role", UUID.randomUUID(), t0.plusSeconds(20));
        String token = adminToken(fx);

        mockMvc.perform(get(auditUrl(fx.orgId())).header("Authorization", bearer(token)))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.totalElements").value(3))
                .andExpect(jsonPath("$.items[0].action").value("role.created"))
                .andExpect(jsonPath("$.items[1].action").value("user.created"))
                .andExpect(jsonPath("$.items[2].action").value("organization.updated"))
                .andExpect(jsonPath("$.items[0].actorType").value("USER"))
                .andExpect(jsonPath("$.items[0].actorUserId").value(fx.userId().toString()));
    }

    @Test
    void crossOrganizationReadIsForbidden() throws Exception {
        RbacFixture fxA = fixture();
        RbacFixture fxB = fixture();
        insertEvent(fxB.orgId(), null, fxB.userId(), "user.created", "user", UUID.randomUUID(), Instant.now());
        String tokenA = adminToken(fxA);

        mockMvc.perform(get(auditUrl(fxB.orgId())).header("Authorization", bearer(tokenA)))
                .andExpect(status().isForbidden())
                .andExpect(jsonPath("$.code").value("FORBIDDEN"));
    }

    @Test
    void branchIdFilterAndForeignBranchReject() throws Exception {
        RbacFixture fx = fixture();
        Organization organization = organizationRepository.findById(fx.orgId()).orElseThrow();
        Branch branch = branchRepository.saveAndFlush(
                Branch.create(organization, "Branch F", "f-suc", null));
        Instant now = Instant.now();
        insertEvent(fx.orgId(), branch.getId(), fx.userId(), "branch.updated", "branch", branch.getId(), now);
        insertEvent(fx.orgId(), null, fx.userId(), "user.created", "user", UUID.randomUUID(), now.plusSeconds(1));
        String token = adminToken(fx);

        mockMvc.perform(get(auditUrl(fx.orgId()))
                        .param("branchId", branch.getId().toString())
                        .header("Authorization", bearer(token)))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.totalElements").value(1))
                .andExpect(jsonPath("$.items[0].entityType").value("branch"));

        // A branch of another organization is NOT silently filtered to zero.
        RbacFixture fxB = fixture();
        Organization orgB = organizationRepository.findById(fxB.orgId()).orElseThrow();
        Branch branchB = branchRepository.saveAndFlush(
                Branch.create(orgB, "Branch B", "b-suc", null));
        mockMvc.perform(get(auditUrl(fx.orgId()))
                        .param("branchId", branchB.getId().toString())
                        .header("Authorization", bearer(token)))
                .andExpect(status().isNotFound())
                .andExpect(jsonPath("$.code").value("BRANCH_NOT_FOUND"));
    }

    @Test
    void actorUserIdFilterAndForeignUserReject() throws Exception {
        RbacFixture fx = fixture();
        UUID actorA = fx.userId();
        User secondActor =
                createUserForOrganization(fx.orgId().toString(), uniqueEmail("audit-actor2@"), PASSWORD);
        UUID other = UUID.randomUUID();
        Instant now = Instant.now();
        insertEvent(fx.orgId(), null, actorA, "user.created", "user", other, now);
        insertEvent(fx.orgId(), null, secondActor.getId(), "role.created", "role", UUID.randomUUID(), now.plusSeconds(1));
        String token = adminToken(fx);

        mockMvc.perform(get(auditUrl(fx.orgId()))
                        .param("actorUserId", actorA.toString())
                        .header("Authorization", bearer(token)))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.totalElements").value(1))
                .andExpect(jsonPath("$.items[0].actorUserId").value(actorA.toString()));

        mockMvc.perform(get(auditUrl(fx.orgId()))
                        .param("actorUserId", UUID.randomUUID().toString())
                        .header("Authorization", bearer(token)))
                .andExpect(status().isNotFound())
                .andExpect(jsonPath("$.code").value("USER_NOT_FOUND"));
    }

    @Test
    void actionAndEntityFiltersAreApplied() throws Exception {
        RbacFixture fx = fixture();
        UUID entity = UUID.randomUUID();
        Instant now = Instant.now();
        insertEvent(fx.orgId(), null, fx.userId(), "user.created", "user", entity, now);
        insertEvent(fx.orgId(), null, fx.userId(), "user.updated", "user", entity, now.plusSeconds(1));
        insertEvent(fx.orgId(), null, fx.userId(), "role.created", "role", UUID.randomUUID(), now.plusSeconds(2));
        String token = adminToken(fx);

        mockMvc.perform(get(auditUrl(fx.orgId()))
                        .param("action", "user.created")
                        .header("Authorization", bearer(token)))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.totalElements").value(1));

        mockMvc.perform(get(auditUrl(fx.orgId()))
                        .param("entityType", "user")
                        .param("entityId", entity.toString())
                        .header("Authorization", bearer(token)))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.totalElements").value(2));
    }

    @Test
    void fromIsInclusiveToIsExclusiveAndFromAfterToIsRejected() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        Instant t1 = Instant.parse("2026-10-01T10:00:00Z");
        Instant t2 = t1.plusSeconds(60);
        Instant t3 = t2.plusSeconds(60);
        insertEvent(fx.orgId(), null, fx.userId(), "user.created", "user", UUID.randomUUID(), t1);
        insertEvent(fx.orgId(), null, fx.userId(), "user.updated", "user", UUID.randomUUID(), t2);
        insertEvent(fx.orgId(), null, fx.userId(), "user.deactivated", "user", UUID.randomUUID(), t3);

        mockMvc.perform(get(auditUrl(fx.orgId()))
                        .param("from", t1.toString())
                        .param("to", t3.toString())
                        .header("Authorization", bearer(token)))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.totalElements").value(2)) // includes t1, t2; excludes t3
                .andExpect(jsonPath("$.items[0].action").value("user.updated"))
                .andExpect(jsonPath("$.items[1].action").value("user.created"));

        mockMvc.perform(get(auditUrl(fx.orgId()))
                        .param("from", t3.toString())
                        .param("to", t1.toString())
                        .header("Authorization", bearer(token)))
                .andExpect(status().isBadRequest())
                .andExpect(jsonPath("$.code").value("VALIDATION_ERROR"));
    }

    @Test
    void paginationHonoursDeterministicOrdering() throws Exception {
        RbacFixture fx = fixture();
        Instant t0 = Instant.parse("2026-10-01T10:00:00Z");
        insertEvent(fx.orgId(), null, fx.userId(), "a.created", "a", UUID.randomUUID(), t0);
        insertEvent(fx.orgId(), null, fx.userId(), "b.created", "b", UUID.randomUUID(), t0.plusSeconds(10));
        insertEvent(fx.orgId(), null, fx.userId(), "c.created", "c", UUID.randomUUID(), t0.plusSeconds(20));
        String token = adminToken(fx);

        mockMvc.perform(get(auditUrl(fx.orgId()))
                        .param("size", "2")
                        .header("Authorization", bearer(token)))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.totalPages").value(2))
                .andExpect(jsonPath("$.totalElements").value(3))
                .andExpect(jsonPath("$.items.length()").value(2))
                .andExpect(jsonPath("$.items[0].action").value("c.created"))
                .andExpect(jsonPath("$.items[1].action").value("b.created"));
    }

    @Test
    void mutationEndpointsRemainDenied() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);

        mockMvc.perform(post(auditUrl(fx.orgId()))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{}"))
                .andExpect(status().isForbidden())
                .andExpect(jsonPath("$.code").value("FORBIDDEN"));
    }

    // --- fixtures ----------------------------------------------------------

    private UUID insertEvent(
            UUID organizationId,
            UUID branchId,
            UUID actorUserId,
            String action,
            String entityType,
            UUID entityId,
            Instant occurredAt) {
        UUID id = UUID.randomUUID();
        jdbcTemplate.update(
                INSERT_SQL,
                id,
                organizationId,
                branchId,
                actorUserId,
                "jti-" + UUID.randomUUID(),
                action,
                entityType,
                entityId,
                "{}",
                Timestamp.from(occurredAt));
        return id;
    }

    private void assignRoleTo(RbacFixture fx, UUID targetUserId, String permissionCode) {
        UUID roleId = createRole(fx, "actor-" + UUID.randomUUID().toString().substring(0, 8), "Actor", permissionCode);
        userOrganizationRoleRepository.saveAndFlush(
                com.hiqnet.pos.rbac.domain.UserOrganizationRole.of(fx.orgId(), targetUserId, roleId));
    }

    private static String auditUrl(UUID organizationId) {
        return "/api/v1/organizations/" + organizationId + "/audit-events";
    }
}