package com.hiqnet.pos.rbac.domain;

/**
 * Lifecycle of a role. INACTIVE grants nothing while kept in the database:
 * assignments survive and re-grant as soon as the role returns to ACTIVE.
 */
public enum RoleStatus {
    ACTIVE,
    INACTIVE
}