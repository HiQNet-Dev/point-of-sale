package com.hiqnet.pos.inventory.stock;
import static org.assertj.core.api.Assertions.assertThatThrownBy;
import com.hiqnet.pos.AbstractRbacIntegrationTest;
import com.hiqnet.pos.branch.domain.Branch;
import com.hiqnet.pos.branch.domain.BranchRepository;
import com.hiqnet.pos.inventory.application.InventoryItemService;
import com.hiqnet.pos.inventory.domain.InventoryBaseUnit;
import com.hiqnet.pos.inventory.stock.application.InventoryStockService;
import com.hiqnet.pos.organization.domain.OrganizationRepository;
import java.util.UUID;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.dao.DataIntegrityViolationException;
import org.springframework.jdbc.core.JdbcTemplate;

class InventoryMovementAppendOnlyTest extends AbstractRbacIntegrationTest {
    @Autowired BranchRepository branches;
    @Autowired OrganizationRepository organizations;
    @Autowired InventoryItemService items;
    @Autowired InventoryStockService stock;
    @Autowired JdbcTemplate jdbc;
    @Test void databaseRejectsCrossOrganizationBranchAndItemCombination() {
        var a=fixture(); var b=fixture();
        var branch=branches.saveAndFlush(Branch.create(organizations.findById(a.orgId()).orElseThrow(),"A","cross-a-"+UUID.randomUUID().toString().substring(0,8),null));
        var item=items.create(b.orgId(),"cross-b-"+UUID.randomUUID().toString().substring(0,8),"B Item",null,InventoryBaseUnit.GRAM,false);
        assertThatThrownBy(()->jdbc.update("insert into branch_inventory (id,organization_id,branch_id,inventory_item_id,quantity_on_hand,version,created_at,updated_at) values (?,?,?,?,0,0,now(),now())",
                UUID.randomUUID(),a.orgId(),branch.getId(),item.getId())).isInstanceOf(DataIntegrityViolationException.class);
    }
    @Test void databaseRejectsMovementUpdateAndDelete() {
        var fx=fixture(); var branch=branches.saveAndFlush(Branch.create(organizations.findById(fx.orgId()).orElseThrow(),"Main","append-"+UUID.randomUUID().toString().substring(0,8),null));
        var item=items.create(fx.orgId(),"append-"+UUID.randomUUID().toString().substring(0,8),"Item",null,InventoryBaseUnit.GRAM,false);
        stock.open(fx.orgId(),branch.getId(),item.getId(),fx.userId(),0,"1",null);
        UUID id=jdbc.queryForObject("select id from inventory_movements where inventory_item_id=?",UUID.class,item.getId());
        assertThatThrownBy(()->jdbc.update("update inventory_movements set reason='tampered' where id=?",id)).isInstanceOf(DataIntegrityViolationException.class);
        assertThatThrownBy(()->jdbc.update("delete from inventory_movements where id=?",id)).isInstanceOf(DataIntegrityViolationException.class);
    }
}
