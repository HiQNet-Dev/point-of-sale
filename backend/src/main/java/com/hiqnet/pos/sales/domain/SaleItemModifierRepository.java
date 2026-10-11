package com.hiqnet.pos.sales.domain;

import java.util.List;
import java.util.UUID;
import org.springframework.data.jpa.repository.JpaRepository;

public interface SaleItemModifierRepository extends JpaRepository<SaleItemModifier, UUID> {
    List<SaleItemModifier> findBySaleItemIdOrderByDisplayOrderAsc(UUID saleItemId);
    void deleteBySaleItemId(UUID saleItemId);
}
