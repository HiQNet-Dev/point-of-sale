package com.hiqnet.pos.inventory.modifier.domain;

import jakarta.persistence.LockModeType;
import java.util.Optional;
import java.util.UUID;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.data.jpa.repository.Lock;
import org.springframework.data.jpa.repository.Query;
import org.springframework.data.repository.query.Param;

public interface ModifierOptionInventoryConfigRepository extends JpaRepository<ModifierOptionInventoryConfig, UUID> {
    Optional<ModifierOptionInventoryConfig> findByOrganizationIdAndModifierOptionId(UUID organizationId, UUID modifierOptionId);

    @Lock(LockModeType.PESSIMISTIC_WRITE)
    @Query("select c from ModifierOptionInventoryConfig c where c.organizationId = :organizationId and c.modifierOptionId = :optionId")
    Optional<ModifierOptionInventoryConfig> findByOrganizationIdAndModifierOptionIdForUpdate(
            @Param("organizationId") UUID organizationId, @Param("optionId") UUID modifierOptionId);
}
