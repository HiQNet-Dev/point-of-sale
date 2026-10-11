package com.hiqnet.pos.sales.domain;

import java.util.List;
import java.util.Optional;
import java.util.UUID;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.data.jpa.repository.Query;
import org.springframework.data.repository.query.Param;

public interface SaleItemRepository extends JpaRepository<SaleItem, UUID> {
    List<SaleItem> findBySaleIdOrderByPositionAsc(UUID saleId);
    List<SaleItem> findBySaleIdAndStatusOrderByPositionAsc(UUID saleId, SaleItemStatus status);
    Optional<SaleItem> findByIdAndSaleId(UUID id, UUID saleId);
    @Query("select coalesce(max(i.position), -1) from SaleItem i where i.saleId = :saleId")
    int findMaxPositionBySaleId(@Param("saleId") UUID saleId);
}
