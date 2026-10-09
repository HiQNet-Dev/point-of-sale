package com.hiqnet.pos.membership.api;

import io.swagger.v3.oas.annotations.media.Schema;
import jakarta.validation.constraints.NotNull;
import java.util.UUID;

/**
 * Creates a membership: the user joins the branch. Organization, branch and
 * user must exist in the same organization and be ACTIVE (the service enforces
 * it); a duplicate (organization, branch, user) is rejected with 409.
 */
public record CreateMembershipRequest(
        @Schema(description = "User to link to the branch (must belong to the organization)", example = "709d4cf3-…")
        @NotNull
                UUID userId) {}