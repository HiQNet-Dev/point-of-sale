package com.hiqnet.pos.branch.application;

import com.hiqnet.pos.branch.domain.BranchStatus;

/** Partial update payload. Null fields are left unchanged; code is immutable. */
public record UpdateBranchCommand(String name, String timezone, BranchStatus status) {
}