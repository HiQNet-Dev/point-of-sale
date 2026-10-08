package com.hiqnet.pos.users.api;

import com.hiqnet.pos.users.domain.UserStatus;
import io.swagger.v3.oas.annotations.media.Schema;
import jakarta.validation.constraints.Min;
import jakarta.validation.constraints.NotNull;

/**
 * Partial update: only {@code email} and {@code status} are editable and both
 * are optional (absent = no change). Immutable fields are not exposed, so a
 * user can never be moved between organizations or have its password set here.
 */
public record UpdateUserRequest(
        @Schema(description = "Version the client read; must match the current version.", example = "3")
        @NotNull
                @Min(0)
                Long expectedVersion,
        @Schema(description = "New email; normalized to trim+lowercase.", example = "new-email@example.com")
        String email,
        @Schema(description = "ACTIVE to reactivate, INACTIVE to deactivate.", example = "INACTIVE")
        UserStatus status) {
}
