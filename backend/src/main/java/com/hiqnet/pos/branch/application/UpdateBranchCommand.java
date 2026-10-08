package com.hiqnet.pos.branch.application;

import com.hiqnet.pos.branch.domain.BranchStatus;
import com.hiqnet.pos.shared.api.PatchField;

/**
 * Partial update payload. Null fields are left unchanged; code is immutable.
 * {@code timezone} uses {@link PatchField}: null means "not present" (no
 * change), a present null value clears the override, a present value sets it.
 */
public record UpdateBranchCommand(long expectedVersion, String name, PatchField timezone, BranchStatus status) {
}