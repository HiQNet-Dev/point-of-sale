package com.hiqnet.pos.auth.domain;

import static org.assertj.core.api.Assertions.assertThat;

import java.time.Duration;
import java.time.Instant;
import java.util.UUID;
import org.junit.jupiter.api.Test;

/**
 * Pure domain check of {@code usableUntil()} (the value exposed as
 * {@code refreshExpiresAt}): it must be the earlier of the session TTL and the
 * absolute family TTL, and rotation must never extend it.
 */
class RefreshSessionUsableUntilTest {

    private static final Instant NOW = Instant.parse("2026-01-01T00:00:00Z");

    @Test
    void usableUntilIsSessionExpiryWhenSessionTtlIsShorterThanFamily() {
        RefreshSession session =
                RefreshSession.create(UUID.randomUUID(), "hash", NOW, Duration.ofHours(1), Duration.ofDays(30));

        assertThat(session.usableUntil()).isEqualTo(NOW.plus(Duration.ofHours(1)));
    }

    @Test
    void usableUntilIsFamilyExpiryWhenFamilyTtlIsShorterThanSession() {
        RefreshSession session =
                RefreshSession.create(UUID.randomUUID(), "hash", NOW, Duration.ofDays(30), Duration.ofHours(2));

        assertThat(session.usableUntil()).isEqualTo(NOW.plus(Duration.ofHours(2)));
    }

    @Test
    void rotationInheritsFamilyExpirySoUsableUntilNeverExtendsTheFamily() {
        RefreshSession session =
                RefreshSession.create(UUID.randomUUID(), "hash", NOW, Duration.ofDays(30), Duration.ofHours(2));

        RefreshSession next = session.rotateTo("hash-2", NOW.plus(Duration.ofMinutes(30)), Duration.ofDays(30));

        assertThat(next.usableUntil()).isEqualTo(NOW.plus(Duration.ofHours(2)));
        assertThat(session.isReusable(NOW.plus(Duration.ofMinutes(1)))).isFalse();
    }
}
