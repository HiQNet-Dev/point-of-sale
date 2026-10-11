package com.hiqnet.pos.sales;

import static org.assertj.core.api.Assertions.assertThat;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.get;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.patch;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.put;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.jsonPath;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import com.hiqnet.pos.AbstractRbacIntegrationTest;
import com.hiqnet.pos.branch.domain.Branch;
import com.hiqnet.pos.branch.domain.BranchRepository;
import com.hiqnet.pos.branch.domain.BranchStatus;
import com.hiqnet.pos.organization.domain.Organization;
import java.util.UUID;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.http.MediaType;

/** Exercises the public sales contract through the real security chain and DB. */
class SaleApiTest extends AbstractRbacIntegrationTest {

    @Autowired private BranchRepository branchRepository;

    @Test
    void pricesServerSideSnapshotsTheCatalogAndCancellationUnblocksShiftClose() throws Exception {
        RbacFixture fixture = fixture();
        String token = adminToken(fixture);
        UUID branchId = createBranch(fixture, "centro");
        String registerId = createRegister(fixture, branchId, token);
        String shiftId = openShift(fixture, branchId, registerId, token);
        String productId = createProduct(fixture, token, "CAFE", "Café", "12.5000");
        String salesUrl = salesUrl(fixture, branchId, registerId, shiftId);
        String saleId = objectMapper.readTree(mockMvc.perform(post(salesUrl).header("Authorization", bearer(token)))
                .andExpect(status().isCreated()).andExpect(jsonPath("$.status").value("OPEN"))
                .andReturn().getResponse().getContentAsString()).get("id").asText();

        mockMvc.perform(post(salesUrl + "/" + saleId + "/items")
                        .header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedSaleVersion\":0,\"productId\":\"" + productId
                                + "\",\"quantity\":2,\"modifierOptionIds\":[] ,\"lineTotal\":999999}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.sale.totalAmount").value(25.0))
                .andExpect(jsonPath("$.items[0].productSkuSnapshot").value("CAFE"))
                .andExpect(jsonPath("$.items[0].productNameSnapshot").value("Café"))
                .andExpect(jsonPath("$.items[0].baseUnitPrice").value(12.5))
                .andExpect(jsonPath("$.items[0].lineTotal").value(25.0))
                .andExpect(jsonPath("$.sale.version").value(1));

        mockMvc.perform(post(shiftsUrl(fixture, branchId, registerId) + "/" + shiftId + "/close")
                        .header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"declaredCashAmount\":0}"))
                .andExpect(status().isConflict()).andExpect(jsonPath("$.code").value("CASH_SHIFT_HAS_OPEN_SALES"));

        mockMvc.perform(post(salesUrl + "/" + saleId + "/cancel").header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON).content("{\"expectedVersion\":1}"))
                .andExpect(status().isOk()).andExpect(jsonPath("$.sale.status").value("CANCELLED"));

        mockMvc.perform(post(shiftsUrl(fixture, branchId, registerId) + "/" + shiftId + "/close")
                        .header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"declaredCashAmount\":0}"))
                .andExpect(status().isOk());
    }

    @Test
    void validatesModifierSelectionsAndPreservesModifierSnapshots() throws Exception {
        RbacFixture fixture = fixture();
        String token = adminToken(fixture);
        UUID branchId = createBranch(fixture, "modifier-sales");
        String registerId = createRegister(fixture, branchId, token);
        String shiftId = openShift(fixture, branchId, registerId, token);
        String productId = createProduct(fixture, token, "LATTE", "Latte", "10.0000");
        String groupId = createModifierGroup(fixture, token, "milk", 1, 1);
        String optionId = createModifierOption(fixture, token, groupId, "oat", "Oat", "2.0000");
        assignModifierGroup(fixture, token, productId, groupId);
        String salesUrl = salesUrl(fixture, branchId, registerId, shiftId);
        String saleId = id(post(salesUrl).header("Authorization", bearer(token)));

        mockMvc.perform(post(salesUrl + "/" + saleId + "/items").header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedSaleVersion\":0,\"productId\":\"" + productId + "\",\"quantity\":1,\"modifierOptionIds\":[]}"))
                .andExpect(status().isBadRequest()).andExpect(jsonPath("$.code").value("INVALID_MODIFIER_SELECTION"));
        mockMvc.perform(post(salesUrl + "/" + saleId + "/items").header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedSaleVersion\":0,\"productId\":\"" + productId + "\",\"quantity\":1,\"modifierOptionIds\":[\"" + optionId + "\",\"" + optionId + "\"]}"))
                .andExpect(status().isBadRequest()).andExpect(jsonPath("$.code").value("INVALID_MODIFIER_SELECTION"));
        mockMvc.perform(post(salesUrl + "/" + saleId + "/items").header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedSaleVersion\":0,\"productId\":\"" + productId + "\",\"quantity\":1,\"modifierOptionIds\":[\"" + optionId + "\"]}"))
                .andExpect(status().isOk()).andExpect(jsonPath("$.items[0].modifierUnitDelta").value(2.0))
                .andExpect(jsonPath("$.items[0].effectiveUnitPrice").value(12.0))
                .andExpect(jsonPath("$.items[0].modifiers[0].priceDeltaSnapshot").value(2.0));

        mockMvc.perform(patch("/api/v1/organizations/" + fixture.orgId() + "/modifier-groups/" + groupId + "/options/" + optionId)
                        .header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"priceDelta\":3.0000}"))
                .andExpect(status().isOk());
        mockMvc.perform(get(salesUrl + "/" + saleId).header("Authorization", bearer(token)))
                .andExpect(status().isOk()).andExpect(jsonPath("$.items[0].modifiers[0].priceDeltaSnapshot").value(2.0));

        mockMvc.perform(patch("/api/v1/organizations/" + fixture.orgId() + "/modifier-groups/" + groupId)
                        .header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"status\":\"INACTIVE\"}"))
                .andExpect(status().isOk());
        String laterSaleId = id(post(salesUrl).header("Authorization", bearer(token)));
        mockMvc.perform(post(salesUrl + "/" + laterSaleId + "/items").header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedSaleVersion\":0,\"productId\":\"" + productId + "\",\"quantity\":1,\"modifierOptionIds\":[]}"))
                .andExpect(status().isConflict()).andExpect(jsonPath("$.code").value("PRODUCT_MODIFIER_CONFIGURATION_UNAVAILABLE"));
    }

    @Test
    void inactiveBranchStillAllowsReadingAndCancellingAnOpenSaleButNotItemMutations() throws Exception {
        RbacFixture fixture = fixture();
        String token = adminToken(fixture);
        UUID branchId = createBranch(fixture, "inactive-recovery");
        String registerId = createRegister(fixture, branchId, token);
        String shiftId = openShift(fixture, branchId, registerId, token);
        String productId = createProduct(fixture, token, "RECOVERY", "Recovery", "1.0000");
        String salesUrl = salesUrl(fixture, branchId, registerId, shiftId);
        String saleId = id(post(salesUrl).header("Authorization", bearer(token)));
        Branch inactiveBranch = branchRepository.findById(branchId).orElseThrow();
        inactiveBranch.updateStatus(BranchStatus.INACTIVE);
        branchRepository.saveAndFlush(inactiveBranch);

        mockMvc.perform(get(salesUrl + "/" + saleId).header("Authorization", bearer(token))).andExpect(status().isOk());
        mockMvc.perform(post(salesUrl + "/" + saleId + "/items").header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedSaleVersion\":0,\"productId\":\"" + productId + "\",\"quantity\":1,\"modifierOptionIds\":[]}"))
                .andExpect(status().isConflict()).andExpect(jsonPath("$.code").value("BRANCH_INACTIVE"));
        mockMvc.perform(post(salesUrl + "/" + saleId + "/cancel").header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON).content("{\"expectedVersion\":0}"))
                .andExpect(status().isOk()).andExpect(jsonPath("$.sale.status").value("CANCELLED"));
    }

    @Test
    void inactiveProductAndStaleVersionAreRejected() throws Exception {
        RbacFixture fixture = fixture();
        String token = adminToken(fixture);
        UUID branchId = createBranch(fixture, "centro");
        String registerId = createRegister(fixture, branchId, token);
        String shiftId = openShift(fixture, branchId, registerId, token);
        String productId = createProduct(fixture, token, "TE", "Té", "5.0000");
        String salesUrl = salesUrl(fixture, branchId, registerId, shiftId);
        String saleId = objectMapper.readTree(mockMvc.perform(post(salesUrl).header("Authorization", bearer(token)))
                .andExpect(status().isCreated()).andReturn().getResponse().getContentAsString()).get("id").asText();

        mockMvc.perform(post(salesUrl + "/" + saleId + "/items").header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON).content("{\"expectedSaleVersion\":4,\"productId\":\"" + productId + "\",\"quantity\":1}"))
                .andExpect(status().isConflict()).andExpect(jsonPath("$.code").value("CONCURRENT_MODIFICATION"));

        mockMvc.perform(patch("/api/v1/organizations/" + fixture.orgId() + "/products/" + productId)
                        .header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"status\":\"INACTIVE\"}"))
                .andExpect(status().isOk());
        mockMvc.perform(post(salesUrl + "/" + saleId + "/items").header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON).content("{\"expectedSaleVersion\":0,\"productId\":\"" + productId + "\",\"quantity\":1}"))
                .andExpect(status().isConflict()).andExpect(jsonPath("$.code").value("PRODUCT_INACTIVE"));
        assertThat(productId).isNotBlank();
    }

    private UUID createBranch(RbacFixture fixture, String code) {
        Organization organization = organizationRepository.findById(fixture.orgId()).orElseThrow();
        return branchRepository.saveAndFlush(Branch.create(organization, "Branch " + code, code, null)).getId();
    }

    private String createRegister(RbacFixture fixture, UUID branchId, String token) throws Exception {
        return id(post(registersUrl(fixture, branchId)).header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON)
                .content("{\"code\":\"register\",\"name\":\"Register\"}"));
    }

    private String openShift(RbacFixture fixture, UUID branchId, String registerId, String token) throws Exception {
        return id(post(shiftsUrl(fixture, branchId, registerId)).header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON)
                .content("{\"openingAmount\":0}"));
    }

    private String createProduct(RbacFixture fixture, String token, String sku, String name, String price) throws Exception {
        return id(post("/api/v1/organizations/" + fixture.orgId() + "/products").header("Authorization", bearer(token))
                .contentType(MediaType.APPLICATION_JSON).content("{\"sku\":\"" + sku + "\",\"name\":\"" + name + "\",\"unitPrice\":" + price + "}"));
    }

    private String createModifierGroup(RbacFixture fixture, String token, String code, int minSelections, int maxSelections) throws Exception {
        return id(post("/api/v1/organizations/" + fixture.orgId() + "/modifier-groups").header("Authorization", bearer(token))
                .contentType(MediaType.APPLICATION_JSON).content("{\"code\":\"" + code + "\",\"name\":\"" + code
                        + "\",\"minSelections\":" + minSelections + ",\"maxSelections\":" + maxSelections + "}"));
    }

    private String createModifierOption(RbacFixture fixture, String token, String groupId, String code, String name, String priceDelta) throws Exception {
        return id(post("/api/v1/organizations/" + fixture.orgId() + "/modifier-groups/" + groupId + "/options")
                .header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON)
                .content("{\"code\":\"" + code + "\",\"name\":\"" + name + "\",\"priceDelta\":" + priceDelta + ",\"displayOrder\":0}"));
    }

    private void assignModifierGroup(RbacFixture fixture, String token, String productId, String groupId) throws Exception {
        mockMvc.perform(put("/api/v1/organizations/" + fixture.orgId() + "/products/" + productId + "/modifier-groups")
                        .header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"modifierGroupIds\":[\"" + groupId + "\"]}"))
                .andExpect(status().isOk());
    }

    private String id(org.springframework.test.web.servlet.request.MockHttpServletRequestBuilder request) throws Exception {
        return objectMapper.readTree(mockMvc.perform(request).andExpect(status().isCreated()).andReturn().getResponse().getContentAsString()).get("id").asText();
    }

    private static String registersUrl(RbacFixture fixture, UUID branchId) { return "/api/v1/organizations/" + fixture.orgId() + "/branches/" + branchId + "/cash-registers"; }
    private static String shiftsUrl(RbacFixture fixture, UUID branchId, String registerId) { return registersUrl(fixture, branchId) + "/" + registerId + "/cash-shifts"; }
    private static String salesUrl(RbacFixture fixture, UUID branchId, String registerId, String shiftId) { return shiftsUrl(fixture, branchId, registerId) + "/" + shiftId + "/sales"; }
}
