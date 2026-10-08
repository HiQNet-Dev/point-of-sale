package com.hiqnet.pos.users.domain;

/**
 * Lifecycle status of a user. Temporal lockout is NOT a status: it is
 * represented by {@code locked_until}; INACTIVE is an administrative
 * deactivation (future users module) that blocks login immediately.
 */
public enum UserStatus {
    ACTIVE,
    INACTIVE
}