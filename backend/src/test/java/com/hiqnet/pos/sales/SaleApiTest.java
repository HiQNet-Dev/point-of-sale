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
import com.hiqnet.pos.cashregister.domain.CashRegister;
import com.hiqnet.pos.cashregister.domain.CashRegisterRepository;
import com.hiqnet.pos.cashregister.domain.CashRegisterStatus;
import com.hiqnet.pos.organization.domain.Organization;
import java.util.UUID;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.http.MediaType;
import org.springframework.jdbc.core.JdbcTemplate;

/** Exercises the public sales contract through the real security chain and DB. */
class SaleApiTest extends AbstractRbacIntegrationTest {

    @Autowired private BranchRepository branchRepository;
    @Autowired private CashRegisterRepository cashRegisterRepository;
    @Autowired private JdbcTemplate jdbcTemplate;

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

        mockMvc.perform(patch("/api/v1/organizations/" + fixture.orgId() + "/products/" + productId)
                        .header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"name\":\"Café renombrado\",\"unitPrice\":75.0000}"))
                .andExpect(status().isOk());
        mockMvc.perform(get(salesUrl + "/" + saleId).header("Authorization", bearer(token)))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.sale.totalAmount").value(25.0))
                .andExpect(jsonPath("$.items[0].productSkuSnapshot").value("CAFE"))
                .andExpect(jsonPath("$.items[0].productNameSnapshot").value("Café"))
                .andExpect(jsonPath("$.items[0].baseUnitPrice").value(12.5))
                .andExpect(jsonPath("$.items[0].effectiveUnitPrice").value(12.5))
                .andExpect(jsonPath("$.items[0].lineTotal").value(25.0));

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
    void sameItemPatchIsANoOpWithoutVersionAuditOrModifierRewrite() throws Exception {
        RbacFixture fixture = fixture();
        String token = adminToken(fixture);
        UUID branchId = createBranch(fixture, "same-item");
        String registerId = createRegister(fixture, branchId, token);
        String shiftId = openShift(fixture, branchId, registerId, token);
        String productId = createProduct(fixture, token, "NOOP", "No-op", "10.0000");
        String groupId = createModifierGroup(fixture, token, "size", 0, 1);
        String optionId = createModifierOption(fixture, token, groupId, "large", "Large", "1.0000");
        assignModifierGroup(fixture, token, productId, groupId);
        String salesUrl = salesUrl(fixture, branchId, registerId, shiftId);
        String saleId = id(post(salesUrl).header("Authorization", bearer(token)));
        String addResponse = mockMvc.perform(post(salesUrl + "/" + saleId + "/items").header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedSaleVersion\":0,\"productId\":\"" + productId + "\",\"quantity\":2,\"modifierOptionIds\":[\"" + optionId + "\"]}"))
                .andExpect(status().isOk()).andReturn().getResponse().getContentAsString();
        String itemId = objectMapper.readTree(addResponse).at("/items/0/id").asText();
        String modifierSnapshotId = objectMapper.readTree(addResponse).at("/items/0/modifiers/0/id").asText();
        long auditBefore = auditCount(fixture.orgId());

        mockMvc.perform(patch(salesUrl + "/" + saleId + "/items/" + itemId).header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedSaleVersion\":1,\"quantity\":2,\"modifierOptionIds\":[\"" + optionId + "\"]}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.sale.version").value(1))
                .andExpect(jsonPath("$.sale.totalAmount").value(22.0))
                .andExpect(jsonPath("$.items[0].modifiers[0].id").value(modifierSnapshotId));
        assertThat(auditCount(fixture.orgId())).isEqualTo(auditBefore);
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
    void inactiveRegisterStillAllowsReadingAndCancellingAnOpenSaleButNotItemMutations() throws Exception {
        RbacFixture fixture = fixture();
        String token = adminToken(fixture);
        UUID branchId = createBranch(fixture, "inactive-register-recovery");
        String registerId = createRegister(fixture, branchId, token);
        String shiftId = openShift(fixture, branchId, registerId, token);
        String productId = createProduct(fixture, token, "REGISTER-RECOVERY", "Register recovery", "1.0000");
        String salesUrl = salesUrl(fixture, branchId, registerId, shiftId);
        String saleId = id(post(salesUrl).header("Authorization", bearer(token)));
        CashRegister inactiveRegister = cashRegisterRepository.findById(UUID.fromString(registerId)).orElseThrow();
        inactiveRegister.updateStatus(CashRegisterStatus.INACTIVE);
        cashRegisterRepository.saveAndFlush(inactiveRegister);

        mockMvc.perform(get(salesUrl + "/" + saleId).header("Authorization", bearer(token))).andExpect(status().isOk());
        mockMvc.perform(post(salesUrl + "/" + saleId + "/items").header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedSaleVersion\":0,\"productId\":\"" + productId + "\",\"quantity\":1,\"modifierOptionIds\":[]}"))
                .andExpect(status().isConflict()).andExpect(jsonPath("$.code").value("CASH_REGISTER_INACTIVE"));
        mockMvc.perform(post(salesUrl + "/" + saleId + "/cancel").header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON).content("{\"expectedVersion\":0}"))
                .andExpect(status().isOk()).andExpect(jsonPath("$.sale.status").value("CANCELLED"));
    }

    @Test
    void modifierValidationCoversOptionalBoundsInactiveUnassignedAndCrossOrganizationOptions() throws Exception {
        RbacFixture fixture = fixture();
        String token = adminToken(fixture);
        UUID branchId = createBranch(fixture, "modifier-matrix");
        String registerId = createRegister(fixture, branchId, token);
        String shiftId = openShift(fixture, branchId, registerId, token);
        String productId = createProduct(fixture, token, "MATRIX", "Matrix", "10.0000");
        String groupId = createModifierGroup(fixture, token, "extras", 0, 1);
        String optionOne = createModifierOption(fixture, token, groupId, "extra-one", "Extra one", "1.0000");
        String optionTwo = createModifierOption(fixture, token, groupId, "extra-two", "Extra two", "2.0000");
        String unassignedGroup = createModifierGroup(fixture, token, "unassigned", 0, 1);
        String unassignedOption = createModifierOption(fixture, token, unassignedGroup, "unassigned-option", "Unassigned", "1.0000");
        assignModifierGroup(fixture, token, productId, groupId);
        String salesUrl = salesUrl(fixture, branchId, registerId, shiftId);
        String saleId = id(post(salesUrl).header("Authorization", bearer(token)));

        mockMvc.perform(post(salesUrl + "/" + saleId + "/items").header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedSaleVersion\":0,\"productId\":\"" + productId + "\",\"quantity\":1,\"modifierOptionIds\":[\"" + optionOne + "\"]}"))
                .andExpect(status().isOk());
        String tooManySale = id(post(salesUrl).header("Authorization", bearer(token)));
        mockMvc.perform(post(salesUrl + "/" + tooManySale + "/items").header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedSaleVersion\":0,\"productId\":\"" + productId + "\",\"quantity\":1,\"modifierOptionIds\":[\"" + optionOne + "\",\"" + optionTwo + "\"]}"))
                .andExpect(status().isBadRequest()).andExpect(jsonPath("$.code").value("INVALID_MODIFIER_SELECTION"));
        String unassignedSale = id(post(salesUrl).header("Authorization", bearer(token)));
        mockMvc.perform(post(salesUrl + "/" + unassignedSale + "/items").header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedSaleVersion\":0,\"productId\":\"" + productId + "\",\"quantity\":1,\"modifierOptionIds\":[\"" + unassignedOption + "\"]}"))
                .andExpect(status().isBadRequest()).andExpect(jsonPath("$.code").value("INVALID_MODIFIER_SELECTION"));
        RbacFixture otherFixture = fixture();
        String otherToken = adminToken(otherFixture);
        String otherGroup = createModifierGroup(otherFixture, otherToken, "other-org", 0, 1);
        String otherOption = createModifierOption(otherFixture, otherToken, otherGroup, "other-option", "Other option", "1.0000");
        String crossOrganizationSale = id(post(salesUrl).header("Authorization", bearer(token)));
        mockMvc.perform(post(salesUrl + "/" + crossOrganizationSale + "/items").header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedSaleVersion\":0,\"productId\":\"" + productId + "\",\"quantity\":1,\"modifierOptionIds\":[\"" + otherOption + "\"]}"))
                .andExpect(status().isBadRequest()).andExpect(jsonPath("$.code").value("INVALID_MODIFIER_SELECTION"));
        mockMvc.perform(patch("/api/v1/organizations/" + fixture.orgId() + "/modifier-groups/" + groupId + "/options/" + optionTwo)
                        .header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"status\":\"INACTIVE\"}"))
                .andExpect(status().isOk());
        String inactiveOptionSale = id(post(salesUrl).header("Authorization", bearer(token)));
        mockMvc.perform(post(salesUrl + "/" + inactiveOptionSale + "/items").header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedSaleVersion\":0,\"productId\":\"" + productId + "\",\"quantity\":1,\"modifierOptionIds\":[\"" + optionTwo + "\"]}"))
                .andExpect(status().isConflict()).andExpect(jsonPath("$.code").value("INVALID_MODIFIER_SELECTION"));
        mockMvc.perform(patch("/api/v1/organizations/" + fixture.orgId() + "/modifier-groups/" + groupId)
                        .header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"status\":\"INACTIVE\"}"))
                .andExpect(status().isOk());
        String optionalInactiveSale = id(post(salesUrl).header("Authorization", bearer(token)));
        mockMvc.perform(post(salesUrl + "/" + optionalInactiveSale + "/items").header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedSaleVersion\":0,\"productId\":\"" + productId + "\",\"quantity\":1,\"modifierOptionIds\":[]}"))
                .andExpect(status().isOk());
    }

    @Test
    void rejectsNegativeEffectivePricesAndAmountOverflowWithoutPersistingLines() throws Exception {
        RbacFixture fixture = fixture();
        String token = adminToken(fixture);
        UUID branchId = createBranch(fixture, "pricing-boundaries");
        String registerId = createRegister(fixture, branchId, token);
        String shiftId = openShift(fixture, branchId, registerId, token);
        String salesUrl = salesUrl(fixture, branchId, registerId, shiftId);
        String affordableProduct = createProduct(fixture, token, "NEGATIVE", "Negative", "1.0000");
        String groupId = createModifierGroup(fixture, token, "negative-price", 0, 1);
        String negativeOption = createModifierOption(fixture, token, groupId, "minus-two", "Minus two", "-2.0000");
        assignModifierGroup(fixture, token, affordableProduct, groupId);
        String negativeSale = id(post(salesUrl).header("Authorization", bearer(token)));
        mockMvc.perform(post(salesUrl + "/" + negativeSale + "/items").header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedSaleVersion\":0,\"productId\":\"" + affordableProduct + "\",\"quantity\":1,\"modifierOptionIds\":[\"" + negativeOption + "\"]}"))
                .andExpect(status().isConflict()).andExpect(jsonPath("$.code").value("NEGATIVE_EFFECTIVE_PRICE"));
        mockMvc.perform(get(salesUrl + "/" + negativeSale).header("Authorization", bearer(token)))
                .andExpect(jsonPath("$.sale.totalAmount").value(0));

        String maximumProduct = createProduct(fixture, token, "MAXIMUM", "Maximum", "999999999999999.9999");
        String overflowSale = id(post(salesUrl).header("Authorization", bearer(token)));
        mockMvc.perform(post(salesUrl + "/" + overflowSale + "/items").header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedSaleVersion\":0,\"productId\":\"" + maximumProduct + "\",\"quantity\":2,\"modifierOptionIds\":[]}"))
                .andExpect(status().isBadRequest()).andExpect(jsonPath("$.code").value("VALIDATION_ERROR"));
    }

    @Test
    void cancelledSalesAndRemovedItemsAreOperationallyImmutable() throws Exception {
        RbacFixture fixture = fixture();
        String token = adminToken(fixture);
        UUID branchId = createBranch(fixture, "item-lifecycles");
        String registerId = createRegister(fixture, branchId, token);
        String shiftId = openShift(fixture, branchId, registerId, token);
        String productId = createProduct(fixture, token, "LIFECYCLE", "Lifecycle", "2.0000");
        String salesUrl = salesUrl(fixture, branchId, registerId, shiftId);
        String cancelledSale = id(post(salesUrl).header("Authorization", bearer(token)));
        mockMvc.perform(post(salesUrl + "/" + cancelledSale + "/cancel").header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON).content("{\"expectedVersion\":0}"))
                .andExpect(status().isOk());
        mockMvc.perform(post(salesUrl + "/" + cancelledSale + "/items").header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedSaleVersion\":1,\"productId\":\"" + productId + "\",\"quantity\":1,\"modifierOptionIds\":[]}"))
                .andExpect(status().isConflict()).andExpect(jsonPath("$.code").value("SALE_NOT_OPEN"));
        mockMvc.perform(post(salesUrl + "/" + cancelledSale + "/cancel").header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON).content("{\"expectedVersion\":1}"))
                .andExpect(status().isConflict()).andExpect(jsonPath("$.code").value("SALE_ALREADY_CANCELLED"));

        String openSale = id(post(salesUrl).header("Authorization", bearer(token)));
        String added = mockMvc.perform(post(salesUrl + "/" + openSale + "/items").header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedSaleVersion\":0,\"productId\":\"" + productId + "\",\"quantity\":1,\"modifierOptionIds\":[]}"))
                .andExpect(status().isOk()).andReturn().getResponse().getContentAsString();
        String itemId = objectMapper.readTree(added).at("/items/0/id").asText();
        mockMvc.perform(post(salesUrl + "/" + openSale + "/items/" + itemId + "/remove").header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON).content("{\"expectedVersion\":1}"))
                .andExpect(status().isOk()).andExpect(jsonPath("$.items[0].status").value("REMOVED"));
        mockMvc.perform(patch(salesUrl + "/" + openSale + "/items/" + itemId).header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedSaleVersion\":2,\"quantity\":1,\"modifierOptionIds\":[]}"))
                .andExpect(status().isConflict()).andExpect(jsonPath("$.code").value("SALE_ITEM_ALREADY_REMOVED"));
        mockMvc.perform(post(salesUrl + "/" + openSale + "/items/" + itemId + "/remove").header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON).content("{\"expectedVersion\":2}"))
                .andExpect(status().isConflict()).andExpect(jsonPath("$.code").value("SALE_ITEM_ALREADY_REMOVED"));
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

    private long auditCount(UUID organizationId) {
        Long count = jdbcTemplate.queryForObject(
                "select count(*) from audit_events where organization_id = ?", Long.class, organizationId);
        return count == null ? 0 : count;
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
