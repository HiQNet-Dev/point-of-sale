package com.hiqnet.pos.sales;

import static org.assertj.core.api.Assertions.assertThat;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.get;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.jsonPath;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import com.hiqnet.pos.AbstractRbacIntegrationTest;
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
