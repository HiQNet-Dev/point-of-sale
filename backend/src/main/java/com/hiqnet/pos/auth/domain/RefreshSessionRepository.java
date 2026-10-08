package com.hiqnet.pos.auth.domain;

import jakarta.persistence.LockModeType;
import java.time.Instant;
import java.util.Optional;
import java.util.UUID;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.data.jpa.repository.Lock;
import org.springframework.data.jpa.repository.Modifying;
import org.springframework.data.jpa.repository.Query;
import org.springframework.data.repository.query.Param;

public interface RefreshSessionRepository extends JpaRepository<RefreshSession, UUID> {

    /**
     * Locking lookup used by refresh: concurrent attempts on the same token
     * serialize on the DB row (SELECT ... FOR UPDATE), so rotation is atomic
     * and the second attempt deterministically observes the rotated state.
     */
    @Lock(LockModeType.PESSIMISTIC_WRITE)
    @Query("select s from RefreshSession s where s.tokenHash = :tokenHash")
    Optional<RefreshSession> findByTokenHashForUpdate(@Param("tokenHash") String tokenHash);

    Optional<RefreshSession> findByTokenHash(String tokenHash);

    @Modifying
    @Query("update RefreshSession s set s.revokedAt = :now "
            + "where s.userId = :userId and s.revokedAt is null and s.rotatedAt is null")
    int revokeActiveByUserId(@Param("userId") UUID userId, @Param("now") Instant now);

    @Modifying
    @Query("update RefreshSession s set s.revokedAt = :now "
            + "where s.familyId = :familyId and s.revokedAt is null")
    int revokeFamily(@Param("familyId") UUID familyId, @Param("now") Instant now);
}