package com.hiqnet.pos.branch.api;

import com.hiqnet.pos.branch.domain.BranchStatus;
import jakarta.validation.constraints.Size;

/** Partial update: only fields present are applied; {@code code} is immutable. */
public record UpdateBranchRequest(
        @Size(max = 120) String name,
        @Size(max = 64) String timezone,
        BranchStatus status) {
}