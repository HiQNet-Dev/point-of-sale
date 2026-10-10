package com.hiqnet.pos.membership.application;

import com.hiqnet.pos.branch.domain.Branch;
import com.hiqnet.pos.branch.domain.BranchRepository;
import com.hiqnet.pos.branch.domain.BranchStatus;
import com.hiqnet.pos.membership.domain.BranchMembership;
import com.hiqnet.pos.membership.domain.BranchMembershipRepository;
import com.hiqnet.pos.membership.domain.BranchMembershipStatus;
import com.hiqnet.pos.membership.domain.BranchMembershipRole;
import com.hiqnet.pos.membership.domain.BranchMembershipRoleRepository;
import com.hiqnet.pos.organization.domain.Organization;
import com.hiqnet.pos.organization.domain.OrganizationRepository;
import com.hiqnet.pos.organization.domain.OrganizationStatus;
import com.hiqnet.pos.rbac.domain.Role;
import com.hiqnet.pos.rbac.domain.RoleRepository;
import com.hiqnet.pos.shared.audit.AuditMetadata;
import com.hiqnet.pos.shared.audit.AuditPort;
import com.hiqnet.pos.shared.audit.AuditRecord;
import com.hiqnet.pos.shared.error.BusinessException;
import com.hiqnet.pos.shared.error.ErrorCodes;
import com.hiqnet.pos.users.domain.User;
import com.hiqnet.pos.users.domain.UserRepository;
import com.hiqnet.pos.users.domain.UserStatus;
import java.util.Comparator;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.UUID;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.data.domain.Page;
import org.springframework.data.domain.Pageable;
import org.springframework.http.HttpStatus;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

/**
 * Use cases of the {@code BranchMembership} aggregate: creation, listing,
 * status changes and the branch-scoped role assignments. Everything is scoped
 * by organization and branch: a membership never resolves without its branch,
 * and a branch never resolves without its organization (IDOR-safe).
 *
 * <p>Management of memberships is ALWAYS authorized with ORGANIZATION-wide
 * permissions ({@code branch.membership.*}) by the controller — never with a
 * branch-scoped grant. A branch manager of a single branch cannot assign
 * memberships or roles to himself or to other branches.</p>
 *
 * <p>Lock order for mutations is fixed: the membership row (PESSIMISTIC_WRITE)
 * is acquired BEFORE any {@code branch_membership_roles} row, never the other
 * way around.</p>
 */
@Service
public class BranchMembershipService {

    private static final Logger log = LoggerFactory.getLogger(BranchMembershipService.class);

    private final BranchMembershipRepository membershipRepository;
    private final BranchMembershipRoleRepository membershipRoleRepository;
    private final OrganizationRepository organizationRepository;
    private final BranchRepository branchRepository;
    private final UserRepository userRepository;
    private final RoleRepository roleRepository;
    private final AuditPort auditPort;

    public BranchMembershipService(
            BranchMembershipRepository membershipRepository,
            BranchMembershipRoleRepository membershipRoleRepository,
            OrganizationRepository organizationRepository,
            BranchRepository branchRepository,
            UserRepository userRepository,
            RoleRepository roleRepository,
            AuditPort auditPort) {
        this.membershipRepository = membershipRepository;
        this.membershipRoleRepository = membershipRoleRepository;
        this.organizationRepository = organizationRepository;
        this.branchRepository = branchRepository;
        this.userRepository = userRepository;
        this.roleRepository = roleRepository;
        this.auditPort = auditPort;
    }

    @Transactional
    public BranchMembership create(UUID organizationId, UUID branchId, UUID userId) {
        Organization organization = requireOrganization(organizationId);
        if (organization.getStatus() != OrganizationStatus.ACTIVE) {
            throw new BusinessException(
                    HttpStatus.CONFLICT, ErrorCodes.ORGANIZATION_INACTIVE, "Organization is inactive");
        }
        Branch branch = branchRepository
                .findByIdAndOrganizationId(branchId, organizationId)
                .orElseThrow(() -> new BusinessException(
                        HttpStatus.NOT_FOUND, ErrorCodes.BRANCH_NOT_FOUND, "Branch not found"));
        if (branch.getStatus() != BranchStatus.ACTIVE) {
            throw new BusinessException(
                    HttpStatus.CONFLICT, ErrorCodes.BRANCH_INACTIVE, "Branch is inactive");
        }
        User user = userRepository
                .findByIdAndOrganizationId(userId, organizationId)
                .orElseThrow(() -> new BusinessException(
                        HttpStatus.NOT_FOUND, ErrorCodes.USER_NOT_FOUND, "User not found"));
        if (user.getStatus() != UserStatus.ACTIVE) {
            // A disabled identity never receives a new operational assignment.
            throw new BusinessException(
                    HttpStatus.CONFLICT, ErrorCodes.USER_INACTIVE, "User is inactive");
        }
        if (membershipRepository.existsByOrganizationIdAndBranchIdAndUserId(organizationId, branchId, userId)) {
            // UX-only pre-check; PostgreSQL's unique constraint is the final
            // authority under concurrency (translated in ConstraintTranslator).
            throw new BusinessException(
                    HttpStatus.CONFLICT,
                    ErrorCodes.BRANCH_MEMBERSHIP_ALREADY_EXISTS,
                    "The user already has a membership in this branch");
        }
        BranchMembership membership = BranchMembership.create(organizationId, branchId, userId);
        membershipRepository.saveAndFlush(membership);
        log.info(
                "security-event=branch-membership-created organizationId={} branchId={} userId={} membershipId={}",
                organizationId,
                branchId,
                userId,
                membership.getId());
        auditPort.record(AuditRecord.ofBranch(
                organizationId,
                branchId,
                "branch.membership.created",
                "branch_membership",
                membership.getId(),
                AuditMetadata.empty()));
        return membership;
    }

    @Transactional(readOnly = true)
    public Page<BranchMembership> list(
            UUID organizationId, UUID branchId, BranchMembershipStatus status, Pageable pageable) {
        requireOrganization(organizationId);
        requireBranch(organizationId, branchId);
        return status == null
                ? membershipRepository.findByOrganizationIdAndBranchId(organizationId, branchId, pageable)
                : membershipRepository.findByOrganizationIdAndBranchIdAndStatus(organizationId, branchId, status, pageable);
    }

    @Transactional(readOnly = true)
    public BranchMembership get(UUID organizationId, UUID branchId, UUID userId) {
        requireOrganization(organizationId);
        requireBranch(organizationId, branchId);
        return membershipRepository
                .findByOrganizationIdAndBranchIdAndUserId(organizationId, branchId, userId)
                .orElseThrow(() -> new BusinessException(
                        HttpStatus.NOT_FOUND,
                        ErrorCodes.BRANCH_MEMBERSHIP_NOT_FOUND,
                        "Branch membership not found"));
    }

    @Transactional
    public BranchMembership update(UUID organizationId, UUID branchId, UUID userId, UpdateMembershipCommand command) {
        requireOrganization(organizationId);
        requireBranch(organizationId, branchId);
        BranchMembership membership = membershipRepository
                .findByOrganizationIdAndBranchIdAndUserIdForUpdate(organizationId, branchId, userId)
                .orElseThrow(() -> new BusinessException(
                        HttpStatus.NOT_FOUND,
                        ErrorCodes.BRANCH_MEMBERSHIP_NOT_FOUND,
                        "Branch membership not found"));
        if (membership.getVersion() != command.expectedVersion()) {
            throw new BusinessException(
                    HttpStatus.CONFLICT,
                    ErrorCodes.CONCURRENT_MODIFICATION,
                    "The resource was modified concurrently; retry with fresh data");
        }
        BranchMembershipStatus statusBefore = membership.getStatus();
        if (command.status() != null) {
            membership.updateStatus(command.status());
        }
        membershipRepository.saveAndFlush(membership);
        log.info(
                "security-event=branch-membership-updated organizationId={} branchId={} userId={} status={}",
                organizationId,
                branchId,
                userId,
                membership.getStatus());
        if (command.status() != null && command.status() != statusBefore) {
            Map<String, Object> metadata = new LinkedHashMap<>();
            metadata.put("fromStatus", statusBefore.name());
            metadata.put("toStatus", membership.getStatus().name());
            auditPort.record(AuditRecord.ofBranch(
                    organizationId,
                    branchId,
                    "branch.membership.updated",
                    "branch_membership",
                    membership.getId(),
                    AuditMetadata.of(metadata)));
        }
        return membership;
    }

    @Transactional(readOnly = true)
    public List<Role> listRoles(UUID organizationId, UUID branchId, UUID userId) {
        BranchMembership membership = requireMembership(organizationId, branchId, userId);
        List<UUID> roleIds = membershipRoleRepository
                .findByOrganizationIdAndMembershipId(organizationId, membership.getId())
                .stream()
                .map(BranchMembershipRole::getRoleId)
                .toList();
        return roleRepository.findByIdInAndOrganizationId(roleIds, organizationId).stream()
                .sorted(Comparator.comparing(Role::getCode))
                .toList();
    }

    @Transactional
    public void replaceRoles(UUID organizationId, UUID branchId, UUID userId, List<UUID> roleIds) {
        if (new HashSet<>(roleIds).size() != roleIds.size()) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST,
                    ErrorCodes.VALIDATION_ERROR,
                    "roleIds must not contain duplicates");
        }
        requireOrganization(organizationId);
        requireBranch(organizationId, branchId);
        // Lock the membership row FIRST (lock order membership -> membership_roles);
        // an unknown/cross-org role fails before ANY mutation (atomic replace).
        BranchMembership membership = membershipRepository
                .findByOrganizationIdAndBranchIdAndUserIdForUpdate(organizationId, branchId, userId)
                .orElseThrow(() -> new BusinessException(
                        HttpStatus.NOT_FOUND,
                        ErrorCodes.BRANCH_MEMBERSHIP_NOT_FOUND,
                        "Branch membership not found"));
        List<Role> scopedRoles = roleRepository.findByIdInAndOrganizationId(roleIds, organizationId);
        if (scopedRoles.size() != new HashSet<>(roleIds).size()) {
            throw new BusinessException(HttpStatus.NOT_FOUND, ErrorCodes.ROLE_NOT_FOUND, "Role not found");
        }
        membershipRoleRepository.deleteByOrganizationIdAndMembershipId(organizationId, membership.getId());
        membershipRoleRepository.saveAll(roleIds.stream()
                .map(roleId -> BranchMembershipRole.of(organizationId, membership.getId(), roleId))
                .toList());
        log.info(
                "security-event=branch-membership-roles-updated organizationId={} branchId={} userId={} roles={}",
                organizationId,
                branchId,
                userId,
                roleIds.size());
        auditPort.record(AuditRecord.ofBranch(
                organizationId,
                branchId,
                "branch.membership.roles.replaced",
                "branch_membership",
                membership.getId(),
                AuditMetadata.of("roleCount", roleIds.size())));
    }

    private BranchMembership requireMembership(UUID organizationId, UUID branchId, UUID userId) {
        requireOrganization(organizationId);
        requireBranch(organizationId, branchId);
        return membershipRepository
                .findByOrganizationIdAndBranchIdAndUserId(organizationId, branchId, userId)
                .orElseThrow(() -> new BusinessException(
                        HttpStatus.NOT_FOUND,
                        ErrorCodes.BRANCH_MEMBERSHIP_NOT_FOUND,
                        "Branch membership not found"));
    }

    private Organization requireOrganization(UUID organizationId) {
        return organizationRepository
                .findById(organizationId)
                .orElseThrow(() -> new BusinessException(
                        HttpStatus.NOT_FOUND, ErrorCodes.ORGANIZATION_NOT_FOUND, "Organization not found"));
    }

    private void requireBranch(UUID organizationId, UUID branchId) {
        if (branchRepository.findByIdAndOrganizationId(branchId, organizationId).isEmpty()) {
            throw new BusinessException(
                    HttpStatus.NOT_FOUND, ErrorCodes.BRANCH_NOT_FOUND, "Branch not found");
        }
    }
}