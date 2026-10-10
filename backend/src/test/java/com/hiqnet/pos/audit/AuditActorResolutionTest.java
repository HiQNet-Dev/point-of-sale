package com.hiqnet.pos.audit;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;

import com.hiqnet.pos.branch.application.BranchService;
import com.hiqnet.pos.branch.application.UpdateBranchCommand;
import com.hiqnet.pos.branch.domain.Branch;
import com.hiqnet.pos.branch.domain.BranchRepository;
import com.hiqnet.pos.organization.domain.Organization;
import com.hiqnet.pos.organization.domain.OrganizationRepository;
import com.hiqnet.pos.shared.audit.AuditException;
import com.hiqnet.pos.shared.audit.AuditMetadata;
import com.hiqnet.pos.shared.audit.AuditRecord;
import java.util.List;
import java.util.UUID;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.security.authentication.AnonymousAuthenticationToken;
import org.springframework.security.authentication.UsernamePasswordAuthenticationToken;
import org.springframework.security.core.authority.SimpleGrantedAuthority;
import org.springframework.security.core.context.SecurityContextHolder;
import org.springframework.security.oauth2.jwt.Jwt;
import org.springframework.transaction.support.TransactionTemplate;

/**
 * Actor attribution is FAIL-CLOSED: only a JWT principal becomes USER (with a
 * required jti); an empty context, an anonymous token or an unauthenticated
 * token become SYSTEM (no technical user); ANY OTHER authenticated principal
 * throws — a misconfigured future authentication mechanism is never silently
 * recorded as SYSTEM, and the surrounding business transaction rolls back.
 */
class AuditActorResolutionTest extends AbstractAuditIntegrationTest {

    @Autowired
    private BranchService branchService;

    @Autowired
    private BranchRepository branchRepository;

    @Autowired
    private OrganizationRepository organizationRepository;

    @Test
    void jwtPrincipalIsResolvedToUserActor() {
        RbacFixture fx = fixture();
        Jwt jwt = jwt(fx.userId().toString(), "jti-ok");
        setAuthentication(new UsernamePasswordAuthenticationToken(jwt, null, List.of()));

        try {
            recordInTransaction(fx.orgId());
        } finally {
            SecurityContextHolder.clearContext();
        }

        AuditRow row = latestAudit(fx.orgId());
        assertThat(row.actorType()).isEqualTo("USER");
        assertThat(row.actorUserId()).isEqualTo(fx.userId());
        assertThat(row.tokenJti()).isEqualTo("jti-ok");
    }

    @Test
    void emptySecurityContextIsSystemActor() {
        RbacFixture fx = fixture();
        SecurityContextHolder.clearContext();

        recordInTransaction(fx.orgId());

        AuditRow row = latestAudit(fx.orgId());
        assertThat(row.actorType()).isEqualTo("SYSTEM");
        assertThat(row.actorUserId()).isNull();
        assertThat(row.tokenJti()).isNull();
    }

    @Test
    void anonymousAuthenticationIsSystemActor() {
        RbacFixture fx = fixture();
        setAuthentication(new AnonymousAuthenticationToken(
                "key", "anonymousUser", List.of(new SimpleGrantedAuthority("ROLE_ANONYMOUS"))));

        try {
            recordInTransaction(fx.orgId());
        } finally {
            SecurityContextHolder.clearContext();
        }

        AuditRow row = latestAudit(fx.orgId());
        assertThat(row.actorType()).isEqualTo("SYSTEM");
        assertThat(row.actorUserId()).isNull();
    }

    @Test
    void authenticatedNonJwtPrincipalFailsClosed() {
        RbacFixture fx = fixture();
        setAuthentication(new UsernamePasswordAuthenticationToken("someone", null, List.of()));

        try {
            assertThatThrownBy(() -> recordInTransaction(fx.orgId()))
                    .isInstanceOf(AuditException.class)
                    .hasMessageContaining("cannot attribute");
        } finally {
            SecurityContextHolder.clearContext();
        }
        assertThat(auditCount(fx.orgId())).isZero();
    }

    @Test
    void jwtWithNonUuidSubjectFailsClosed() {
        RbacFixture fx = fixture();
        setAuthentication(new UsernamePasswordAuthenticationToken(jwt("not-a-uuid", "jti-x"), null, List.of()));

        try {
            assertThatThrownBy(() -> recordInTransaction(fx.orgId()))
                    .isInstanceOf(AuditException.class)
                    .hasMessageContaining("non-UUID subject");
        } finally {
            SecurityContextHolder.clearContext();
        }
    }

    @Test
    void jwtWithoutJtiFailsClosed() {
        RbacFixture fx = fixture();
        Jwt withoutJti = Jwt.withTokenValue("token")
                .header("alg", "HS256")
                .subject(fx.userId().toString())
                .build();
        setAuthentication(new UsernamePasswordAuthenticationToken(withoutJti, null, List.of()));

        try {
            assertThatThrownBy(() -> recordInTransaction(fx.orgId()))
                    .isInstanceOf(AuditException.class)
                    .hasMessageContaining("jti");
        } finally {
            SecurityContextHolder.clearContext();
        }
    }

    @Test
    void attributionFailureRollsBackTheBusinessMutation() {
        RbacFixture fx = fixture();
        Organization organization = organizationRepository.findById(fx.orgId()).orElseThrow();
        Branch branch = branchRepository.saveAndFlush(
                Branch.create(organization, "Old Name", "attr-rollback", null));
        UUID branchId = branch.getId();

        setAuthentication(new UsernamePasswordAuthenticationToken("someone", null, List.of()));
        try {
            assertThatThrownBy(() -> branchService.update(
                            fx.orgId(), branchId, new UpdateBranchCommand(0, "New Name", null, null)))
                    .isInstanceOf(AuditException.class);
        } finally {
            SecurityContextHolder.clearContext();
        }

        Branch reloaded = branchRepository.findById(branchId).orElseThrow();
        assertThat(reloaded.getName()).isEqualTo("Old Name");
        assertThat(reloaded.getVersion()).isZero();
        assertThat(auditCount(fx.orgId())).isZero();
    }

    // -- helpers -------------------------------------------------------------

    private Jwt jwt(String subject, String jti) {
        return Jwt.withTokenValue("token")
                .header("alg", "HS256")
                .subject(subject)
                .claim("jti", jti)
                .build();
    }

    private void setAuthentication(org.springframework.security.core.Authentication authentication) {
        SecurityContextHolder.clearContext();
        SecurityContextHolder.getContext().setAuthentication(authentication);
    }

    private void recordInTransaction(UUID organizationId) {
        new TransactionTemplate(transactionManager).executeWithoutResult(status ->
                auditPort.record(AuditRecord.of(
                        organizationId, "actor.resolution", "test", null, AuditMetadata.empty())));
    }
}