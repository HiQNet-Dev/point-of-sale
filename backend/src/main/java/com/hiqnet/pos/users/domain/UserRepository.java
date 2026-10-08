package com.hiqnet.pos.users.domain;

import jakarta.persistence.LockModeType;
import java.util.Optional;
import java.util.UUID;
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

    @Query("""
            select count(u) > 0 from User u
            where u.organizationId = :organizationId and lower(u.email) = lower(:email)
            """)
    boolean existsByOrganizationIdAndEmail(
            @Param("organizationId") UUID organizationId, @Param("email") String email);
}