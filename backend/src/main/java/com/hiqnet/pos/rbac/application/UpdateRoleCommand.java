package com.hiqnet.pos.rbac.application;

import com.hiqnet.pos.rbac.domain.RoleStatus;

/** Partial update payload. Null fields are left unchanged; code is immutable. */
public record UpdateRoleCommand(long expectedVersion, String name, RoleStatus status) {
}