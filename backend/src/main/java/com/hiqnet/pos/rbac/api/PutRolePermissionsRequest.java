package com.hiqnet.pos.rbac.api;

import io.swagger.v3.oas.annotations.media.Schema;
import jakarta.validation.constraints.NotBlank;
import jakarta.validation.constraints.NotNull;
import jakarta.validation.constraints.Size;
import java.util.List;

/** Full replacement of the role's permission set (atomic; duplicates rejected). */
public record PutRolePermissionsRequest(
        @Schema(description = "Exact permission codes; the set replaces the current one.", example = "[\"user.read\",\"branch.read\"]")
        @NotNull
                List<@NotBlank @Size(max = 64) String> permissions) {
}