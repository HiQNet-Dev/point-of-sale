package com.hiqnet.pos.branch.api;

import com.hiqnet.pos.branch.domain.Branch;
import com.hiqnet.pos.branch.domain.BranchStatus;
import java.time.Instant;
import java.util.UUID;

public record BranchResponse(
        UUID id,
        UUID organizationId,
        String name,
        String code,
        String timezone,
        BranchStatus status,
        long version,
        Instant createdAt,
        Instant updatedAt) {

    public static BranchResponse from(Branch branch) {
        return new BranchResponse(
                branch.getId(),
                branch.getOrganization().getId(),
                branch.getName(),
                branch.getCode(),
                branch.getTimezone(),
                branch.getStatus(),
                branch.getVersion(),
                branch.getCreatedAt(),
                branch.getUpdatedAt());
    }
}