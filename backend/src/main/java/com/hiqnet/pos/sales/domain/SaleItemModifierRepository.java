package com.hiqnet.pos.sales.domain;

import java.util.Collection;
import java.util.List;
import java.util.UUID;
import org.springframework.data.jpa.repository.JpaRepository;

public interface SaleItemModifierRepository extends JpaRepository<SaleItemModifier, UUID> {
    List<SaleItemModifier> findBySaleItemIdOrderByDisplayOrderAsc(UUID saleItemId);
    List<SaleItemModifier> findBySaleItemIdInOrderBySaleItemIdAscDisplayOrderAsc(Collection<UUID> saleItemIds);
    void deleteBySaleItemId(UUID saleItemId);
}
