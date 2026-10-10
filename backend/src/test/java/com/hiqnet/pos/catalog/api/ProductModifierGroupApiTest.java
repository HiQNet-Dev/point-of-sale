package com.hiqnet.pos.catalog.api;

import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.get;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.patch;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.put;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.jsonPath;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import com.hiqnet.pos.AbstractRbacIntegrationTest;
import com.hiqnet.pos.catalog.domain.ModifierGroup;
import com.hiqnet.pos.catalog.domain.ModifierGroupRepository;
import com.hiqnet.pos.catalog.domain.Product;
import com.hiqnet.pos.catalog.domain.ProductRepository;
import com.hiqnet.pos.organization.domain.Organization;
import com.hiqnet.pos.organization.domain.OrganizationRepository;
import java.math.BigDecimal;
import java.util.UUID;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.http.MediaType;

/**
 * Product -> modifier-group configuration: PUT replaces the ordered list, the
 * product's version is the concurrency boundary, a no-op list does not bump the
 * version, and reading requires {@code product.read} while replacing requires
 * {@code product.modifiers.manage}.
 */
class ProductModifierGroupApiTest extends AbstractRbacIntegrationTest {

    @Autowired
    private OrganizationRepository organizationRepository;

    @Autowired
    private ProductRepository productRepository;

    @Autowired
    private ModifierGroupRepository modifierGroupRepository;

    private UUID productEntity(RbacFixture fx, String sku) {
        Organization organization = organizationRepository.findById(fx.orgId()).orElseThrow();
        return productRepository
                .saveAndFlush(Product.create(organization, sku, "Product " + sku, new BigDecimal("5.0000")))
                .getId();
    }

    private UUID modifierGroupEntity(RbacFixture fx, String code) {
        Organization organization = organizationRepository.findById(fx.orgId()).orElseThrow();
        return modifierGroupRepository
                .saveAndFlush(ModifierGroup.create(organization, code, "Group " + code, 0, 1))
                .getId();
    }

    private String createProduct(String organizationId, String token, String sku) throws Exception {
        String response = mockMvc.perform(post("/api/v1/organizations/" + organizationId + "/products")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"sku\":\"" + sku + "\",\"name\":\"" + sku + "\",\"unitPrice\":10.0000}"))
                .andExpect(status().isCreated())
                .andReturn()
                .getResponse()
                .getContentAsString();
        return objectMapper.readTree(response).get("id").asText();
    }

    private String createGroup(String organizationId, String token, String code) throws Exception {
        String response = mockMvc.perform(post("/api/v1/organizations/" + organizationId + "/modifier-groups")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"" + code + "\",\"name\":\"Group " + code + "\"}"))
                .andExpect(status().isCreated())
                .andReturn()
                .getResponse()
                .getContentAsString();
        return objectMapper.readTree(response).get("id").asText();
    }

    private String assignmentUrl(String organizationId, String productId) {
        return "/api/v1/organizations/" + organizationId + "/products/" + productId + "/modifier-groups";
    }

    private long productVersion(String organizationId, String productId, String token) throws Exception {
        String response = mockMvc.perform(get("/api/v1/organizations/" + organizationId + "/products/" + productId)
                        .header("Authorization", bearer(token)))
                .andExpect(status().isOk())
                .andReturn()
                .getResponse()
                .getContentAsString();
        return objectMapper.readTree(response).get("version").asLong();
    }

    @Test
    void listAssignmentsIsEmptyInitially() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String productId = createProduct(fx.orgId().toString(), token, "LATTE");
        mockMvc.perform(get(assignmentUrl(fx.orgId().toString(), productId))
                        .header("Authorization", bearer(token)))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.length()").value(0));
    }

    @Test
    void replaceAssignsGroupsInOrder() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String productId = createProduct(fx.orgId().toString(), token, "LATTE");
        String milk = createGroup(fx.orgId().toString(), token, "milk");
        String size = createGroup(fx.orgId().toString(), token, "size");
        mockMvc.perform(put(assignmentUrl(fx.orgId().toString(), productId))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"modifierGroupIds\":[\"" + size + "\",\"" + milk + "\"]}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.length()").value(2))
                .andExpect(jsonPath("$[0].modifierGroupId").value(size))
                .andExpect(jsonPath("$[0].displayOrder").value(0))
                .andExpect(jsonPath("$[1].modifierGroupId").value(milk))
                .andExpect(jsonPath("$[1].displayOrder").value(1));
    }

    @Test
    void replaceBumpsProductVersion() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String productId = createProduct(fx.orgId().toString(), token, "LATTE");
        String milk = createGroup(fx.orgId().toString(), token, "milk");
        mockMvc.perform(put(assignmentUrl(fx.orgId().toString(), productId))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"modifierGroupIds\":[\"" + milk + "\"]}"))
                .andExpect(status().isOk());
        org.assertj.core.api.Assertions.assertThat(productVersion(fx.orgId().toString(), productId, token))
                .isEqualTo(1);
    }

    @Test
    void replaceWithSameListIsNoOpAndDoesNotBumpVersion() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String productId = createProduct(fx.orgId().toString(), token, "LATTE");
        String milk = createGroup(fx.orgId().toString(), token, "milk");
        String body = "{\"expectedVersion\":0,\"modifierGroupIds\":[\"" + milk + "\"]}";
        mockMvc.perform(put(assignmentUrl(fx.orgId().toString(), productId))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(body))
                .andExpect(status().isOk());
        mockMvc.perform(put(assignmentUrl(fx.orgId().toString(), productId))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":1,\"modifierGroupIds\":[\"" + milk + "\"]}"))
                .andExpect(status().isOk());
        org.assertj.core.api.Assertions.assertThat(productVersion(fx.orgId().toString(), productId, token))
                .isEqualTo(1);
    }

    @Test
    void replaceReorderingSameSetBumpsVersion() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String productId = createProduct(fx.orgId().toString(), token, "LATTE");
        String milk = createGroup(fx.orgId().toString(), token, "milk");
        String size = createGroup(fx.orgId().toString(), token, "size");
        mockMvc.perform(put(assignmentUrl(fx.orgId().toString(), productId))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"modifierGroupIds\":[\"" + milk + "\",\"" + size + "\"]}"))
                .andExpect(status().isOk());
        mockMvc.perform(put(assignmentUrl(fx.orgId().toString(), productId))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":1,\"modifierGroupIds\":[\"" + size + "\",\"" + milk + "\"]}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$[0].modifierGroupId").value(size));
        org.assertj.core.api.Assertions.assertThat(productVersion(fx.orgId().toString(), productId, token))
                .isEqualTo(2);
    }

    @Test
    void replaceWithEmptyListClearsConfiguration() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String productId = createProduct(fx.orgId().toString(), token, "LATTE");
        String milk = createGroup(fx.orgId().toString(), token, "milk");
        mockMvc.perform(put(assignmentUrl(fx.orgId().toString(), productId))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"modifierGroupIds\":[\"" + milk + "\"]}"))
                .andExpect(status().isOk());
        mockMvc.perform(put(assignmentUrl(fx.orgId().toString(), productId))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":1,\"modifierGroupIds\":[]}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.length()").value(0));
    }

    @Test
    void replaceWithDuplicateIdsIs400() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String productId = createProduct(fx.orgId().toString(), token, "LATTE");
        String milk = createGroup(fx.orgId().toString(), token, "milk");
        mockMvc.perform(put(assignmentUrl(fx.orgId().toString(), productId))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"modifierGroupIds\":[\"" + milk + "\",\"" + milk + "\"]}"))
                .andExpect(status().isBadRequest())
                .andExpect(jsonPath("$.code").value("VALIDATION_ERROR"));
    }

    @Test
    void replaceWithUnknownGroupIs404() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String productId = createProduct(fx.orgId().toString(), token, "LATTE");
        mockMvc.perform(put(assignmentUrl(fx.orgId().toString(), productId))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"modifierGroupIds\":[\"" + UUID.randomUUID() + "\"]}"))
                .andExpect(status().isNotFound())
                .andExpect(jsonPath("$.code").value("MODIFIER_GROUP_NOT_FOUND"));
    }

    @Test
    void replaceWithGroupOfAnotherOrganizationIs404() throws Exception {
        RbacFixture fxA = fixture();
        RbacFixture fxB = fixture();
        String tokenA = adminToken(fxA);
        String tokenB = adminToken(fxB);
        String productId = createProduct(fxA.orgId().toString(), tokenA, "LATTE");
        String groupB = createGroup(fxB.orgId().toString(), tokenB, "milk");
        mockMvc.perform(put(assignmentUrl(fxA.orgId().toString(), productId))
                        .header("Authorization", bearer(tokenA))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"modifierGroupIds\":[\"" + groupB + "\"]}"))
                .andExpect(status().isNotFound())
                .andExpect(jsonPath("$.code").value("MODIFIER_GROUP_NOT_FOUND"));
    }

    @Test
    void replaceProductOfAnotherOrganizationIs404() throws Exception {
        RbacFixture fxA = fixture();
        RbacFixture fxB = fixture();
        String tokenA = adminToken(fxA);
        String tokenB = adminToken(fxB);
        String productB = createProduct(fxB.orgId().toString(), tokenB, "LATTE");
        String groupA = createGroup(fxA.orgId().toString(), tokenA, "milk");
        mockMvc.perform(put(assignmentUrl(fxA.orgId().toString(), productB))
                        .header("Authorization", bearer(tokenA))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"modifierGroupIds\":[\"" + groupA + "\"]}"))
                .andExpect(status().isNotFound())
                .andExpect(jsonPath("$.code").value("PRODUCT_NOT_FOUND"));
    }

    @Test
    void replaceStaleExpectedVersionIs409() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String productId = createProduct(fx.orgId().toString(), token, "LATTE");
        String milk = createGroup(fx.orgId().toString(), token, "milk");
        mockMvc.perform(patch("/api/v1/organizations/" + fx.orgId() + "/products/" + productId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"name\":\"Latte grande\"}"))
                .andExpect(status().isOk());
        mockMvc.perform(put(assignmentUrl(fx.orgId().toString(), productId))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"modifierGroupIds\":[\"" + milk + "\"]}"))
                .andExpect(status().isConflict())
                .andExpect(jsonPath("$.code").value("CONCURRENT_MODIFICATION"));
    }

    @Test
    void replaceWithInactiveGroupSucceeds() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String productId = createProduct(fx.orgId().toString(), token, "LATTE");
        String milk = createGroup(fx.orgId().toString(), token, "milk");
        mockMvc.perform(patch("/api/v1/organizations/" + fx.orgId() + "/modifier-groups/" + milk)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"status\":\"INACTIVE\"}"))
                .andExpect(status().isOk());
        mockMvc.perform(put(assignmentUrl(fx.orgId().toString(), productId))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"modifierGroupIds\":[\"" + milk + "\"]}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$[0].status").value("INACTIVE"));
    }

    @Test
    void deactivatingGroupKeepsOptionsAndAssignmentWithoutBumpingProduct() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String productId = createProduct(fx.orgId().toString(), token, "LATTE");
        String milk = createGroup(fx.orgId().toString(), token, "milk");

        String optionResponse = mockMvc.perform(post("/api/v1/organizations/" + fx.orgId()
                                + "/modifier-groups/" + milk + "/options")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"oat\",\"name\":\"Oat\",\"priceDelta\":0,\"displayOrder\":0}"))
                .andExpect(status().isCreated())
                .andReturn()
                .getResponse()
                .getContentAsString();
        String optionId = objectMapper.readTree(optionResponse).get("id").asText();

        mockMvc.perform(put(assignmentUrl(fx.orgId().toString(), productId))
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"modifierGroupIds\":[\"" + milk + "\"]}"))
                .andExpect(status().isOk());

        // Retiring the group must NOT cascade: the option stays ACTIVE, the
        // product assignment stays, and the product itself is untouched.
        mockMvc.perform(patch("/api/v1/organizations/" + fx.orgId() + "/modifier-groups/" + milk)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"status\":\"INACTIVE\"}"))
                .andExpect(status().isOk());
        mockMvc.perform(get("/api/v1/organizations/" + fx.orgId() + "/modifier-groups/" + milk
                                + "/options/" + optionId)
                        .header("Authorization", bearer(token)))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.status").value("ACTIVE"));
        mockMvc.perform(get(assignmentUrl(fx.orgId().toString(), productId))
                        .header("Authorization", bearer(token)))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.length()").value(1))
                .andExpect(jsonPath("$[0].modifierGroupId").value(milk));
        org.assertj.core.api.Assertions.assertThat(productVersion(fx.orgId().toString(), productId, token))
                .isEqualTo(1);
    }

    @Test
    void replaceWithoutManagePermissionIsForbidden() throws Exception {
        RbacFixture fx = fixture();
        String productId = productEntity(fx, "LATTE").toString();
        String milk = modifierGroupEntity(fx, "milk").toString();
        String readToken = login(fx);
        mockMvc.perform(put(assignmentUrl(fx.orgId().toString(), productId))
                        .header("Authorization", bearer(readToken))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"modifierGroupIds\":[\"" + milk + "\"]}"))
                .andExpect(status().isForbidden())
                .andExpect(jsonPath("$.code").value("FORBIDDEN"));
    }

    @Test
    void listWithoutProductReadIsForbidden() throws Exception {
        RbacFixture fx = fixture();
        String productId = productEntity(fx, "LATTE").toString();
        String readToken = login(fx);
        mockMvc.perform(get(assignmentUrl(fx.orgId().toString(), productId))
                        .header("Authorization", bearer(readToken)))
                .andExpect(status().isForbidden())
                .andExpect(jsonPath("$.code").value("FORBIDDEN"));
    }
}
