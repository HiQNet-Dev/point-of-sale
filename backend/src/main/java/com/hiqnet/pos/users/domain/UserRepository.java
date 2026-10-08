package com.hiqnet.pos.users.domain;

import jakarta.persistence.LockModeType;
import java.util.Optional;
import java.util.UUID;
import org.springframework.data.domain.Page;
import org.springframework.data.domain.Pageable;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.data.jpa.repository.Lock;
import org.springframework.data.jpa.repository.Query;
import org.springframework.data.repository.query.Param;

public interface UserRepository extends JpaRepository<User, UUID> {

    /**
     * Pessimistic lock on the user row during login: concurrent failed
     * attempts against the same account serialize, so the lockout counter
     * cannot be bypassed with N parallel guesses (each would otherwise
     * increment an optimistic version and only one commit would survive).
     */
    @Query("""
            select u from User u
            where u.organizationId = :organizationId and lower(u.email) = lower(:email)
            """)
    @Lock(LockModeType.PESSIMISTIC_WRITE)
    Optional<User> findByOrganizationIdAndEmail(
            @Param("organizationId") UUID organizationId, @Param("email") String email);

    /**
     * Per-user coordination lock for authentication mutations (refresh,
     * logout, logout-all, family revocation). Postgres is the authority: the
     * user row serializes every session mutation of the same user across
     * application instances, so a concurrent rotation can never commit a live
     * successor behind a revocation.
     *
     * <p>Lock ordering is fixed: the user row is ALWAYS acquired before any
     * {@code refresh_sessions} row, never the other way around (this is what
     * prevents deadlocks between the different mutation paths).</p>
     */
    @Lock(LockModeType.PESSIMISTIC_WRITE)
    @Query("select u from User u where u.id = :id")
    Optional<User> findByIdForUpdate(@Param("id") UUID id);

    @Query("""
            select count(u) > 0 from User u
            where u.organizationId = :organizationId and lower(u.email) = lower(:email)
            """)
    boolean existsByOrganizationIdAndEmail(
            @Param("organizationId") UUID organizationId, @Param("email") String email);

    /**
     * Scoped administrative lookup: a user is never resolved by a global id in
     * the administration API (cross-organization requests must get 404, not
     * 403). Mirrors the {@code branch} isolation pattern.
     */
    Optional<User> findByIdAndOrganizationId(UUID id, UUID organizationId);

    /**
     * Scoped locking lookup for administrative mutations (deactivation):
     * acquires the user row first (PESSIMISTIC_WRITE) so the subsequent
     * refresh-session revocation keeps the established order user -> sessions.
     * Organization scoping (IDOR) is preserved.
     */
    @Lock(LockModeType.PESSIMISTIC_WRITE)
    @Query("select u from User u where u.id = :id and u.organizationId = :organizationId")
    Optional<User> findByIdAndOrganizationIdForUpdate(
            @Param("id") UUID id, @Param("organizationId") UUID organizationId);

    Page<User> findByOrganizationId(UUID organizationId, Pageable pageable);

    Page<User> findByOrganizationIdAndStatus(UUID organizationId, UserStatus status, Pageable pageable);
}