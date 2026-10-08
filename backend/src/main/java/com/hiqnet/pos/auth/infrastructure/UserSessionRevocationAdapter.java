package com.hiqnet.pos.auth.infrastructure;

import com.hiqnet.pos.auth.domain.RefreshSessionRepository;
import com.hiqnet.pos.users.application.UserSessionRevocationPort;
import java.time.Clock;
import java.util.UUID;
import org.springframework.stereotype.Component;

/**
 * Auth-side adapter for {@link UserSessionRevocationPort}: revokes a user's
 * active refresh sessions. It runs inside the caller's transaction (no
 * {@code REQUIRES_NEW}), so deactivation + revocation commit or rollback
 * together. The caller (users application) already holds the user row lock,
 * preserving the fixed lock order user -&gt; sessions established by
 * Authentication.
 */
@Component
public class UserSessionRevocationAdapter implements UserSessionRevocationPort {

    private final RefreshSessionRepository refreshSessionRepository;
    private final Clock clock;

    public UserSessionRevocationAdapter(
            RefreshSessionRepository refreshSessionRepository, Clock clock) {
        this.refreshSessionRepository = refreshSessionRepository;
        this.clock = clock;
    }

    @Override
    public int revokeAllForUser(UUID userId) {
        return refreshSessionRepository.revokeActiveByUserId(userId, clock.instant());
    }
}