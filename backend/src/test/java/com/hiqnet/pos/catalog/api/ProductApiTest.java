package com.hiqnet.pos.catalog.api;

import static org.hamcrest.Matchers.nullValue;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.get;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.patch;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.jsonPath;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import com.hiqnet.pos.AbstractRbacIntegrationTest;
import java.util.UUID;
import org.junit.jupiter.api.Test;
import org.springframework.http.MediaType;

/**
 * Product administration under the REAL security chain + RBAC
 * (product.create / product.read / product.update): category assignment is
 * same-organization and ACTIVE-only, sku is immutable, and {@code categoryId}
 * on PATCH is tri-state (omit / null / value).
 */
class ProductApiTest extends AbstractRbacIntegrationTest {

    private String createCategory(String organizationId, String token, String code, String name)
            throws Exception {
        String response = mockMvc.perform(post("/api/v1/organizations/" + organizationId + "/categories")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(objectMapper.writeValueAsString(new CreateCategoryRequest(code, name))))
                .andExpect(status().isCreated())
                .andReturn()
                .getResponse()
                .getContentAsString();
        return objectMapper.readTree(response).get("id").asText();
    }

    private String createProduct(String organizationId, String token, String sku, String name, String categoryId)
            throws Exception {
        String body = "{"
                + "\"sku\":\"" + sku + "\","
                + "\"name\":\"" + name + "\","
                + "\"unitPrice\":10.0000"
                + (categoryId == null ? "" : ",\"categoryId\":\"" + categoryId + "\"")
                + "}";
        String response = mockMvc.perform(post("/api/v1/organizations/" + organizationId + "/products")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(body))
                .andExpect(status().isCreated())
                .andReturn()
                .getResponse()
                .getContentAsString();
        return objectMapper.readTree(response).get("id").asText();
    }

    @Test
    void createProductWithCategoryReturns201AndDto() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String categoryId = createCategory(fx.orgId().toString(), token, "bebidas", "Bebidas");
        mockMvc.perform(post("/api/v1/organizations/" + fx.orgId() + "/products")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"sku\":\"LATTE-12OZ\",\"barcode\":\"7501234567890\",\"name\":\"Latte 12oz\","
                                + "\"description\":\"Café con leche\",\"categoryId\":\"" + categoryId + "\","
                                + "\"unitPrice\":45.5000}"))
                .andExpect(status().isCreated())
                .andExpect(jsonPath("$.id").isNotEmpty())
                .andExpect(jsonPath("$.organizationId").value(fx.orgId().toString()))
                .andExpect(jsonPath("$.categoryId").value(categoryId))
                .andExpect(jsonPath("$.sku").value("LATTE-12OZ"))
                .andExpect(jsonPath("$.barcode").value("7501234567890"))
                .andExpect(jsonPath("$.name").value("Latte 12oz"))
                .andExpect(jsonPath("$.unitPrice").value(45.5))
                .andExpect(jsonPath("$.status").value("ACTIVE"))
                .andExpect(jsonPath("$.version").value(0));
    }

    @Test
    void createProductWithoutCategoryHasNullCategoryId() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        mockMvc.perform(post("/api/v1/organizations/" + fx.orgId() + "/products")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"sku\":\"AGUA\",\"name\":\"Agua\",\"unitPrice\":5.0000}"))
                .andExpect(status().isCreated())
                .andExpect(jsonPath("$.categoryId").value(nullValue()))
                .andExpect(jsonPath("$.barcode").value(nullValue()));
    }

    @Test
    void createProductWithoutPermissionIsForbidden() throws Exception {
        RbacFixture fx = fixture();
        String token = login(fx);
        mockMvc.perform(post("/api/v1/organizations/" + fx.orgId() + "/products")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"sku\":\"AGUA\",\"name\":\"Agua\",\"unitPrice\":5.0000}"))
                .andExpect(status().isForbidden())
                .andExpect(jsonPath("$.code").value("FORBIDDEN"));
    }

    @Test
    void createProductNormalizesSkuToUppercase() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        mockMvc.perform(post("/api/v1/organizations/" + fx.orgId() + "/products")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"sku\":\"latte-12oz\",\"name\":\"Latte\",\"unitPrice\":5.0000}"))
                .andExpect(status().isCreated())
                .andExpect(jsonPath("$.sku").value("LATTE-12OZ"));
    }

    @Test
    void createProductRejectsDuplicateSkuWithinSameOrganization() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        createProduct(fx.orgId().toString(), token, "AGUA", "Agua", null);
        mockMvc.perform(post("/api/v1/organizations/" + fx.orgId() + "/products")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"sku\":\"agua\",\"name\":\"Otra\",\"unitPrice\":5.0000}"))
                .andExpect(status().isConflict())
                .andExpect(jsonPath("$.code").value("PRODUCT_SKU_ALREADY_EXISTS"));
    }

    @Test
    void createProductRejectsDuplicateBarcodeWithinSameOrganization() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        mockMvc.perform(post("/api/v1/organizations/" + fx.orgId() + "/products")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"sku\":\"AGUA\",\"barcode\":\"111\",\"name\":\"Agua\",\"unitPrice\":5.0000}"))
                .andExpect(status().isCreated());
        mockMvc.perform(post("/api/v1/organizations/" + fx.orgId() + "/products")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"sku\":\"JUGO\",\"barcode\":\"111\",\"name\":\"Jugo\",\"unitPrice\":5.0000}"))
                .andExpect(status().isConflict())
                .andExpect(jsonPath("$.code").value("PRODUCT_BARCODE_ALREADY_EXISTS"));
    }

    @Test
    void createProductAllowsTwoProductsWithoutBarcode() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        createProduct(fx.orgId().toString(), token, "AGUA", "Agua", null);
        mockMvc.perform(post("/api/v1/organizations/" + fx.orgId() + "/products")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"sku\":\"JUGO\",\"name\":\"Jugo\",\"unitPrice\":5.0000}"))
                .andExpect(status().isCreated())
                .andExpect(jsonPath("$.barcode").value(nullValue()));
    }

    @Test
    void createProductWithInactiveCategoryIs409() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String categoryId = createCategory(fx.orgId().toString(), token, "bebidas", "Bebidas");
        mockMvc.perform(patch("/api/v1/organizations/" + fx.orgId() + "/categories/" + categoryId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"status\":\"INACTIVE\"}"))
                .andExpect(status().isOk());
        mockMvc.perform(post("/api/v1/organizations/" + fx.orgId() + "/products")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"sku\":\"AGUA\",\"name\":\"Agua\",\"categoryId\":\"" + categoryId + "\","
                                + "\"unitPrice\":5.0000}"))
                .andExpect(status().isConflict())
                .andExpect(jsonPath("$.code").value("CATEGORY_INACTIVE"));
    }

    @Test
    void createProductWithCategoryOfAnotherOrganizationIs404() throws Exception {
        RbacFixture fxA = fixture();
        RbacFixture fxB = fixture();
        String tokenA = adminToken(fxA);
        String tokenB = adminToken(fxB);
        String categoryB = createCategory(fxB.orgId().toString(), tokenB, "bebidas", "Bebidas");
        mockMvc.perform(post("/api/v1/organizations/" + fxA.orgId() + "/products")
                        .header("Authorization", bearer(tokenA))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"sku\":\"AGUA\",\"name\":\"Agua\",\"categoryId\":\"" + categoryB + "\","
                                + "\"unitPrice\":5.0000}"))
                .andExpect(status().isNotFound())
                .andExpect(jsonPath("$.code").value("CATEGORY_NOT_FOUND"));
    }

    @Test
    void createProductRejectsNegativeUnitPrice() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        mockMvc.perform(post("/api/v1/organizations/" + fx.orgId() + "/products")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"sku\":\"AGUA\",\"name\":\"Agua\",\"unitPrice\":-1.0000}"))
                .andExpect(status().isBadRequest())
                .andExpect(jsonPath("$.code").value("VALIDATION_ERROR"));
    }

    @Test
    void listProductsFiltersByStatusCategoryAndQuery() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String categoryId = createCategory(fx.orgId().toString(), token, "bebidas", "Bebidas");
        createProduct(fx.orgId().toString(), token, "AMERICANO", "Café Americano", categoryId);
        createProduct(fx.orgId().toString(), token, "TE", "Té", null);

        mockMvc.perform(get("/api/v1/organizations/" + fx.orgId() + "/products")
                        .header("Authorization", bearer(token))
                        .param("categoryId", categoryId))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.items.length()").value(1))
                .andExpect(jsonPath("$.items[0].sku").value("AMERICANO"));

        mockMvc.perform(get("/api/v1/organizations/" + fx.orgId() + "/products")
                        .header("Authorization", bearer(token))
                        .param("q", "amer"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.items.length()").value(1))
                .andExpect(jsonPath("$.items[0].name").value("Café Americano"));
    }

    @Test
    void listProductsRejectsCategoryFilterFromAnotherOrganization() throws Exception {
        RbacFixture fxA = fixture();
        RbacFixture fxB = fixture();
        String tokenA = adminToken(fxA);
        String tokenB = adminToken(fxB);
        String categoryB = createCategory(fxB.orgId().toString(), tokenB, "bebidas", "Bebidas");
        mockMvc.perform(get("/api/v1/organizations/" + fxA.orgId() + "/products")
                        .header("Authorization", bearer(tokenA))
                        .param("categoryId", categoryB))
                .andExpect(status().isNotFound())
                .andExpect(jsonPath("$.code").value("CATEGORY_NOT_FOUND"));
    }

    @Test
    void patchProductUpdatesFields() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String productId = createProduct(fx.orgId().toString(), token, "AGUA", "Agua", null);
        mockMvc.perform(patch("/api/v1/organizations/" + fx.orgId() + "/products/" + productId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"name\":\"Agua Mineral\",\"description\":\"Sin gas\","
                                + "\"unitPrice\":7.5000,\"status\":\"INACTIVE\"}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.name").value("Agua Mineral"))
                .andExpect(jsonPath("$.description").value("Sin gas"))
                .andExpect(jsonPath("$.unitPrice").value(7.5))
                .andExpect(jsonPath("$.status").value("INACTIVE"))
                .andExpect(jsonPath("$.sku").value("AGUA"))
                .andExpect(jsonPath("$.version").value(1));
    }

    @Test
    void patchProductSetsThenClearsCategoryWithNull() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String categoryId = createCategory(fx.orgId().toString(), token, "bebidas", "Bebidas");
        String productId = createProduct(fx.orgId().toString(), token, "AGUA", "Agua", null);

        mockMvc.perform(patch("/api/v1/organizations/" + fx.orgId() + "/products/" + productId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"categoryId\":\"" + categoryId + "\"}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.categoryId").value(categoryId))
                .andExpect(jsonPath("$.version").value(1));

        mockMvc.perform(patch("/api/v1/organizations/" + fx.orgId() + "/products/" + productId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":1,\"categoryId\":null}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.categoryId").value(nullValue()))
                .andExpect(jsonPath("$.version").value(2));
    }

    @Test
    void patchProductOmittingCategoryKeepsIt() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String categoryId = createCategory(fx.orgId().toString(), token, "bebidas", "Bebidas");
        String productId = createProduct(fx.orgId().toString(), token, "AGUA", "Agua", categoryId);
        mockMvc.perform(patch("/api/v1/organizations/" + fx.orgId() + "/products/" + productId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"name\":\"Agua Renombrada\"}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.categoryId").value(categoryId))
                .andExpect(jsonPath("$.categoryId").isNotEmpty());
    }

    @Test
    void patchProductIgnoresSkuInBody() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String productId = createProduct(fx.orgId().toString(), token, "AGUA", "Agua", null);
        mockMvc.perform(patch("/api/v1/organizations/" + fx.orgId() + "/products/" + productId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"sku\":\"HACKED\",\"name\":\"Agua 2\"}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.sku").value("AGUA"))
                .andExpect(jsonPath("$.name").value("Agua 2"));
    }

    @Test
    void patchProductStaleExpectedVersionIs409() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String productId = createProduct(fx.orgId().toString(), token, "AGUA", "Agua", null);
        mockMvc.perform(patch("/api/v1/organizations/" + fx.orgId() + "/products/" + productId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"name\":\"Agua 2\"}"))
                .andExpect(status().isOk());
        mockMvc.perform(patch("/api/v1/organizations/" + fx.orgId() + "/products/" + productId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"name\":\"Stale\"}"))
                .andExpect(status().isConflict())
                .andExpect(jsonPath("$.code").value("CONCURRENT_MODIFICATION"));
    }
}
