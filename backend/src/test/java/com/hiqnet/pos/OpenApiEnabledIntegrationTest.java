package com.hiqnet.pos;

import static org.assertj.core.api.Assertions.assertThat;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.get;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import java.util.UUID;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.boot.test.context.SpringBootTest;
import org.springframework.boot.webmvc.test.autoconfigure.AutoConfigureMockMvc;
import org.springframework.context.annotation.Import;
import org.springframework.test.context.ActiveProfiles;
import org.springframework.test.web.servlet.MockMvc;
import tools.jackson.databind.JsonNode;
import tools.jackson.databind.ObjectMapper;

/**
 * OpenAPI exposure policy (enabled). With {@code app.security.permit-opendoc=true}
 * the generated spec and the Swagger UI are reachable through the REAL security
 * chain, the spec describes the real contract (global metadata, representative
 * operations, decimal-string quantities and the bearer scheme) and the business
 * endpoints keep requiring authentication (deny-by-default is preserved).
 */
@ActiveProfiles("test")
@Import({TestcontainersConfiguration.class})
@SpringBootTest(properties = "app.security.permit-opendoc=true")
@AutoConfigureMockMvc
class OpenApiEnabledIntegrationTest {

    private static final String INVENTORY_ITEMS_PATH =
            "/api/v1/organizations/{organizationId}/branches/{branchId}/inventory-items";

    private static final String SUPPLIERS_PATH = "/api/v1/organizations/{organizationId}/suppliers";

    private static final String PURCHASE_ORDERS_PATH =
            "/api/v1/organizations/{organizationId}/branches/{branchId}/purchase-orders";

    private static final String CHECKOUT_PATH =
            "/api/v1/organizations/{organizationId}/branches/{branchId}/cash-registers/{cashRegisterId}"
                    + "/cash-shifts/{cashShiftId}/sales/{saleId}/checkout";

    @Autowired
    private MockMvc mockMvc;

    @Autowired
    private ObjectMapper objectMapper;

    private JsonNode apiDocs() throws Exception {
        String json = mockMvc.perform(get("/v3/api-docs"))
                .andExpect(status().isOk())
                .andReturn()
                .getResponse()
                .getContentAsString(StandardCharsets.UTF_8);
        return objectMapper.readTree(json);
    }

    @Test
    void openApiSpecIsServedWithGlobalMetadataAndBearerScheme() throws Exception {
        JsonNode docs = apiDocs();
        assertThat(docs.path("info").path("title").asText()).isEqualTo("HiQNet POS API");
        assertThat(docs.path("info").path("version").asText()).isNotBlank();
        JsonNode bearer = docs.path("components").path("securitySchemes").path("bearerAuth");
        assertThat(bearer.path("type").asText()).isEqualTo("http");
        assertThat(bearer.path("scheme").asText()).isEqualTo("bearer");
        assertThat(bearer.path("bearerFormat").asText()).isEqualTo("JWT");
    }

    @Test
    void openApiAppliesGlobalBearerAndOptsOutPublicOperations() throws Exception {
        JsonNode docs = apiDocs();
        // Global requirement: every operation requires bearerAuth unless it opts out.
        assertThat(docs.path("security").isArray()).isTrue();
        assertThat(docs.path("security")).isNotEmpty();
        assertThat(docs.path("security").get(0).has("bearerAuth")).isTrue();
        // The public auth endpoints opt out explicitly (empty security).
        for (String op : new String[] {"login", "refresh", "logout"}) {
            JsonNode security = docs.path("paths").path("/api/v1/auth/" + op).path("post").path("security");
            assertThat(security.isArray()).as("security of " + op).isTrue();
            assertThat(security).as("public endpoint " + op + " must not require auth").isEmpty();
        }
    }

    @Test
    void openApiSpecContainsRepresentativeOperations() throws Exception {
        JsonNode paths = apiDocs().path("paths");
        assertThat(paths.has("/api/v1/auth/login")).isTrue();
        assertThat(paths.path("/api/v1/auth/login").has("post")).isTrue();
        assertThat(paths.has(INVENTORY_ITEMS_PATH)).isTrue();
        assertThat(paths.path(INVENTORY_ITEMS_PATH).has("get")).isTrue();
        assertThat(paths.has(SUPPLIERS_PATH)).isTrue();
        assertThat(paths.path(SUPPLIERS_PATH).has("post")).isTrue();
        assertThat(paths.has(PURCHASE_ORDERS_PATH)).isTrue();
        assertThat(paths.path(PURCHASE_ORDERS_PATH).has("post")).isTrue();
        assertThat(paths.path(PURCHASE_ORDERS_PATH + "/{purchaseOrderId}/lines").has("post")).isTrue();
        assertThat(paths.path(PURCHASE_ORDERS_PATH + "/{purchaseOrderId}/lines/{purchaseOrderLineId}")
                        .has("patch"))
                .isTrue();
        assertThat(paths.path(PURCHASE_ORDERS_PATH + "/{purchaseOrderId}/issue").has("post")).isTrue();
        assertThat(paths.path(PURCHASE_ORDERS_PATH + "/{purchaseOrderId}/cancel").has("post")).isTrue();
        assertThat(paths.has(CHECKOUT_PATH)).isTrue();
        assertThat(paths.path(CHECKOUT_PATH).has("post")).isTrue();
    }

    @Test
    void openApiSpecDocumentsDecimalStringContract() throws Exception {
        JsonNode schemas = apiDocs().path("components").path("schemas");
        assertThat(schemas.path("OpenInventoryStockRequest").path("properties").path("quantity").path("type").asText())
                .isEqualTo("string");
        assertThat(
                        schemas.path("AdjustInventoryStockRequest")
                                .path("properties")
                                .path("quantityDelta")
                                .path("type")
                                .asText())
                .isEqualTo("string");
        assertThat(
                        schemas.path("InventoryStockResponse")
                                .path("properties")
                                .path("quantityOnHand")
                                .path("type")
                                .asText())
                .isEqualTo("string");
        assertNullableDecimalString(
                schemas.path("UpdatePurchaseOrderLineRequest").path("properties").path("unitCost"));
        assertNullableDecimalString(
                schemas.path("CreatePurchaseOrderLineRequest").path("properties").path("unitCost"));
        assertNullableDecimalString(
                schemas.path("PurchaseOrderLineResponse").path("properties").path("unitCost"));
        List<String> updateLineRequired = new ArrayList<>();
        schemas.path("UpdatePurchaseOrderLineRequest")
                .path("required")
                .forEach(node -> updateLineRequired.add(node.asText()));
        assertThat(updateLineRequired).doesNotContain("unitCost");
    }

    /**
     * Un costo unitario opcional se documenta como la unión de tipos de OpenAPI 3.1
     * {@code ["string", "null"]}, de modo que el contrato permita formalmente el
     * {@code null} que limpia el valor.
     */
    private static void assertNullableDecimalString(JsonNode schema) {
        assertThat(schema.path("type").isArray())
                .as("un decimal nullable debe declarar la unión de tipos de OpenAPI 3.1")
                .isTrue();
        List<String> types = new ArrayList<>();
        schema.path("type").forEach(node -> types.add(node.asText()));
        assertThat(types).containsExactlyInAnyOrder("string", "null");
    }

    @Test
    void openApiDocumentsTransitionVersionRequestSchema() throws Exception {
        JsonNode schemas = apiDocs().path("components").path("schemas");
        JsonNode request = schemas.path("ExpectedPurchaseOrderVersionRequest");
        assertThat(request.isMissingNode()).as("transition request schema must be registered").isFalse();
        assertThat(request.path("properties").path("expectedVersion").path("type").asText())
                .isEqualTo("integer");
        List<String> required = new ArrayList<>();
        request.path("required").forEach(node -> required.add(node.asText()));
        assertThat(required).contains("expectedVersion");
    }

    @Test
    void openApiDocumentsInventoryItemUnitLockAndImmutableUnitError() throws Exception {
        JsonNode docs = apiDocs();
        JsonNode schemas = docs.path("components").path("schemas");
        assertThat(schemas.path("InventoryItemResponse").path("properties").path("unitLocked").path("type").asText())
                .isEqualTo("boolean");
        JsonNode updateResponses = docs.path("paths")
                .path("/api/v1/organizations/{organizationId}/inventory-items/{inventoryItemId}")
                .path("patch")
                .path("responses");
        assertApiErrorRef(updateResponses.path("409"));
        assertThat(docs.path("paths")
                        .path("/api/v1/organizations/{organizationId}/inventory-items/{inventoryItemId}")
                        .path("patch")
                        .path("description")
                        .asText())
                .contains("INVENTORY_ITEM_UNIT_LOCKED");
    }

    @Test
    void swaggerUiIsServedAlongsideTheSpec() throws Exception {
        mockMvc.perform(get("/swagger-ui/index.html")).andExpect(status().isOk());
        // Canonical springdoc entry: redirects to the UI served above.
        mockMvc.perform(get("/swagger-ui.html")).andExpect(status().is3xxRedirection());
    }

    @Test
    void businessEndpointsStillRequireAuthentication() throws Exception {
        mockMvc.perform(get(INVENTORY_ITEMS_PATH, UUID.randomUUID(), UUID.randomUUID()))
                .andExpect(status().isUnauthorized());
    }

    @Test
    void openApiSpecExposesReusableApiErrorSchema() throws Exception {
        JsonNode apiError = apiDocs().path("components").path("schemas").path("ApiError");
        assertThat(apiError.isMissingNode()).as("ApiError schema must be registered").isFalse();
        JsonNode properties = apiError.path("properties");
        for (String field : new String[] {"timestamp", "status", "code", "message", "path", "details"}) {
            assertThat(properties.has(field)).as("ApiError." + field).isTrue();
        }
        assertThat(properties.path("status").path("type").asText()).isEqualTo("integer");
        assertThat(properties.path("details").path("type").asText()).isEqualTo("array");
    }

    @Test
    void openApiErrorResponsesReferenceApiErrorSchema() throws Exception {
        JsonNode paths = apiDocs().path("paths");
        // Auth: public login declares 400/401.
        assertApiErrorRef(paths.path("/api/v1/auth/login").path("post").path("responses").path("401"));
        // Business admin: 409 (duplicate) and 403 (forbidden).
        JsonNode usersResponses = paths.path("/api/v1/organizations/{organizationId}/users")
                .path("post")
                .path("responses");
        assertApiErrorRef(usersResponses.path("409"));
        assertApiErrorRef(usersResponses.path("403"));
        JsonNode suppliersResponses = paths.path(SUPPLIERS_PATH).path("post").path("responses");
        assertApiErrorRef(suppliersResponses.path("409"));
        assertApiErrorRef(suppliersResponses.path("403"));
        JsonNode orderResponses = paths.path(PURCHASE_ORDERS_PATH).path("post").path("responses");
        assertApiErrorRef(orderResponses.path("409"));
        assertApiErrorRef(orderResponses.path("403"));
        JsonNode issueResponses = paths.path(PURCHASE_ORDERS_PATH + "/{purchaseOrderId}/issue")
                .path("post")
                .path("responses");
        assertApiErrorRef(issueResponses.path("409"));
        assertApiErrorRef(issueResponses.path("403"));
        JsonNode cancelResponses = paths.path(PURCHASE_ORDERS_PATH + "/{purchaseOrderId}/cancel")
                .path("post")
                .path("responses");
        assertApiErrorRef(cancelResponses.path("409"));
        assertApiErrorRef(cancelResponses.path("403"));
    }

    private static void assertApiErrorRef(JsonNode response) {
        assertThat(response.isMissingNode()).as("error response must be documented").isFalse();
        assertThat(response.path("content").path("application/json").path("schema").path("$ref").asText())
                .isEqualTo("#/components/schemas/ApiError");
    }
}
