package com.hiqnet.pos.inventory.stock.infrastructure;

import com.hiqnet.pos.inventory.stock.domain.InventoryMovementRepository;
import com.hiqnet.pos.inventory.stock.domain.InventoryMovementRow;
import com.hiqnet.pos.inventory.stock.domain.InventoryMovementType;
import java.math.BigDecimal;
import java.sql.ResultSet;
import java.sql.SQLException;
import java.util.UUID;
import org.springframework.data.domain.Page;
import org.springframework.data.domain.PageImpl;
import org.springframework.data.domain.Pageable;
import org.springframework.jdbc.core.JdbcTemplate;
import org.springframework.stereotype.Repository;

@Repository
public class JdbcInventoryMovementRepository implements InventoryMovementRepository {

    private final JdbcTemplate jdbcTemplate;

    public JdbcInventoryMovementRepository(JdbcTemplate jdbcTemplate) {
        this.jdbcTemplate = jdbcTemplate;
    }

    @Override
    public void insert(
            UUID id,
            UUID organizationId,
            UUID branchId,
            UUID inventoryItemId,
            InventoryMovementType type,
            BigDecimal delta,
            BigDecimal resultingQuantity,
            String reason,
            UUID actorUserId) {
        jdbcTemplate.update(
                "insert into inventory_movements "
                        + "(id,organization_id,branch_id,inventory_item_id,type,quantity_delta,resulting_quantity,reason,actor_user_id,occurred_at) "
                        + "values (?,?,?,?,?,?,?,?,?,now())",
                id,
                organizationId,
                branchId,
                inventoryItemId,
                type.name(),
                delta,
                resultingQuantity,
                reason,
                actorUserId);
    }

    @Override
    public boolean existsForBranchItem(UUID organizationId, UUID branchId, UUID inventoryItemId) {
        Boolean exists = jdbcTemplate.queryForObject(
                "select exists(select 1 from inventory_movements "
                        + "where organization_id=? and branch_id=? and inventory_item_id=?)",
                Boolean.class,
                organizationId,
                branchId,
                inventoryItemId);
        return Boolean.TRUE.equals(exists);
    }

    @Override
    public Page<InventoryMovementRow> findHistory(
            UUID organizationId, UUID branchId, UUID inventoryItemId, Pageable pageable) {
        var rows = jdbcTemplate.query(
                "select id,type,quantity_delta,resulting_quantity,reason,actor_user_id,occurred_at "
                        + "from inventory_movements where organization_id=? and branch_id=? and inventory_item_id=? "
                        + "order by occurred_at desc,id desc limit ? offset ?",
                (resultSet, rowNumber) -> map(resultSet),
                organizationId,
                branchId,
                inventoryItemId,
                pageable.getPageSize(),
                pageable.getOffset());
        Long total = jdbcTemplate.queryForObject(
                "select count(*) from inventory_movements "
                        + "where organization_id=? and branch_id=? and inventory_item_id=?",
                Long.class,
                organizationId,
                branchId,
                inventoryItemId);
        return new PageImpl<>(rows, pageable, total == null ? 0 : total);
    }

    private static InventoryMovementRow map(ResultSet resultSet) throws SQLException {
        return new InventoryMovementRow(
                resultSet.getObject("id", UUID.class),
                InventoryMovementType.valueOf(resultSet.getString("type")),
                resultSet.getBigDecimal("quantity_delta"),
                resultSet.getBigDecimal("resulting_quantity"),
                resultSet.getString("reason"),
                resultSet.getObject("actor_user_id", UUID.class),
                resultSet.getTimestamp("occurred_at").toInstant());
    }
}
