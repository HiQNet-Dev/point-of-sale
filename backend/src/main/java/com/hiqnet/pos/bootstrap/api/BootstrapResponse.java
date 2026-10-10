package com.hiqnet.pos.bootstrap.api;

import com.hiqnet.pos.bootstrap.application.BootstrapResult;
import java.util.UUID;

/**
 * Bootstrap response: ONLY identity ids (plus the organization code for UX).
 * It never contains the password, the password hash, the bootstrap token,
 * access/refresh tokens or the granted permissions — provisioning is separate
 * from authentication ({@code POST /auth/login} is the only way into the app).
 */
public record BootstrapResponse(
        UUID organizationId,
        String organizationCode,
        UUID administratorUserId,
        UUID administratorRoleId) {

    public static BootstrapResponse from(BootstrapResult result) {
        return new BootstrapResponse(
                result.organizationId(),
                result.organizationCode(),
                result.administratorUserId(),
                result.administratorRoleId());
    }
}