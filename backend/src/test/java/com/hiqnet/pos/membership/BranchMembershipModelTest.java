package com.hiqnet.pos.membership;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;

import com.hiqnet.pos.membership.domain.BranchMembership;
import com.hiqnet.pos.membership.domain.BranchMembershipStatus;
import com.hiqnet.pos.shared.error.BusinessException;
import java.util.UUID;
import org.junit.jupiter.api.Test;

class BranchMembershipModelTest {

    private final UUID orgId = UUID.randomUUID();
    private final UUID branchId = UUID.randomUUID();
    private final UUID userId = UUID.randomUUID();

    @Test
    void createReturnsActiveMembershipWithVersionZero() {
        BranchMembership membership = BranchMembership.create(orgId, branchId, userId);
        assertThat(membership.getId()).isNotNull();
        assertThat(membership.getOrganizationId()).isEqualTo(orgId);
        assertThat(membership.getBranchId()).isEqualTo(branchId);
        assertThat(membership.getUserId()).isEqualTo(userId);
        assertThat(membership.getStatus()).isEqualTo(BranchMembershipStatus.ACTIVE);
        assertThat(membership.getVersion()).isZero();
    }

    @Test
    void createRejectsNullIds() {
        assertThatThrownBy(() -> BranchMembership.create(null, branchId, userId))
                .isInstanceOf(BusinessException.class);
        assertThatThrownBy(() -> BranchMembership.create(orgId, null, userId))
                .isInstanceOf(BusinessException.class);
        assertThatThrownBy(() -> BranchMembership.create(orgId, branchId, null))
                .isInstanceOf(BusinessException.class);
    }

    @Test
    void statusTogglesPreserveConfiguration() {
        BranchMembership membership = BranchMembership.create(orgId, branchId, userId);
        membership.updateStatus(BranchMembershipStatus.INACTIVE);
        assertThat(membership.getStatus()).isEqualTo(BranchMembershipStatus.INACTIVE);
        membership.updateStatus(BranchMembershipStatus.ACTIVE);
        assertThat(membership.getStatus()).isEqualTo(BranchMembershipStatus.ACTIVE);
        assertThat(membership.getBranchId()).isEqualTo(branchId);
        assertThat(membership.getUserId()).isEqualTo(userId);
    }

    @Test
    void updateStatusRejectsNull() {
        BranchMembership membership = BranchMembership.create(orgId, branchId, userId);
        assertThatThrownBy(() -> membership.updateStatus(null)).isInstanceOf(BusinessException.class);
    }
}