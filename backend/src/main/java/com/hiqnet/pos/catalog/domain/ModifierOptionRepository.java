package com.hiqnet.pos.catalog.domain;

import jakarta.persistence.LockModeType;
import java.util.Collection;
import java.util.List;
import java.util.Optional;
import java.util.UUID;
import org.springframework.data.domain.Page;
import org.springframework.data.domain.Pageable;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.data.jpa.repository.Lock;
import org.springframework.data.jpa.repository.Query;
import org.springframework.data.repository.query.Param;

public interface ModifierOptionRepository extends JpaRepository<ModifierOption, UUID> {

    /** Scoped lookup: prevents accidental cross-organization access (IDOR). */
    Optional<ModifierOption> findByIdAndOrganizationId(UUID id, UUID organizationId);

    /** Scoped locking lookup for administrative mutations (expectedVersion). */
    @Lock(LockModeType.PESSIMISTIC_WRITE)
    @Query("select o from ModifierOption o where o.id = :id and o.organization.id = :organizationId")
    Optional<ModifierOption> findByIdAndOrganizationIdForUpdate(
            @Param("id") UUID id, @Param("organizationId") UUID organizationId);

    Page<ModifierOption> findByOrganizationIdAndModifierGroupId(
            UUID organizationId, UUID modifierGroupId, Pageable pageable);

    boolean existsByModifierGroupIdAndCode(UUID modifierGroupId, String code);

    /** Bulk same-organization lookup used to construct server-authoritative sale snapshots. */
    List<ModifierOption> findByOrganizationIdAndIdIn(UUID organizationId, Collection<UUID> ids);

    /** Load all options for a product configuration without one query per group. */
    @Query("select o from ModifierOption o join fetch o.modifierGroup "
            + "where o.organization.id = :organizationId and o.modifierGroup.id in :groupIds "
            + "order by o.displayOrder asc, o.id asc")
    List<ModifierOption> findAllForGroups(
            @Param("organizationId") UUID organizationId, @Param("groupIds") Collection<UUID> groupIds);
}
