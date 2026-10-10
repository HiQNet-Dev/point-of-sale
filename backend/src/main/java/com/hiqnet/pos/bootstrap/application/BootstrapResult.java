package com.hiqnet.pos.bootstrap.application;

import java.util.UUID;

/**
 * Outcome of a successful provisioning. Contains ONLY identities (never the
 * password, the password hash, permissions, the bootstrap token or any
 * session/token material).
 */
public record BootstrapResult(
        UUID organizationId, String organizationCode, UUID administratorUserId, UUID administratorRoleId) {
}