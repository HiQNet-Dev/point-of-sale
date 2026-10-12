package com.hiqnet.pos.inventory.stock;
import static org.assertj.core.api.Assertions.assertThat;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.get;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.jsonPath;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;
import com.hiqnet.pos.audit.AbstractAuditIntegrationTest;
import com.hiqnet.pos.branch.domain.Branch;
import com.hiqnet.pos.branch.domain.BranchRepository;
import com.hiqnet.pos.branch.domain.BranchStatus;
import com.hiqnet.pos.inventory.application.InventoryItemService;
import com.hiqnet.pos.inventory.domain.InventoryBaseUnit;
import com.hiqnet.pos.inventory.domain.InventoryItemRepository;
import com.hiqnet.pos.inventory.domain.InventoryItemStatus;
import com.hiqnet.pos.organization.domain.OrganizationRepository;
import com.hiqnet.pos.membership.domain.BranchMembership;
import com.hiqnet.pos.membership.domain.BranchMembershipRepository;
import com.hiqnet.pos.membership.domain.BranchMembershipRole;
import com.hiqnet.pos.membership.domain.BranchMembershipRoleRepository;
import java.util.UUID;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.http.MediaType;

class InventoryStockApiTest extends AbstractAuditIntegrationTest {
    @Autowired BranchRepository branches;
    @Autowired InventoryItemRepository itemRepository;
    @Autowired OrganizationRepository organizations;
    @Autowired InventoryItemService items;
    @Autowired org.springframework.jdbc.core.JdbcTemplate jdbc;
    @Autowired BranchMembershipRepository memberships;
    @Autowired BranchMembershipRoleRepository membershipRoles;
    private Fixture setup(String unit, boolean fractional) throws Exception {
        var fx=fixture();
        var branch=branches.saveAndFlush(Branch.create(organizations.findById(fx.orgId()).orElseThrow(),"Main","main-"+UUID.randomUUID().toString().substring(0,8),null));
        var item=items.create(fx.orgId(),"milk-"+UUID.randomUUID().toString().substring(0,8),"Milk",null,InventoryBaseUnit.valueOf(unit),fractional);
        return new Fixture(fx.orgId(),branch.getId(),item.getId(),adminToken(fx));
    }
    private String url(Fixture f) { return "/api/v1/organizations/"+f.org+"/branches/"+f.branch+"/inventory-items/"+f.item; }
    @Test void lazyReadsExactStringsAndMutationsRecordLedgerAndAudit() throws Exception {
        Fixture f=setup("GRAM",false); String auth=bearer(f.token); long before=auditCount(f.org);
        assertThat(jdbc.queryForObject("select count(*) from branch_inventory where branch_id=?",Long.class,f.branch)).isZero();
        mockMvc.perform(get(url(f)).header("Authorization",auth)).andExpect(status().isOk()).andExpect(jsonPath("$.quantityOnHand").value("0.000000"));
        assertThat(jdbc.queryForObject("select count(*) from branch_inventory where branch_id=?",Long.class,f.branch)).isZero();
        mockMvc.perform(post(url(f)+"/opening-stock").header("Authorization",auth).contentType(MediaType.APPLICATION_JSON).content("{\"expectedVersion\":0,\"quantity\":\"12500.5\"}"))
          .andExpect(status().isOk()).andExpect(jsonPath("$.quantityOnHand").value("12500.500000")).andExpect(jsonPath("$.version").value(1));
        mockMvc.perform(post(url(f)+"/adjustments").header("Authorization",auth).contentType(MediaType.APPLICATION_JSON).content("{\"expectedVersion\":1,\"quantityDelta\":\"-12501\",\"reason\":\" Physical count correction \"}"))
          .andExpect(status().isOk()).andExpect(jsonPath("$.quantityOnHand").value("-0.500000"));
        mockMvc.perform(get(url(f)+"/movements").header("Authorization",auth)).andExpect(status().isOk()).andExpect(jsonPath("$.items.length()").value(2));
        assertThat(auditCount(f.org)).isEqualTo(before+2); assertThat(latestAudit(f.org).action()).isEqualTo("inventory.stock.adjusted");
    }
    @Test void openingZeroIsValidButZeroAdjustmentAndFractionalDiscreteUnitAreRejected() throws Exception {
        Fixture f=setup("UNIT",false); String auth=bearer(f.token);
        mockMvc.perform(post(url(f)+"/opening-stock").header("Authorization",auth).contentType(MediaType.APPLICATION_JSON).content("{\"expectedVersion\":0,\"quantity\":\"0\"}"))
          .andExpect(status().isOk()).andExpect(jsonPath("$.quantityOnHand").value("0.000000"))
          .andExpect(jsonPath("$.version").value(1));
        mockMvc.perform(post(url(f)+"/opening-stock").header("Authorization",auth).contentType(MediaType.APPLICATION_JSON).content("{\"expectedVersion\":1,\"quantity\":\"1\"}"))
          .andExpect(status().isConflict()).andExpect(jsonPath("$.code").value("INVENTORY_OPENING_ALREADY_EXISTS"));
        mockMvc.perform(post(url(f)+"/adjustments").header("Authorization",auth).contentType(MediaType.APPLICATION_JSON).content("{\"expectedVersion\":1,\"quantityDelta\":\"0\",\"reason\":\"count\"}"))
          .andExpect(status().isBadRequest());
        Fixture d=setup("UNIT",false);
        mockMvc.perform(post(url(d)+"/opening-stock").header("Authorization",bearer(d.token)).contentType(MediaType.APPLICATION_JSON).content("{\"expectedVersion\":0,\"quantity\":\"1.5\"}"))
          .andExpect(status().isBadRequest()).andExpect(jsonPath("$.code").value("INVALID_INVENTORY_QUANTITY"));
        assertThat(jdbc.queryForObject("select count(*) from branch_inventory where branch_id=? and inventory_item_id=?",Long.class,d.branch,d.item)).isZero();
        mockMvc.perform(post(url(d)+"/opening-stock").header("Authorization",bearer(d.token)).contentType(MediaType.APPLICATION_JSON).content("{\"expectedVersion\":0,\"quantity\":1.5}"))
          .andExpect(status().isBadRequest()).andExpect(jsonPath("$.code").value("INVALID_INVENTORY_QUANTITY"));
        Fixture fractional=setup("UNIT",true);
        mockMvc.perform(post(url(fractional)+"/opening-stock").header("Authorization",bearer(fractional.token)).contentType(MediaType.APPLICATION_JSON).content("{\"expectedVersion\":0,\"quantity\":\"0.5\"}"))
          .andExpect(status().isOk()).andExpect(jsonPath("$.quantityOnHand").value("0.500000"));
    }
    @Test void adjustmentHistoryBlocksOpeningAndStaleVersionConflicts() throws Exception {
        Fixture f=setup("MILLILITER",false); String auth=bearer(f.token);
        mockMvc.perform(post(url(f)+"/adjustments").header("Authorization",auth).contentType(MediaType.APPLICATION_JSON).content("{\"expectedVersion\":0,\"quantityDelta\":\"1\",\"reason\":\"correction\"}"))
          .andExpect(status().isOk());
        mockMvc.perform(post(url(f)+"/opening-stock").header("Authorization",auth).contentType(MediaType.APPLICATION_JSON).content("{\"expectedVersion\":1,\"quantity\":\"5\"}"))
          .andExpect(status().isConflict()).andExpect(jsonPath("$.code").value("INVENTORY_HISTORY_ALREADY_EXISTS"));
        mockMvc.perform(post(url(f)+"/adjustments").header("Authorization",auth).contentType(MediaType.APPLICATION_JSON).content("{\"expectedVersion\":0,\"quantityDelta\":\"1\",\"reason\":\"stale\"}"))
          .andExpect(status().isConflict()).andExpect(jsonPath("$.code").value("CONCURRENT_MODIFICATION"));
    }

    @Test void branchMembershipStockReadIsLimitedToItsBranch() throws Exception {
        var fx=fixture();
        var a=branches.saveAndFlush(Branch.create(organizations.findById(fx.orgId()).orElseThrow(),"A","branch-a-"+UUID.randomUUID().toString().substring(0,8),null));
        var b=branches.saveAndFlush(Branch.create(organizations.findById(fx.orgId()).orElseThrow(),"B","branch-b-"+UUID.randomUUID().toString().substring(0,8),null));
        UUID role=createRole(fx,"stock-reader","Stock reader","inventory.stock.read");
        var membership=memberships.saveAndFlush(BranchMembership.create(fx.orgId(),a.getId(),fx.userId()));
        membershipRoles.saveAndFlush(BranchMembershipRole.of(fx.orgId(),membership.getId(),role));
        String token=bearer(login(fx));
        mockMvc.perform(get("/api/v1/organizations/"+fx.orgId()+"/branches/"+a.getId()+"/inventory-items").header("Authorization",token))
                .andExpect(status().isOk());
        mockMvc.perform(get("/api/v1/organizations/"+fx.orgId()+"/branches/"+b.getId()+"/inventory-items").header("Authorization",token))
                .andExpect(status().isForbidden());
    }

    @Test void crossOrganizationBranchAndItemIdsAreNotReadable() throws Exception {
        Fixture a=setup("GRAM",false); Fixture b=setup("GRAM",false);
        mockMvc.perform(get("/api/v1/organizations/"+a.org+"/branches/"+b.branch+"/inventory-items").header("Authorization",bearer(a.token)))
                .andExpect(status().isNotFound());
        mockMvc.perform(get("/api/v1/organizations/"+a.org+"/branches/"+a.branch+"/inventory-items/"+b.item).header("Authorization",bearer(a.token)))
                .andExpect(status().isNotFound());
    }


    @Test void inactiveBranchAndItemRemainReadableButCannotReceiveMutations() throws Exception {
        Fixture f=setup("GRAM",false); String auth=bearer(f.token);
        var item=itemRepository.findByIdAndOrganizationId(f.item,f.org).orElseThrow();
        item.updateStatus(InventoryItemStatus.INACTIVE); itemRepository.saveAndFlush(item);
        mockMvc.perform(get(url(f)).header("Authorization",auth)).andExpect(status().isOk());
        mockMvc.perform(post(url(f)+"/opening-stock").header("Authorization",auth).contentType(MediaType.APPLICATION_JSON)
                .content("{\"expectedVersion\":0,\"quantity\":\"1\"}"))
                .andExpect(status().isConflict()).andExpect(jsonPath("$.code").value("INVENTORY_ITEM_INACTIVE"));
        var branch=branches.findByIdAndOrganizationId(f.branch,f.org).orElseThrow();
        branch.updateStatus(BranchStatus.INACTIVE); branches.saveAndFlush(branch);
        mockMvc.perform(get(url(f)+"/movements").header("Authorization",auth)).andExpect(status().isOk());
        mockMvc.perform(post(url(f)+"/adjustments").header("Authorization",auth).contentType(MediaType.APPLICATION_JSON)
                .content("{\"expectedVersion\":0,\"quantityDelta\":\"1\",\"reason\":\"count\"}"))
                .andExpect(status().isConflict()).andExpect(jsonPath("$.code").value("BRANCH_INACTIVE"));
    }

    private record Fixture(UUID org,UUID branch,UUID item,String token) {}
}
