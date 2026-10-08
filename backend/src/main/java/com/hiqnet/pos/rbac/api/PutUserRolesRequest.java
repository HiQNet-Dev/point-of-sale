package com.hiqnet.pos.rbac.api;

import io.swagger.v3.oas.annotations.media.Schema;
import jakarta.validation.constraints.NotNull;
import java.util.List;
import java.util.UUID;

/** Full replacement of the user's organization role set (atomic; duplicates rejected). */
public record PutUserRolesRequest(
        @Schema(description = "Role ids to assign (same organization).", example = "[\"3f2b...\",\"7a1c...\"]")
        @NotNull
                List<@NotNull UUID> roleIds) {
}