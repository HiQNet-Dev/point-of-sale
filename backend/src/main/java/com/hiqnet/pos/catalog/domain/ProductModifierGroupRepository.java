package com.hiqnet.pos.catalog.domain;

import java.util.List;
import java.util.UUID;
import org.springframework.data.jpa.repository.JpaRepository;

public interface ProductModifierGroupRepository extends JpaRepository<ProductModifierGroup, ProductModifierGroupId> {

    /** Ordered assignment list of a product, scoped by organization. */
    List<ProductModifierGroup> findByProductIdAndOrganizationIdOrderByDisplayOrderAsc(
            UUID productId, UUID organizationId);

    void deleteByProductIdAndOrganizationId(UUID productId, UUID organizationId);
}
