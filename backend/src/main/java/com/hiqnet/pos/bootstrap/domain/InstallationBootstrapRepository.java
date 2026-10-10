package com.hiqnet.pos.bootstrap.domain;

import jakarta.persistence.LockModeType;
import java.util.Optional;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.data.jpa.repository.Lock;
import org.springframework.data.jpa.repository.Query;
import org.springframework.data.repository.query.Param;

/**
 * Access to the singleton installation-bootstrap row. The only mutation path is
 * {@link #findByIdForUpdate} (a {@code SELECT ... FOR UPDATE} pessimistic write
 * lock): concurrent bootstrap attempts serialize on the row, so exactly one
 * wins and the rest observe COMPLETED and fail. Postgres is the coordinator —
 * there is no JVM lock.
 */
public interface InstallationBootstrapRepository extends JpaRepository<InstallationBootstrap, Short> {

    @Lock(LockModeType.PESSIMISTIC_WRITE)
    @Query("select b from InstallationBootstrap b where b.id = :id")
    Optional<InstallationBootstrap> findByIdForUpdate(@Param("id") Short id);
}
