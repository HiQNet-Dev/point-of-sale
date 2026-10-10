package com.hiqnet.pos.bootstrap.application;

/**
 * Provisioning facts extracted from the bootstrap request. No identity fields,
 * no role/permission/status fields: the client only decides the organization
 * shape and the administrator email/password; everything else is created by the
 * service with fixed, safe semantics (organization/user/role ACTIVE, role
 * {@code administrator} granted EVERY current catalog permission).
 */
public record BootstrapCommand(
        String organizationName,
        String organizationCode,
        String timezone,
        String currency,
        String locale,
        String administratorEmail,
        String administratorPassword) {
}