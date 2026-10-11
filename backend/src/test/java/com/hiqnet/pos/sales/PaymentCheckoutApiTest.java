package com.hiqnet.pos.sales;

import static org.assertj.core.api.Assertions.assertThat;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.get;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.jsonPath;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import com.hiqnet.pos.AbstractRbacIntegrationTest;
import com.hiqnet.pos.rbac.domain.Permission;
import java.util.List;
import java.util.UUID;
import org.junit.jupiter.api.Test;
import org.springframework.http.MediaType;
import org.springframework.jdbc.core.JdbcTemplate;
import org.springframework.beans.factory.annotation.Autowired;

/** Public checkout contract: payment facts and completion are a single operation. */
class PaymentCheckoutApiTest extends AbstractRbacIntegrationTest {
    @Autowired private JdbcTemplate jdbcTemplate;

    @Test
    void splitTenderCompletesAtomicallyCalculatesCashChangeAndUnblocksShiftClose() throws Exception {
        RbacFixture fixture = fixture();
        String token = adminToken(fixture);
        UUID branchId = createBranch(fixture, "checkout-split", token);
        String registerId = createRegister(fixture, branchId, token);
        String shiftId = openShift(fixture, branchId, registerId, token);
        String productId = createProduct(fixture, token, "CHECKOUT", "Checkout", "100.0000");
        String salesUrl = salesUrl(fixture, branchId, registerId, shiftId);
        String saleId = id(post(salesUrl).header("Authorization", bearer(token)));
        mockMvc.perform(post(salesUrl + "/" + saleId + "/items").header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedSaleVersion\":0,\"productId\":\"" + productId + "\",\"quantity\":1,\"modifierOptionIds\":[]}"))
                .andExpect(status().isOk()).andExpect(jsonPath("$.sale.version").value(1));

        mockMvc.perform(post(salesUrl + "/" + saleId + "/checkout").header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedSaleVersion\":1,\"payments\":[{\"method\":\"CARD\",\"amount\":\"40.0000\"},{\"method\":\"CASH\",\"amount\":\"60.0000\",\"tenderedAmount\":\"100.0000\"}]}"))
                .andExpect(status().isOk()).andExpect(jsonPath("$.sale.status").value("COMPLETED"))
                .andExpect(jsonPath("$.sale.completedAt").exists()).andExpect(jsonPath("$.sale.version").value(2));
        mockMvc.perform(get(salesUrl + "/" + saleId + "/payments").header("Authorization", bearer(token)))
                .andExpect(status().isOk()).andExpect(jsonPath("$.length()").value(2))
                .andExpect(jsonPath("$[0].method").value("CARD")).andExpect(jsonPath("$[1].method").value("CASH"))
                .andExpect(jsonPath("$[1].amount").value(60.0)).andExpect(jsonPath("$[1].tenderedAmount").value(100.0))
                .andExpect(jsonPath("$[1].changeAmount").value(40.0));
        mockMvc.perform(post(salesUrl + "/" + saleId + "/checkout").header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedSaleVersion\":2,\"payments\":[{\"method\":\"CARD\",\"amount\":\"100.0000\"}]}"))
                .andExpect(status().isConflict()).andExpect(jsonPath("$.code").value("SALE_ALREADY_COMPLETED"));
        assertThat(jdbcTemplate.queryForObject("select count(*) from payments where sale_id = ?", Long.class, UUID.fromString(saleId))).isEqualTo(2);
        mockMvc.perform(post(shiftsUrl(fixture, branchId, registerId) + "/" + shiftId + "/close")
                        .header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"declaredCashAmount\":0}"))
                .andExpect(status().isOk());
    }

    @Test
    void rejectsEmptySaleAndMismatchedPaymentsWithoutPersistingAnything() throws Exception {
        RbacFixture fixture = fixture();
        String token = adminToken(fixture);
        UUID branchId = createBranch(fixture, "checkout-validation", token);
        String registerId = createRegister(fixture, branchId, token);
        String shiftId = openShift(fixture, branchId, registerId, token);
        String productId = createProduct(fixture, token, "VALIDATE", "Validate", "100.0000");
        String salesUrl = salesUrl(fixture, branchId, registerId, shiftId);
        String emptySaleId = id(post(salesUrl).header("Authorization", bearer(token)));
        mockMvc.perform(post(salesUrl + "/" + emptySaleId + "/checkout").header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON).content("{\"expectedSaleVersion\":0,\"payments\":[]}"))
                .andExpect(status().isConflict()).andExpect(jsonPath("$.code").value("SALE_HAS_NO_ACTIVE_ITEMS"));
        String saleId = id(post(salesUrl).header("Authorization", bearer(token)));
        mockMvc.perform(post(salesUrl + "/" + saleId + "/items").header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedSaleVersion\":0,\"productId\":\"" + productId + "\",\"quantity\":1,\"modifierOptionIds\":[]}"))
                .andExpect(status().isOk());
        mockMvc.perform(post(salesUrl + "/" + saleId + "/checkout").header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedSaleVersion\":1,\"payments\":[{\"method\":\"CARD\",\"amount\":\"99.0000\"}]}"))
                .andExpect(status().isConflict()).andExpect(jsonPath("$.code").value("PAYMENT_TOTAL_MISMATCH"));
        assertThat(jdbcTemplate.queryForObject("select count(*) from payments where sale_id = ?", Long.class, UUID.fromString(saleId))).isZero();
        assertThat(jdbcTemplate.queryForObject("select status from sales where id = ?", String.class, UUID.fromString(saleId))).isEqualTo("OPEN");
    }

    @Test
    void checkoutUsesSaleSnapshotsAndAllowsAZeroTotalSaleWithoutPayments() throws Exception {
        RbacFixture fixture = fixture();
        String token = adminToken(fixture);
        UUID branchId = createBranch(fixture, "checkout-snapshots", token);
        String registerId = createRegister(fixture, branchId, token);
        String shiftId = openShift(fixture, branchId, registerId, token);
        String salesUrl = salesUrl(fixture, branchId, registerId, shiftId);
        String pricedProductId = createProduct(fixture, token, "SNAPSHOT-100", "Snapshot", "100.0000");
        String pricedSaleId = id(post(salesUrl).header("Authorization", bearer(token)));
        mockMvc.perform(post(salesUrl + "/" + pricedSaleId + "/items").header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedSaleVersion\":0,\"productId\":\"" + pricedProductId + "\",\"quantity\":1,\"modifierOptionIds\":[]}"))
                .andExpect(status().isOk());
        jdbcTemplate.update("update products set unit_price = 200.0000 where id = ?", UUID.fromString(pricedProductId));
        mockMvc.perform(post(salesUrl + "/" + pricedSaleId + "/checkout").header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedSaleVersion\":1,\"payments\":[{\"method\":\"CARD\",\"amount\":\"100.0000\"}]}"))
                .andExpect(status().isOk()).andExpect(jsonPath("$.sale.status").value("COMPLETED"))
                .andExpect(jsonPath("$.sale.totalAmount").value(100.0));
        assertThat(jdbcTemplate.queryForObject("select amount from payments where sale_id = ?", java.math.BigDecimal.class,
                UUID.fromString(pricedSaleId))).isEqualByComparingTo("100.0000");

        String freeProductId = createProduct(fixture, token, "SNAPSHOT-0", "Free", "0.0000");
        String freeSaleId = id(post(salesUrl).header("Authorization", bearer(token)));
        mockMvc.perform(post(salesUrl + "/" + freeSaleId + "/items").header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedSaleVersion\":0,\"productId\":\"" + freeProductId + "\",\"quantity\":1,\"modifierOptionIds\":[]}"))
                .andExpect(status().isOk());
        mockMvc.perform(post(salesUrl + "/" + freeSaleId + "/checkout").header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON).content("{\"expectedSaleVersion\":1,\"payments\":[]}"))
                .andExpect(status().isOk()).andExpect(jsonPath("$.sale.status").value("COMPLETED"));
        assertThat(jdbcTemplate.queryForObject("select count(*) from payments where sale_id = ?", Long.class,
                UUID.fromString(freeSaleId))).isZero();
    }

    @Test
    void rejectsRemovedOnlySalesAndInvalidPaymentListShapesWithoutWrites() throws Exception {
        RbacFixture fixture = fixture();
        String token = adminToken(fixture);
        UUID branchId = createBranch(fixture, "checkout-shapes", token);
        String registerId = createRegister(fixture, branchId, token);
        String shiftId = openShift(fixture, branchId, registerId, token);
        String productId = createProduct(fixture, token, "SHAPES", "Shapes", "100.0000");
        String salesUrl = salesUrl(fixture, branchId, registerId, shiftId);
        String removedSaleId = id(post(salesUrl).header("Authorization", bearer(token)));
        String itemResponse = mockMvc.perform(post(salesUrl + "/" + removedSaleId + "/items").header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedSaleVersion\":0,\"productId\":\"" + productId + "\",\"quantity\":1,\"modifierOptionIds\":[]}"))
                .andExpect(status().isOk()).andReturn().getResponse().getContentAsString();
        String itemId = objectMapper.readTree(itemResponse).get("items").get(0).get("id").asText();
        mockMvc.perform(post(salesUrl + "/" + removedSaleId + "/items/" + itemId + "/remove").header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON).content("{\"expectedVersion\":1}"))
                .andExpect(status().isOk());
        mockMvc.perform(post(salesUrl + "/" + removedSaleId + "/checkout").header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON).content("{\"expectedSaleVersion\":2,\"payments\":[]}"))
                .andExpect(status().isConflict()).andExpect(jsonPath("$.code").value("SALE_HAS_NO_ACTIVE_ITEMS"));

        String saleId = id(post(salesUrl).header("Authorization", bearer(token)));
        mockMvc.perform(post(salesUrl + "/" + saleId + "/items").header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedSaleVersion\":0,\"productId\":\"" + productId + "\",\"quantity\":1,\"modifierOptionIds\":[]}"))
                .andExpect(status().isOk());
        String checkoutUrl = salesUrl + "/" + saleId + "/checkout";
        mockMvc.perform(post(checkoutUrl).header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedSaleVersion\":1,\"payments\":[{\"method\":\"CARD\",\"amount\":\"100.0000\",\"tenderedAmount\":\"100.0000\"}]}"))
                .andExpect(status().isBadRequest()).andExpect(jsonPath("$.code").value("INVALID_PAYMENT"));
        mockMvc.perform(post(checkoutUrl).header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedSaleVersion\":1,\"payments\":[{\"method\":\"CASH\",\"amount\":\"50.0000\",\"tenderedAmount\":\"50.0000\"},{\"method\":\"CASH\",\"amount\":\"50.0000\",\"tenderedAmount\":\"50.0000\"}]}"))
                .andExpect(status().isBadRequest()).andExpect(jsonPath("$.code").value("MULTIPLE_CASH_PAYMENTS_NOT_ALLOWED"));
        assertThat(jdbcTemplate.queryForObject("select count(*) from payments where sale_id = ?", Long.class, UUID.fromString(saleId))).isZero();
        assertThat(jdbcTemplate.queryForObject("select version from sales where id = ?", Long.class, UUID.fromString(saleId))).isEqualTo(1L);
    }

    @Test
    void permitsMultipleCardsUpToTenAndRejectsElevenOrExcessPrecisionWithoutWrites() throws Exception {
        RbacFixture fixture = fixture();
        String token = adminToken(fixture);
        UUID branchId = createBranch(fixture, "checkout-payment-count", token);
        String registerId = createRegister(fixture, branchId, token);
        String shiftId = openShift(fixture, branchId, registerId, token);
        String productId = createProduct(fixture, token, "TEN-CARDS", "Ten cards", "100.0000");
        String salesUrl = salesUrl(fixture, branchId, registerId, shiftId);
        String validSaleId = id(post(salesUrl).header("Authorization", bearer(token)));
        mockMvc.perform(post(salesUrl + "/" + validSaleId + "/items").header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedSaleVersion\":0,\"productId\":\"" + productId + "\",\"quantity\":1,\"modifierOptionIds\":[]}"))
                .andExpect(status().isOk());
        String tenCards = "{\"expectedSaleVersion\":1,\"payments\":["
                + "{\"method\":\"CARD\",\"amount\":\"10.0000\"},{\"method\":\"CARD\",\"amount\":\"10.0000\"},"
                + "{\"method\":\"CARD\",\"amount\":\"10.0000\"},{\"method\":\"CARD\",\"amount\":\"10.0000\"},"
                + "{\"method\":\"CARD\",\"amount\":\"10.0000\"},{\"method\":\"CARD\",\"amount\":\"10.0000\"},"
                + "{\"method\":\"CARD\",\"amount\":\"10.0000\"},{\"method\":\"CARD\",\"amount\":\"10.0000\"},"
                + "{\"method\":\"CARD\",\"amount\":\"10.0000\"},{\"method\":\"CARD\",\"amount\":\"10.0000\"}]}";
        mockMvc.perform(post(salesUrl + "/" + validSaleId + "/checkout").header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON).content(tenCards))
                .andExpect(status().isOk());
        assertThat(jdbcTemplate.queryForObject("select count(*) from payments where sale_id = ?", Long.class, UUID.fromString(validSaleId))).isEqualTo(10L);

        String rejectedSaleId = id(post(salesUrl).header("Authorization", bearer(token)));
        mockMvc.perform(post(salesUrl + "/" + rejectedSaleId + "/items").header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedSaleVersion\":0,\"productId\":\"" + productId + "\",\"quantity\":1,\"modifierOptionIds\":[]}"))
                .andExpect(status().isOk());
        String elevenCards = tenCards.replace("\"expectedSaleVersion\":1", "\"expectedSaleVersion\":1")
                .replace("]}", ",{\"method\":\"CARD\",\"amount\":\"1.0000\"}]}");
        mockMvc.perform(post(salesUrl + "/" + rejectedSaleId + "/checkout").header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON).content(elevenCards))
                .andExpect(status().isBadRequest()).andExpect(jsonPath("$.code").value("TOO_MANY_PAYMENTS"));
        mockMvc.perform(post(salesUrl + "/" + rejectedSaleId + "/checkout").header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedSaleVersion\":1,\"payments\":[{\"method\":\"CARD\",\"amount\":\"100.00001\"}]}"))
                .andExpect(status().isBadRequest()).andExpect(jsonPath("$.code").value("INVALID_PAYMENT"));
        assertThat(jdbcTemplate.queryForObject("select count(*) from payments where sale_id = ?", Long.class, UUID.fromString(rejectedSaleId))).isZero();
        assertThat(jdbcTemplate.queryForObject("select version from sales where id = ?", Long.class, UUID.fromString(rejectedSaleId))).isEqualTo(1L);
    }

    @Test
    void paymentReadIsIndependentOfSaleReadAndSurvivesInactiveOperationalParents() throws Exception {
        RbacFixture fixture = fixture();
        List<String> allPermissions = permissionRepository.findAllByOrderByCodeAsc().stream().map(Permission::getCode).toList();
        UUID setupRole = createRoleAndAssign(fixture, "setup-admin", "Setup admin", allPermissions.toArray(String[]::new));
        String setupToken = login(fixture);
        UUID branchId = createBranch(fixture, "checkout-payment-read", setupToken);
        String registerId = createRegister(fixture, branchId, setupToken);
        String shiftId = openShift(fixture, branchId, registerId, setupToken);
        String productId = createProduct(fixture, setupToken, "PAYMENT-READ", "Payment read", "10.0000");
        String salesUrl = salesUrl(fixture, branchId, registerId, shiftId);
        String saleId = id(post(salesUrl).header("Authorization", bearer(setupToken)));
        mockMvc.perform(post(salesUrl + "/" + saleId + "/items").header("Authorization", bearer(setupToken))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedSaleVersion\":0,\"productId\":\"" + productId + "\",\"quantity\":1,\"modifierOptionIds\":[]}"))
                .andExpect(status().isOk());
        String checkoutBody = "{\"expectedSaleVersion\":1,\"payments\":[{\"method\":\"CARD\",\"amount\":\"10.0000\"}]}";
        jdbcTemplate.update("update branches set status = 'INACTIVE' where id = ?", branchId);
        mockMvc.perform(post(salesUrl + "/" + saleId + "/checkout").header("Authorization", bearer(setupToken))
                        .contentType(MediaType.APPLICATION_JSON).content(checkoutBody))
                .andExpect(status().isConflict()).andExpect(jsonPath("$.code").value("BRANCH_INACTIVE"));
        jdbcTemplate.update("update branches set status = 'ACTIVE' where id = ?", branchId);
        jdbcTemplate.update("update cash_registers set status = 'INACTIVE' where id = ?", UUID.fromString(registerId));
        mockMvc.perform(post(salesUrl + "/" + saleId + "/checkout").header("Authorization", bearer(setupToken))
                        .contentType(MediaType.APPLICATION_JSON).content(checkoutBody))
                .andExpect(status().isConflict()).andExpect(jsonPath("$.code").value("CASH_REGISTER_INACTIVE"));
        jdbcTemplate.update("update cash_registers set status = 'ACTIVE' where id = ?", UUID.fromString(registerId));
        mockMvc.perform(post(salesUrl + "/" + saleId + "/checkout").header("Authorization", bearer(setupToken))
                        .contentType(MediaType.APPLICATION_JSON).content(checkoutBody))
                .andExpect(status().isOk());

        unassignRole(fixture, setupRole);
        UUID readerRole = createRoleAndAssign(fixture, "sale-reader", "Sale reader", "sale.read");
        String readerToken = login(fixture);
        mockMvc.perform(get(salesUrl + "/" + saleId).header("Authorization", bearer(readerToken))).andExpect(status().isOk());
        mockMvc.perform(get(salesUrl + "/" + saleId + "/payments").header("Authorization", bearer(readerToken)))
                .andExpect(status().isForbidden()).andExpect(jsonPath("$.code").value("FORBIDDEN"));

        grantPermissionsToRole(readerRole, "payment.read");
        jdbcTemplate.update("update branches set status = 'INACTIVE' where id = ?", branchId);
        jdbcTemplate.update("update cash_registers set status = 'INACTIVE' where id = ?", UUID.fromString(registerId));
        mockMvc.perform(get(salesUrl + "/" + saleId + "/payments").header("Authorization", bearer(readerToken)))
                .andExpect(status().isOk()).andExpect(jsonPath("$.length()").value(1));
    }

    private UUID createBranch(RbacFixture fixture, String code, String token) throws Exception {
        String result = mockMvc.perform(post("/api/v1/organizations/" + fixture.orgId() + "/branches").header("Authorization", bearer(token))
                .contentType(MediaType.APPLICATION_JSON).content("{\"code\":\"" + code + "\",\"name\":\"Branch\"}"))
                .andExpect(status().isCreated()).andReturn().getResponse().getContentAsString();
        return UUID.fromString(objectMapper.readTree(result).get("id").asText());
    }
    private String createRegister(RbacFixture fixture, UUID branchId, String token) throws Exception { return id(post(registersUrl(fixture, branchId)).header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON).content("{\"code\":\"register\",\"name\":\"Register\"}")); }
    private String openShift(RbacFixture fixture, UUID branchId, String registerId, String token) throws Exception { return id(post(shiftsUrl(fixture, branchId, registerId)).header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON).content("{\"openingAmount\":0}")); }
    private String createProduct(RbacFixture fixture, String token, String sku, String name, String price) throws Exception { return id(post("/api/v1/organizations/" + fixture.orgId() + "/products").header("Authorization", bearer(token)).contentType(MediaType.APPLICATION_JSON).content("{\"sku\":\"" + sku + "\",\"name\":\"" + name + "\",\"unitPrice\":" + price + "}")); }
    private String id(org.springframework.test.web.servlet.request.MockHttpServletRequestBuilder request) throws Exception { return objectMapper.readTree(mockMvc.perform(request).andExpect(status().isCreated()).andReturn().getResponse().getContentAsString()).get("id").asText(); }
    private static String registersUrl(RbacFixture fixture, UUID branchId) { return "/api/v1/organizations/" + fixture.orgId() + "/branches/" + branchId + "/cash-registers"; }
    private static String shiftsUrl(RbacFixture fixture, UUID branchId, String registerId) { return registersUrl(fixture, branchId) + "/" + registerId + "/cash-shifts"; }
    private static String salesUrl(RbacFixture fixture, UUID branchId, String registerId, String shiftId) { return shiftsUrl(fixture, branchId, registerId) + "/" + shiftId + "/sales"; }
}
