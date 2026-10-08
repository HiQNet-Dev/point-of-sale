package com.hiqnet.pos.membership.api;

import com.hiqnet.pos.membership.domain.BranchMembershipStatus;
import io.swagger.v3.oas.annotations.media.Schema;
import jakarta.validation.constraints.Min;
import jakarta.validation.constraints.NotNull;

/**
 * Partial update of a membership. {@code expectedVersion} is required (the
 * version the client read); {@code status} is the only editable field. There is
 * no DELETE: setting {@code status = INACTIVE} is the way «down», and it keeps
 * the configuration (branch link and role assignments) while granting nothing.
 */
public record UpdateMembershipRequest(
        @Schema(description = "Version the client read; must match the current version.", example = "2")
        @NotNull
                @Min(0)
                Long expectedVersion,
        BranchMembershipStatus status) {}