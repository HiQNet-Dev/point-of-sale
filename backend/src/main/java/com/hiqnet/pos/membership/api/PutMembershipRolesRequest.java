package com.hiqnet.pos.membership.api;

import io.swagger.v3.oas.annotations.media.Schema;
import jakarta.validation.constraints.NotNull;
import java.util.List;
import java.util.UUID;

/**
 * Atomically replaces the full set of branch-scoped roles of a membership.
 * Duplicate ids are rejected (400); an unknown or cross-organization role
 * aborts the whole replacement (404, previous set untouched). The role
 * definitions come from the organization-scoped role catalog (no separate
 * BranchRole model).
 */
public record PutMembershipRolesRequest(
        @Schema(description = "Complete role set of the membership; empty array clears it", example = "[\"…\", \"…\"]")
        @NotNull
                List<UUID> roleIds) {}