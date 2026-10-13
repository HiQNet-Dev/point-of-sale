package com.hiqnet.pos.inventory.stock.domain;

import jakarta.persistence.LockModeType;
import java.util.Collection;
import java.util.List;
import java.util.Optional;
import java.util.UUID;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.data.jpa.repository.Lock;
import org.springframework.data.jpa.repository.Modifying;
import org.springframework.data.jpa.repository.Query;
import org.springframework.data.repository.query.Param;

public interface BranchInventoryRepository extends JpaRepository<BranchInventory, UUID> {

    @Modifying
    @Query(
            value = "insert into branch_inventory "
                    + "(id,organization_id,branch_id,inventory_item_id,quantity_on_hand,version,created_at,updated_at) "
                    + "values (:id,:org,:branch,:item,0,0,now(),now()) "
                    + "on conflict (branch_id,inventory_item_id) do nothing",
            nativeQuery = true)
    int ensureBalanceRow(
            @Param("id") UUID id,
            @Param("org") UUID organizationId,
            @Param("branch") UUID branchId,
            @Param("item") UUID inventoryItemId);

    @Lock(LockModeType.PESSIMISTIC_WRITE)
    @Query("select b from BranchInventory b "
            + "where b.organizationId = :org and b.branchId = :branch and b.inventoryItemId = :item")
    Optional<BranchInventory> lockBalance(
            @Param("org") UUID organizationId,
            @Param("branch") UUID branchId,
            @Param("item") UUID inventoryItemId);

    @Modifying
    @Query(
            value = "insert into branch_inventory "
                    + "(id,organization_id,branch_id,inventory_item_id,quantity_on_hand,version,created_at,updated_at) "
                    + "select gen_random_uuid(),:org,:branch,item_id,0,0,now(),now() "
                    + "from unnest(string_to_array(:ids,',')::uuid[]) as item_id "
                    + "on conflict (branch_id,inventory_item_id) do nothing",
            nativeQuery = true)
    int ensureBalances(
            @Param("org") UUID organizationId,
            @Param("branch") UUID branchId,
            @Param("ids") String inventoryItemIds);

    @Lock(LockModeType.PESSIMISTIC_WRITE)
    @Query("select b from BranchInventory b "
            + "where b.organizationId = :org and b.branchId = :branch and b.inventoryItemId in :items "
            + "order by b.inventoryItemId")
    List<BranchInventory> lockBalances(
            @Param("org") UUID organizationId,
            @Param("branch") UUID branchId,
            @Param("items") Collection<UUID> itemIds);

    List<BranchInventory> findAllByBranchIdAndInventoryItemIdIn(UUID branchId, Collection<UUID> itemIds);

    Optional<BranchInventory> findByOrganizationIdAndBranchIdAndInventoryItemId(
            UUID organizationId, UUID branchId, UUID inventoryItemId);
}
