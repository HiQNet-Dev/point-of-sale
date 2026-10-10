package com.hiqnet.pos.catalog.api;

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
 * Category administration under the REAL security chain + RBAC
 * (category.create / category.read / category.update) and expectedVersion on
 * PATCH. The catalog is organization-owned; a category never resolves through
 * another organization.
 */
class CategoryApiTest extends AbstractRbacIntegrationTest {

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

    @Test
    void createCategoryReturns201AndDto() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        mockMvc.perform(post("/api/v1/organizations/" + fx.orgId() + "/categories")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(objectMapper.writeValueAsString(
                                new CreateCategoryRequest("bebidas-frias", "Bebidas Frías"))))
                .andExpect(status().isCreated())
                .andExpect(jsonPath("$.id").isNotEmpty())
                .andExpect(jsonPath("$.organizationId").value(fx.orgId().toString()))
                .andExpect(jsonPath("$.code").value("bebidas-frias"))
                .andExpect(jsonPath("$.name").value("Bebidas Frías"))
                .andExpect(jsonPath("$.status").value("ACTIVE"))
                .andExpect(jsonPath("$.version").value(0))
                .andExpect(jsonPath("$.createdAt").isNotEmpty());
    }

    @Test
    void createCategoryWithoutPermissionIsForbidden() throws Exception {
        RbacFixture fx = fixture();
        String token = login(fx);
        mockMvc.perform(post("/api/v1/organizations/" + fx.orgId() + "/categories")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"bebidas\",\"name\":\"Bebidas\"}"))
                .andExpect(status().isForbidden())
                .andExpect(jsonPath("$.code").value("FORBIDDEN"));
    }

    @Test
    void createCategoryOutsideReachableOrganizationIsForbidden() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        mockMvc.perform(post("/api/v1/organizations/" + UUID.randomUUID() + "/categories")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"bebidas\",\"name\":\"Bebidas\"}"))
                .andExpect(status().isForbidden())
                .andExpect(jsonPath("$.code").value("FORBIDDEN"));
    }

    @Test
    void createCategoryRejectsDuplicateCodeWithinSameOrganization() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        createCategory(fx.orgId().toString(), token, "bebidas", "Bebidas");
        mockMvc.perform(post("/api/v1/organizations/" + fx.orgId() + "/categories")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"bebidas\",\"name\":\"Otra\"}"))
                .andExpect(status().isConflict())
                .andExpect(jsonPath("$.code").value("CATEGORY_CODE_ALREADY_EXISTS"));
    }

    @Test
    void createCategoryAllowsSameCodeAcrossOrganizations() throws Exception {
        RbacFixture fxA = fixture();
        RbacFixture fxB = fixture();
        String tokenA = adminToken(fxA);
        String tokenB = adminToken(fxB);
        createCategory(fxA.orgId().toString(), tokenA, "bebidas", "Bebidas");
        mockMvc.perform(post("/api/v1/organizations/" + fxB.orgId() + "/categories")
                        .header("Authorization", bearer(tokenB))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"bebidas\",\"name\":\"Bebidas\"}"))
                .andExpect(status().isCreated())
                .andExpect(jsonPath("$.code").value("bebidas"));
    }

    @Test
    void createCategoryRejectsInvalidSlugCode() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        mockMvc.perform(post("/api/v1/organizations/" + fx.orgId() + "/categories")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"Bebidas Frías\",\"name\":\"Bebidas\"}"))
                .andExpect(status().isBadRequest())
                .andExpect(jsonPath("$.code").value("VALIDATION_ERROR"));
    }

    @Test
    void createCategoryTrimsNameWhitespace() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        mockMvc.perform(post("/api/v1/organizations/" + fx.orgId() + "/categories")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"bebidas\",\"name\":\"   Bebidas   Frías   \"}"))
                .andExpect(status().isCreated())
                .andExpect(jsonPath("$.name").value("Bebidas   Frías"));
    }

    @Test
    void listCategoriesIsPaginatedAndOrderedByName() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        createCategory(fx.orgId().toString(), token, "zeta", "Zeta");
        createCategory(fx.orgId().toString(), token, "alfa", "Alfa");
        mockMvc.perform(get("/api/v1/organizations/" + fx.orgId() + "/categories")
                        .header("Authorization", bearer(token)))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.items.length()").value(2))
                .andExpect(jsonPath("$.items[0].name").value("Alfa"))
                .andExpect(jsonPath("$.items[1].name").value("Zeta"))
                .andExpect(jsonPath("$.totalElements").value(2));
    }

    @Test
    void listCategoriesFiltersByStatus() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String inactive = createCategory(fx.orgId().toString(), token, "inactiva", "Inactiva");
        createCategory(fx.orgId().toString(), token, "activa", "Activa");
        mockMvc.perform(patch("/api/v1/organizations/" + fx.orgId() + "/categories/" + inactive)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"status\":\"INACTIVE\"}"))
                .andExpect(status().isOk());
        mockMvc.perform(get("/api/v1/organizations/" + fx.orgId() + "/categories")
                        .header("Authorization", bearer(token))
                        .param("status", "INACTIVE"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.items.length()").value(1))
                .andExpect(jsonPath("$.items[0].code").value("inactiva"));
    }

    @Test
    void getCategoryWithinOrganizationReturns200() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String categoryId = createCategory(fx.orgId().toString(), token, "bebidas", "Bebidas");
        mockMvc.perform(get("/api/v1/organizations/" + fx.orgId() + "/categories/" + categoryId)
                        .header("Authorization", bearer(token)))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.id").value(categoryId))
                .andExpect(jsonPath("$.code").value("bebidas"));
    }

    @Test
    void getCategoryOfAnotherOrganizationReturns404() throws Exception {
        RbacFixture fxA = fixture();
        RbacFixture fxB = fixture();
        String tokenA = adminToken(fxA);
        String tokenB = adminToken(fxB);
        String categoryB = createCategory(fxB.orgId().toString(), tokenB, "bebidas", "Bebidas");

        mockMvc.perform(get("/api/v1/organizations/" + fxA.orgId() + "/categories/" + categoryB)
                        .header("Authorization", bearer(tokenA)))
                .andExpect(status().isNotFound())
                .andExpect(jsonPath("$.code").value("CATEGORY_NOT_FOUND"));
    }

    @Test
    void patchCategoryUpdatesNameAndStatus() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String categoryId = createCategory(fx.orgId().toString(), token, "bebidas", "Bebidas");
        mockMvc.perform(patch("/api/v1/organizations/" + fx.orgId() + "/categories/" + categoryId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"name\":\"Bebidas Frías\",\"status\":\"INACTIVE\"}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.name").value("Bebidas Frías"))
                .andExpect(jsonPath("$.status").value("INACTIVE"))
                .andExpect(jsonPath("$.code").value("bebidas"))
                .andExpect(jsonPath("$.version").value(1));
    }

    @Test
    void patchCategoryEmptyBodyKeepsNameUnchanged() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String categoryId = createCategory(fx.orgId().toString(), token, "bebidas", "Bebidas");
        mockMvc.perform(patch("/api/v1/organizations/" + fx.orgId() + "/categories/" + categoryId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.name").value("Bebidas"))
                .andExpect(jsonPath("$.version").value(0));
    }

    @Test
    void patchCategoryStaleExpectedVersionIs409() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        String categoryId = createCategory(fx.orgId().toString(), token, "bebidas", "Bebidas");
        mockMvc.perform(patch("/api/v1/organizations/" + fx.orgId() + "/categories/" + categoryId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"name\":\"Bebidas 2\"}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.version").value(1));
        mockMvc.perform(patch("/api/v1/organizations/" + fx.orgId() + "/categories/" + categoryId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"name\":\"Stale\"}"))
                .andExpect(status().isConflict())
                .andExpect(jsonPath("$.code").value("CONCURRENT_MODIFICATION"));
    }

    @Test
    void patchCategoryOfAnotherOrganizationReturns404() throws Exception {
        RbacFixture fxA = fixture();
        RbacFixture fxB = fixture();
        String tokenA = adminToken(fxA);
        String tokenB = adminToken(fxB);
        String categoryB = createCategory(fxB.orgId().toString(), tokenB, "bebidas", "Bebidas");
        mockMvc.perform(patch("/api/v1/organizations/" + fxA.orgId() + "/categories/" + categoryB)
                        .header("Authorization", bearer(tokenA))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"name\":\"Hack\"}"))
                .andExpect(status().isNotFound())
                .andExpect(jsonPath("$.code").value("CATEGORY_NOT_FOUND"));
    }
}
