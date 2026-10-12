package com.hiqnet.pos.inventory.recipe.domain;

import jakarta.persistence.LockModeType;
import java.util.Optional;
import java.util.UUID;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.data.jpa.repository.Lock;
import org.springframework.data.jpa.repository.Query;
import org.springframework.data.repository.query.Param;

public interface ProductInventoryRecipeRepository extends JpaRepository<ProductInventoryRecipe, UUID> {
    Optional<ProductInventoryRecipe> findByOrganizationIdAndProductId(UUID organizationId, UUID productId);

    @Lock(LockModeType.PESSIMISTIC_WRITE)
    @Query("select r from ProductInventoryRecipe r where r.organizationId = :organizationId and r.productId = :productId")
    Optional<ProductInventoryRecipe> findByOrganizationIdAndProductIdForUpdate(
            @Param("organizationId") UUID organizationId, @Param("productId") UUID productId);
}
