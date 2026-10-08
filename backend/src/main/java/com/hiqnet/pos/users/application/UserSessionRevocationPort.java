package com.hiqnet.pos.users.application;

import java.util.UUID;

/**
 * Application-level port to revoke a user's persisted sessions when their
 * identity stops being operational (deactivation).
 *
 * <p>Lives in the users application layer so {@code users} never depends on
 * {@code auth} (no module cycle); the authentication module provides the
 * implementation behind this interface (dependency inversion).</p>
 *
 * <p>Implementations MUST join the calling transaction (no {@code REQUIRES_NEW}),
 * so deactivation + revocation commit atomically, and MUST respect the lock
 * ordering established by Authentication (user row -&gt; sessions): the caller
 * already holds the user row lock.</p>
 */
public interface UserSessionRevocationPort {

    /**
     * Marks every still-active refresh session of the user as revoked.
     *
     * @return the number of sessions revoked (0 when there were none)
     */
    int revokeAllForUser(UUID userId);
}