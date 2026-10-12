package com.hiqnet.pos.inventory.stock;
import static org.assertj.core.api.Assertions.assertThat;
import com.hiqnet.pos.AbstractRbacIntegrationTest;
import com.hiqnet.pos.branch.domain.Branch;
import com.hiqnet.pos.branch.domain.BranchRepository;
import com.hiqnet.pos.inventory.application.InventoryItemService;
import com.hiqnet.pos.inventory.domain.InventoryBaseUnit;
import com.hiqnet.pos.inventory.stock.application.InventoryStockService;
import com.hiqnet.pos.organization.domain.OrganizationRepository;
import com.hiqnet.pos.shared.error.BusinessException;
import java.util.ArrayList;
import java.util.List;
import java.util.UUID;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;
import java.util.concurrent.TimeUnit;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;

class InventoryStockConcurrencyTest extends AbstractRbacIntegrationTest {
    @Autowired BranchRepository branches;
    @Autowired OrganizationRepository organizations;
    @Autowired InventoryItemService itemService;
    @Autowired InventoryStockService stock;
    @Autowired org.springframework.jdbc.core.JdbcTemplate jdbc;
    private Fixture setup() {
        var fx=fixture();
        var branch=branches.saveAndFlush(Branch.create(organizations.findById(fx.orgId()).orElseThrow(),"Main","race-"+UUID.randomUUID().toString().substring(0,8),null));
        var item=itemService.create(fx.orgId(),"race-"+UUID.randomUUID().toString().substring(0,8),"Race Item",null,InventoryBaseUnit.GRAM,false);
        return new Fixture(fx.orgId(),branch.getId(),item.getId(),fx.userId());
    }
    @Test void firstMutationsSerializeAndCreateOneBalanceRow() throws Exception {
        Fixture f=setup(); var outcomes=parallel(
                ()->stock.adjust(f.org,f.branch,f.item,f.actor,0,"1","count A"),
                ()->stock.adjust(f.org,f.branch,f.item,f.actor,0,"2","count B"));
        assertOneWinner(outcomes);
        assertThat(jdbc.queryForObject("select count(*) from branch_inventory where branch_id=? and inventory_item_id=?",Long.class,f.branch,f.item)).isEqualTo(1);
        assertThat(jdbc.queryForObject("select count(*) from inventory_movements where branch_id=? and inventory_item_id=?",Long.class,f.branch,f.item)).isEqualTo(1);
    }
    @Test void openingRaceHasOneWinnerAndOpeningVersusAdjustmentStaysSerialized() throws Exception {
        Fixture f=setup();
        assertOneWinner(parallel(
                ()->stock.open(f.org,f.branch,f.item,f.actor,0,"10",null),
                ()->stock.open(f.org,f.branch,f.item,f.actor,0,"20",null)));
        Fixture second=setup();
        assertOneWinner(parallel(
                ()->stock.open(second.org,second.branch,second.item,second.actor,0,"10",null),
                ()->stock.adjust(second.org,second.branch,second.item,second.actor,0,"-1","physical correction")));
        assertThat(jdbc.queryForObject("select count(*) from branch_inventory where branch_id=? and inventory_item_id=?",Long.class,second.branch,second.item)).isEqualTo(1);
        assertThat(jdbc.queryForObject("select count(*) from inventory_movements where branch_id=? and inventory_item_id=?",Long.class,second.branch,second.item)).isEqualTo(1);
    }
    @Test void staleConcurrentAdjustmentsDoNotLoseUpdates() throws Exception {
        Fixture f=setup(); stock.open(f.org,f.branch,f.item,f.actor,0,"10",null);
        assertOneWinner(parallel(
                ()->stock.adjust(f.org,f.branch,f.item,f.actor,1,"-2","count A"),
                ()->stock.adjust(f.org,f.branch,f.item,f.actor,1,"-3","count B")));
        assertThat(jdbc.queryForObject("select version from branch_inventory where branch_id=? and inventory_item_id=?",Long.class,f.branch,f.item)).isEqualTo(2);
        assertThat(jdbc.queryForObject("select count(*) from inventory_movements where branch_id=? and inventory_item_id=?",Long.class,f.branch,f.item)).isEqualTo(2);
    }
    private static void assertOneWinner(List<Outcome> outcomes) {
        assertThat(outcomes.stream().filter(Outcome::success).count()).isEqualTo(1);
        assertThat(outcomes.stream().filter(o->!o.success&&"CONCURRENT_MODIFICATION".equals(o.code)).count()).isEqualTo(1);
    }
    @SafeVarargs private final List<Outcome> parallel(java.util.concurrent.Callable<?>... actions) throws Exception {
        CountDownLatch ready=new CountDownLatch(actions.length),go=new CountDownLatch(1);
        ExecutorService pool=Executors.newFixedThreadPool(actions.length); List<Future<Outcome>> futures=new ArrayList<>();
        try {
            for(var action:actions) futures.add(pool.submit(()->{ready.countDown(); if(!go.await(10,TimeUnit.SECONDS)) throw new IllegalStateException("start timeout");
                try { action.call(); return new Outcome(true,null); } catch(BusinessException ex) { return new Outcome(false,ex.getCode()); }}));
            if(!ready.await(10,TimeUnit.SECONDS)) throw new IllegalStateException("workers not ready"); go.countDown();
            List<Outcome> result=new ArrayList<>(); for(var future:futures) result.add(future.get(20,TimeUnit.SECONDS)); return result;
        } finally { pool.shutdownNow(); }
    }
    private record Fixture(UUID org,UUID branch,UUID item,UUID actor) {}
    private record Outcome(boolean success,String code) {}
}
