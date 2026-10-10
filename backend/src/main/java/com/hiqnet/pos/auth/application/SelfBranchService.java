package com.hiqnet.pos.auth.application;

import com.hiqnet.pos.branch.domain.Branch;
import com.hiqnet.pos.branch.domain.BranchRepository;
import com.hiqnet.pos.branch.domain.BranchStatus;
import com.hiqnet.pos.membership.domain.BranchMembershipRepository;
import com.hiqnet.pos.membership.domain.BranchMembershipStatus;
import com.hiqnet.pos.organization.domain.Organization;
import com.hiqnet.pos.organization.domain.OrganizationRepository;
import com.hiqnet.pos.rbac.domain.UserOrganizationRoleRepository;
import com.hiqnet.pos.shared.error.BusinessException;
import com.hiqnet.pos.shared.error.ErrorCodes;
import com.hiqnet.pos.users.domain.User;
import com.hiqnet.pos.users.domain.UserRepository;
import jakarta.persistence.criteria.Predicate;
import java.util.ArrayList;
import java.util.Collection;
import java.util.List;
import java.util.Locale;
import java.util.UUID;
import org.springframework.data.domain.Page;
import org.springframework.data.domain.Pageable;
import org.springframework.data.jpa.domain.Specification;
import org.springframework.http.HttpStatus;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

/**
 * Self-scoped branch discovery for the authenticated identity:
 * {@code GET /api/v1/auth/me/branches}. This is the secure counterpart of the
 * organization-scoped {@code GET /organizations/{orgId}/branches} list, which
 * requires {@code branch.read} org-wide.
 *
 * <p>Visibility rule (not authorization): the identity always sees the branches
 * it is ACTIVELY a member of; a user that holds {@code branch.read} org-wide
 * sees every branch of its own organization. Membership is resolved by
 * {@code membership.status = ACTIVE} only — a deactivated branch stays
 * discoverable so an already-open operational resource can be recovered, and
 * the effective role state is irrelevant here (visibility ≠ authorization).
 * The organization and user are always derived from the token identity, never
 * from client input, so no cross-organization enumeration is possible.</p>
 */
@Service
public class SelfBranchService {

    private static final String BRANCH_READ_PERMISSION = "branch.read";

    private final UserRepository userRepository;
    private final OrganizationRepository organizationRepository;
    private final UserOrganizationRoleRepository userOrganizationRoleRepository;
    private final BranchRepository branchRepository;
    private final BranchMembershipRepository branchMembershipRepository;

    public SelfBranchService(
            UserRepository userRepository,
            OrganizationRepository organizationRepository,
            UserOrganizationRoleRepository userOrganizationRoleRepository,
            BranchRepository branchRepository,
            BranchMembershipRepository branchMembershipRepository) {
        this.userRepository = userRepository;
        this.organizationRepository = organizationRepository;
        this.userOrganizationRoleRepository = userOrganizationRoleRepository;
        this.branchRepository = branchRepository;
        this.branchMembershipRepository = branchMembershipRepository;
    }

    /**
     * Paginated branches reachable by the identity. Same operational gating as
     * {@code /me}: an inactive user or organization yields 401 TOKEN_INVALID.
     * An identity with no visible branch returns an EMPTY page (200), never 403.
     */
    @Transactional(readOnly = true)
    public Page<Branch> listMyBranches(UUID userId, BranchStatus status, String q, Pageable pageable) {
        User user = userRepository.findById(userId).orElseThrow(SelfBranchService::invalidTokenIdentity);
        Organization organization =
                organizationRepository.findById(user.getOrganizationId()).orElse(null);
        if (!user.isActive() || organization == null || !organization.isActive()) {
            throw invalidTokenIdentity();
        }
        UUID organizationId = user.getOrganizationId();
        if (userOrganizationRoleRepository.existsEffectivePermission(
                userId, organizationId, BRANCH_READ_PERMISSION)) {
            return branchRepository.findAll(specification(organizationId, null, status, q), pageable);
        }
        List<UUID> branchIds = branchMembershipRepository.findBranchIdsByOrganizationIdAndUserIdAndStatus(
                organizationId, userId, BranchMembershipStatus.ACTIVE);
        if (branchIds.isEmpty()) {
            return Page.empty(pageable);
        }
        return branchRepository.findAll(specification(organizationId, branchIds, status, q), pageable);
    }

    private static Specification<Branch> specification(
            UUID organizationId, Collection<UUID> branchIds, BranchStatus status, String q) {
        return (root, query, cb) -> {
            List<Predicate> predicates = new ArrayList<>();
            predicates.add(cb.equal(root.get("organization").get("id"), organizationId));
            if (branchIds != null) {
                predicates.add(root.get("id").in(branchIds));
            }
            if (status != null) {
                predicates.add(cb.equal(root.get("status"), status));
            }
            if (q != null && !q.isBlank()) {
                String trimmed = q.trim().toLowerCase(Locale.ROOT);
                predicates.add(cb.or(
                        cb.like(cb.lower(root.get("name")), "%" + trimmed + "%"),
                        cb.like(cb.lower(root.get("code")), "%" + trimmed + "%")));
            }
            return cb.and(predicates.toArray(new Predicate[0]));
        };
    }

    private static BusinessException invalidTokenIdentity() {
        return new BusinessException(
                HttpStatus.UNAUTHORIZED, ErrorCodes.TOKEN_INVALID, "Invalid token identity");
    }
}
