package com.hiqnet.pos.membership.infrastructure;

import com.hiqnet.pos.membership.domain.BranchMembershipRoleRepository;
import com.hiqnet.pos.rbac.application.BranchMembershipAccessPort;
import java.util.UUID;
import org.springframework.stereotype.Component;

/**
 * Runtime adapter of {@link BranchMembershipAccessPort}: evaluates the
 * branch-scoped authorization predicate against PostgreSQL (single EXISTS in
 * {@link BranchMembershipRoleRepository}). Keeping the implementation in the
 * membership module preserves the compile direction {@code membership -> rbac}
 * (no {@code rbac -> membership} package cycle).
 */
@Component
public class BranchMembershipAccessAdapter implements BranchMembershipAccessPort {

    private final BranchMembershipRoleRepository branchMembershipRoleRepository;

    public BranchMembershipAccessAdapter(BranchMembershipRoleRepository branchMembershipRoleRepository) {
        this.branchMembershipRoleRepository = branchMembershipRoleRepository;
    }

    @Override
    public boolean hasEffectiveBranchPermission(
            UUID userId, UUID organizationId, UUID branchId, String permissionCode) {
        return branchMembershipRoleRepository.existsEffectiveBranchPermission(
                userId, organizationId, branchId, permissionCode);
    }
}