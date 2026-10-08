package com.hiqnet.pos.auth.domain;

import jakarta.persistence.LockModeType;
import java.time.Instant;
import java.util.List;
import java.util.Optional;
import java.util.UUID;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.data.jpa.repository.Lock;
import org.springframework.data.jpa.repository.Modifying;
import org.springframework.data.jpa.repository.Query;
import org.springframework.data.repository.query.Param;

/**
 * Lock ordering is fixed and enforced by {@code AuthService}: the owning user
 * row is acquired first (see {@code UserRepository.findByIdForUpdate}) and only
 * then any row here. Session locking always happens after the user lock, never
 * before, which is what keeps the mutation paths (refresh, logout,
 * logout-all, family revocation) deadlock-free while still serialized per user.
 */
public interface RefreshSessionRepository extends JpaRepository<RefreshSession, UUID> {

    /**
     * Non-locking owner lookup: resolves the user to lock BEFORE any session
     * row is locked (scalar projection, no entity is loaded into the
     * persistence context).
     */
    @Query("select s.userId from RefreshSession s where s.tokenHash = :tokenHash")
    Optional<UUID> findUserIdByTokenHash(@Param("tokenHash") String tokenHash);

    /**
     * Locking lookup used by refresh: after the user lock is held, concurrent
     * attempts on the same token serialize on the DB row (SELECT ... FOR
     * UPDATE), so rotation is atomic and the second attempt deterministically
     * observes the rotated state.
     */
    @Lock(LockModeType.PESSIMISTIC_WRITE)
    @Query("select s from RefreshSession s where s.tokenHash = :tokenHash")
    Optional<RefreshSession> findByTokenHashForUpdate(@Param("tokenHash") String tokenHash);

    Optional<RefreshSession> findByTokenHash(String tokenHash);

    List<RefreshSession> findByFamilyId(UUID familyId);

    List<RefreshSession> findByUserId(UUID userId);

    @Modifying
    @Query("update RefreshSession s set s.revokedAt = :now "
            + "where s.userId = :userId and s.revokedAt is null and s.rotatedAt is null")
    int revokeActiveByUserId(@Param("userId") UUID userId, @Param("now") Instant now);

    @Modifying
    @Query("update RefreshSession s set s.revokedAt = :now "
            + "where s.familyId = :familyId and s.revokedAt is null")
    int revokeFamily(@Param("familyId") UUID familyId, @Param("now") Instant now);
}