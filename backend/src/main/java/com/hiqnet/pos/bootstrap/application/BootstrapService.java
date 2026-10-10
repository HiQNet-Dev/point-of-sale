package com.hiqnet.pos.bootstrap.application;

import com.hiqnet.pos.bootstrap.domain.InstallationBootstrap;
import com.hiqnet.pos.bootstrap.domain.InstallationBootstrapRepository;
import com.hiqnet.pos.organization.application.OrganizationService;
import com.hiqnet.pos.organization.domain.Organization;
import com.hiqnet.pos.organization.domain.OrganizationRepository;
import com.hiqnet.pos.rbac.application.PermissionService;
import com.hiqnet.pos.rbac.application.RolePermissionService;
import com.hiqnet.pos.rbac.application.RoleService;
import com.hiqnet.pos.rbac.application.UserRoleService;
import com.hiqnet.pos.rbac.domain.Role;
import com.hiqnet.pos.shared.audit.AuditMetadata;
import com.hiqnet.pos.shared.audit.AuditPort;
import com.hiqnet.pos.shared.audit.AuditRecord;
import com.hiqnet.pos.shared.error.BusinessException;
import com.hiqnet.pos.shared.error.ErrorCodes;
import com.hiqnet.pos.users.application.UserService;
import com.hiqnet.pos.users.domain.User;
import java.time.Clock;
import java.util.List;
import java.util.UUID;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.http.HttpStatus;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

/**
 * One-shot, deployment-secret-gated, database-coordinated, transactional
 * provisioning of the first installation:
 *
 * <pre>
 * lock installation_bootstrap (id=1) FOR UPDATE
 *   - already completed            -> 409 BOOTSTRAP_ALREADY_COMPLETED
 *   - organizations already exist  -> 409 BOOTSTRAP_NOT_ALLOWED (no adoption)
 *   - create Organization          (+ audit organization.created)
 *   - create administrator User
 *   - create administrator Role (a NORMAL role: code=administrator, no magic)
 *   - grant EVERY current catalog permission (from PostgreSQL, never a second
 *     hardcoded catalog)
 *   - assign the Role to the User via user_organization_roles (organization-wide)
 *   - mark installation_bootstrap COMPLETED
 * commit (single transaction)
 * </pre>
 *
 * <p>Everything — including the audit events and the bootstrap state — commits
 * or rolls back together ({@code REQUIRED}); an audit failure rolls the whole
 * provisioning back and the installation stays PENDING and retryable. The actor
 * is {@code SYSTEM}: no user exists yet and the HTTP guard never introduces a
 * fake Authentication.</p>
 *
 * <p>Direction: {@code bootstrap -> organization, users, rbac, shared.audit}.
 * No foundation module depends on bootstrap.</p>
 */
@Service
public class BootstrapService {

    private static final Logger log = LoggerFactory.getLogger(BootstrapService.class);

    private static final String ADMIN_ROLE_CODE = "administrator";
    private static final String ADMIN_ROLE_NAME = "Administrator";

    private final InstallationBootstrapRepository bootstrapRepository;
    private final OrganizationRepository organizationRepository;
    private final OrganizationService organizationService;
    private final UserService userService;
    private final RoleService roleService;
    private final RolePermissionService rolePermissionService;
    private final UserRoleService userRoleService;
    private final PermissionService permissionService;
    private final AuditPort auditPort;
    private final Clock clock;

    public BootstrapService(
            InstallationBootstrapRepository bootstrapRepository,
            OrganizationRepository organizationRepository,
            OrganizationService organizationService,
            UserService userService,
            RoleService roleService,
            RolePermissionService rolePermissionService,
            UserRoleService userRoleService,
            PermissionService permissionService,
            AuditPort auditPort,
            Clock clock) {
        this.bootstrapRepository = bootstrapRepository;
        this.organizationRepository = organizationRepository;
        this.organizationService = organizationService;
        this.userService = userService;
        this.roleService = roleService;
        this.rolePermissionService = rolePermissionService;
        this.userRoleService = userRoleService;
        this.permissionService = permissionService;
        this.auditPort = auditPort;
        this.clock = clock;
    }

    @Transactional
    public BootstrapResult bootstrap(BootstrapCommand command) {
        // PostgreSQL coordination: serialize concurrent bootstraps on the
        // singleton row (PESSIMISTIC_WRITE). Exactly one request wins.
        InstallationBootstrap state = bootstrapRepository
                .findByIdForUpdate(InstallationBootstrap.SINGLETON_ID)
                .orElseThrow(() -> new IllegalStateException(
                        "installation_bootstrap singleton row (id=1) is missing; V8 must have seeded it"));
        if (state.isCompleted()) {
            throw new BusinessException(
                    HttpStatus.CONFLICT,
                    ErrorCodes.BOOTSTRAP_ALREADY_COMPLETED,
                    "Bootstrap has already been completed");
        }
        // Pristine-installation guard: never "adopt" a database that already
        // holds organizations (created manually or by another process).
        if (organizationRepository.count() > 0) {
            throw new BusinessException(
                    HttpStatus.CONFLICT,
                    ErrorCodes.BOOTSTRAP_NOT_ALLOWED,
                    "Bootstrap is not allowed: the installation already contains organizations");
        }

        Organization organization = organizationService.create(
                command.organizationName(),
                command.organizationCode(),
                command.timezone(),
                command.currency(),
                command.locale());
        // organization.created is a provisioning event; OrganizationService
        // intentionally does not record it (POST /organizations stays closed).
        auditPort.record(AuditRecord.of(
                organization.getId(),
                "organization.created",
                "organization",
                organization.getId(),
                AuditMetadata.empty()));

        User administrator = userService.create(
                organization.getId(), command.administratorEmail(), command.administratorPassword());
        Role administratorRole = roleService.create(organization.getId(), ADMIN_ROLE_CODE, ADMIN_ROLE_NAME);

        // The initial role is a NORMAL role granted EVERY permission currently
        // in the catalog, read from PostgreSQL (no second hardcoded list; the
        // count is whatever the catalog really is today).
        List<String> allPermissionCodes = permissionService.list().stream()
                .map(permission -> permission.getCode())
                .toList();
        rolePermissionService.replace(organization.getId(), administratorRole.getId(), allPermissionCodes);

        // Organization-wide assignment (user_organization_roles), never a
        // BranchMembership: the administrator governs the whole organization.
        userRoleService.replace(organization.getId(), administrator.getId(), List.of(administratorRole.getId()));

        state.complete(organization.getId(), administrator.getId(), administratorRole.getId(), clock.instant());
        bootstrapRepository.saveAndFlush(state);

        log.info(
                "security-event=bootstrap-completed organizationId={} administratorUserId={} administratorRoleId={}",
                organization.getId(),
                administrator.getId(),
                administratorRole.getId());
        return new BootstrapResult(
                organization.getId(), organization.getCode(), administrator.getId(), administratorRole.getId());
    }
}