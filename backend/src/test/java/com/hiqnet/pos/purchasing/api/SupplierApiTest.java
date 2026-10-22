package com.hiqnet.pos.purchasing.api;

import static org.assertj.core.api.Assertions.assertThat;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.get;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.patch;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.jsonPath;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import com.hiqnet.pos.audit.AbstractAuditIntegrationTest;
import com.hiqnet.pos.purchasing.application.SupplierService;
import java.util.UUID;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.http.MediaType;
import org.springframework.test.web.servlet.MvcResult;

class SupplierApiTest extends AbstractAuditIntegrationTest {

    private static final String BASE = "/api/v1/organizations/";

    @Autowired
    private SupplierService supplierService;

    private String createSupplier(UUID organizationId, String token, String code, String name) throws Exception {
        MvcResult result = mockMvc.perform(post(BASE + organizationId + "/suppliers")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"" + code + "\",\"name\":\"" + name + "\"}"))
                .andExpect(status().isCreated())
                .andReturn();
        return objectMapper.readTree(result.getResponse().getContentAsString()).get("id").asText();
    }

    @Test
    void supplierRoutesRequireAuthentication() throws Exception {
        RbacFixture fixture = fixture();
        mockMvc.perform(get(BASE + fixture.orgId() + "/suppliers")).andExpect(status().isUnauthorized());
    }

    @Test
    void createReturnsOrganizationOwnedSupplierAndRecordsSanitizedAudit() throws Exception {
        RbacFixture fixture = fixture();
        String token = adminToken(fixture);

        mockMvc.perform(post(BASE + fixture.orgId() + "/suppliers")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("""
                                {
                                  "code":"distribuidora-centro",
                                  "name":"  Distribuidora Centro  ",
                                  "legalName":"  Distribuidora Centro, S.A. de C.V.  ",
                                  "taxId":"  DCE010203AB4  ",
                                  "contactName":"  Ana López  ",
                                  "email":"  SALES@DISTRIBUIDORA-CENTRO.EXAMPLE  ",
                                  "phone":"  +52 55 5555 0101  ",
                                  "address":"  Av. Central 100  "
                                }
                                """))
                .andExpect(status().isCreated())
                .andExpect(jsonPath("$.id").isNotEmpty())
                .andExpect(jsonPath("$.organizationId").value(fixture.orgId().toString()))
                .andExpect(jsonPath("$.code").value("distribuidora-centro"))
                .andExpect(jsonPath("$.name").value("Distribuidora Centro"))
                .andExpect(jsonPath("$.legalName").value("Distribuidora Centro, S.A. de C.V."))
                .andExpect(jsonPath("$.taxId").value("DCE010203AB4"))
                .andExpect(jsonPath("$.contactName").value("Ana López"))
                .andExpect(jsonPath("$.email").value("sales@distribuidora-centro.example"))
                .andExpect(jsonPath("$.phone").value("+52 55 5555 0101"))
                .andExpect(jsonPath("$.address").value("Av. Central 100"))
                .andExpect(jsonPath("$.status").value("ACTIVE"))
                .andExpect(jsonPath("$.version").value(0));

        assertThat(latestAudit(fixture.orgId()).action()).isEqualTo("supplier.created");
        assertThat(latestAudit(fixture.orgId()).entityType()).isEqualTo("supplier");
        assertThat(metadataOf(latestAudit(fixture.orgId()))).containsOnlyKeys("code");
    }

    @Test
    void createValidatesInputAndEnforcesOrganizationScopedCodeUniqueness() throws Exception {
        RbacFixture fixtureA = fixture();
        RbacFixture fixtureB = fixture();
        String tokenA = adminToken(fixtureA);
        String tokenB = adminToken(fixtureB);
        String url = BASE + fixtureA.orgId() + "/suppliers";

        mockMvc.perform(post(url).header("Authorization", bearer(tokenA)).contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"Supplier_One\",\"name\":\"Supplier\"}"))
                .andExpect(status().isBadRequest())
                .andExpect(jsonPath("$.code").value("VALIDATION_ERROR"));
        mockMvc.perform(post(url).header("Authorization", bearer(tokenA)).contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"supplier-one\",\"name\":\"  \"}"))
                .andExpect(status().isBadRequest())
                .andExpect(jsonPath("$.code").value("VALIDATION_ERROR"));
        mockMvc.perform(post(url).header("Authorization", bearer(tokenA)).contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"supplier-email\",\"name\":\"Supplier\",\"email\":\"not-an-email\"}"))
                .andExpect(status().isBadRequest())
                .andExpect(jsonPath("$.code").value("VALIDATION_ERROR"));

        createSupplier(fixtureA.orgId(), tokenA, "supplier-one", "Supplier One");
        long auditCount = auditCount(fixtureA.orgId());
        mockMvc.perform(post(url).header("Authorization", bearer(tokenA)).contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"supplier-one\",\"name\":\"Duplicate\"}"))
                .andExpect(status().isConflict())
                .andExpect(jsonPath("$.code").value("SUPPLIER_CODE_ALREADY_EXISTS"));
        assertThat(auditCount(fixtureA.orgId())).isEqualTo(auditCount);
        mockMvc.perform(post(BASE + fixtureB.orgId() + "/suppliers")
                        .header("Authorization", bearer(tokenB))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"supplier-one\",\"name\":\"Supplier Two\"}"))
                .andExpect(status().isCreated());
    }

    @Test
    void listFiltersByStatusAndQueryWithStablePagination() throws Exception {
        RbacFixture fixture = fixture();
        String token = adminToken(fixture);
        String inactiveId = createSupplier(fixture.orgId(), token, "central", "Central Supplies");
        String activeId = createSupplier(fixture.orgId(), token, "beans", "Bean Wholesale");
        mockMvc.perform(patch(BASE + fixture.orgId() + "/suppliers/" + inactiveId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"status\":\"INACTIVE\"}"))
                .andExpect(status().isOk());

        mockMvc.perform(get(BASE + fixture.orgId() + "/suppliers")
                        .header("Authorization", bearer(token)))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.items[0].name").value("Bean Wholesale"))
                .andExpect(jsonPath("$.items[1].name").value("Central Supplies"));
        mockMvc.perform(get(BASE + fixture.orgId() + "/suppliers")
                        .header("Authorization", bearer(token))
                        .param("q", "BEAN")
                        .param("status", "ACTIVE")
                        .param("page", "0")
                        .param("size", "1"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.items.length()").value(1))
                .andExpect(jsonPath("$.items[0].id").value(activeId))
                .andExpect(jsonPath("$.page").value(0))
                .andExpect(jsonPath("$.size").value(1))
                .andExpect(jsonPath("$.totalElements").value(1));
        mockMvc.perform(get(BASE + fixture.orgId() + "/suppliers")
                        .header("Authorization", bearer(token))
                        .param("status", "INACTIVE"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.items[0].id").value(inactiveId));
    }

    @Test
    void getAndUpdateNeverResolveSupplierOutsideOrganization() throws Exception {
        RbacFixture fixtureA = fixture();
        RbacFixture fixtureB = fixture();
        String tokenA = adminToken(fixtureA);
        String supplierB = createSupplier(fixtureB.orgId(), adminToken(fixtureB), "supplier", "Supplier");

        mockMvc.perform(get(BASE + fixtureA.orgId() + "/suppliers/" + supplierB)
                        .header("Authorization", bearer(tokenA)))
                .andExpect(status().isNotFound())
                .andExpect(jsonPath("$.code").value("SUPPLIER_NOT_FOUND"));
        mockMvc.perform(patch(BASE + fixtureA.orgId() + "/suppliers/" + supplierB)
                        .header("Authorization", bearer(tokenA))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"name\":\"Tampered\"}"))
                .andExpect(status().isNotFound());
    }

    @Test
    void updateSupportsMasterDataAndStatusWhileAuditingOnlyFieldNames() throws Exception {
        RbacFixture fixture = fixture();
        String token = adminToken(fixture);
        String supplierId = createSupplier(fixture.orgId(), token, "supplier", "Supplier");

        mockMvc.perform(patch(BASE + fixture.orgId() + "/suppliers/" + supplierId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("""
                                {
                                  "expectedVersion":0,
                                  "name":"  Supplier Updated  ",
                                  "legalName":"Legal Name",
                                  "email":"CONTACT@SUPPLIER.EXAMPLE",
                                  "status":"INACTIVE"
                                }
                                """))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.code").value("supplier"))
                .andExpect(jsonPath("$.name").value("Supplier Updated"))
                .andExpect(jsonPath("$.email").value("contact@supplier.example"))
                .andExpect(jsonPath("$.status").value("INACTIVE"))
                .andExpect(jsonPath("$.version").value(1));
        assertThat(metadataOf(latestAudit(fixture.orgId())).get("changedFields").toString())
                .isEqualTo("[name, legalName, email, status]");

        mockMvc.perform(patch(BASE + fixture.orgId() + "/suppliers/" + supplierId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":1,\"legalName\":\"  \",\"email\":\"\"}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.legalName").doesNotExist())
                .andExpect(jsonPath("$.email").doesNotExist())
                .andExpect(jsonPath("$.version").value(2));

        long auditsAfterChange = auditCount(fixture.orgId());
        mockMvc.perform(patch(BASE + fixture.orgId() + "/suppliers/" + supplierId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":2,\"name\":\"Supplier Updated\",\"status\":\"INACTIVE\"}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.version").value(2));
        assertThat(auditCount(fixture.orgId())).isEqualTo(auditsAfterChange);

        mockMvc.perform(patch(BASE + fixture.orgId() + "/suppliers/" + supplierId)
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"name\":\"Stale\"}"))
                .andExpect(status().isConflict())
                .andExpect(jsonPath("$.code").value("CONCURRENT_MODIFICATION"));
    }

    @Test
    void readAndWritePermissionsAreIndependentAndOrganizationScoped() throws Exception {
        RbacFixture reader = fixture();
        String supplierId = supplierService.create(
                        reader.orgId(), "supplier", "Supplier", null, null, null, null, null, null)
                .getId()
                .toString();
        createRoleAndAssign(reader, "supplier-reader", "Supplier Reader", "purchasing.supplier.read");
        String readerToken = login(reader);
        String readerUrl = BASE + reader.orgId() + "/suppliers";
        mockMvc.perform(get(readerUrl).header("Authorization", bearer(readerToken))).andExpect(status().isOk());
        mockMvc.perform(get(readerUrl + "/" + supplierId).header("Authorization", bearer(readerToken)))
                .andExpect(status().isOk());
        mockMvc.perform(post(readerUrl).header("Authorization", bearer(readerToken))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"other\",\"name\":\"Other\"}"))
                .andExpect(status().isForbidden());

        RbacFixture writer = fixture();
        createRoleAndAssign(writer, "supplier-writer", "Supplier Writer", "purchasing.supplier.write");
        String writerToken = login(writer);
        String writerUrl = BASE + writer.orgId() + "/suppliers";
        String writerId = createSupplier(writer.orgId(), writerToken, "supplier", "Supplier");
        mockMvc.perform(patch(writerUrl + "/" + writerId).header("Authorization", bearer(writerToken))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"name\":\"Supplier Updated\"}"))
                .andExpect(status().isOk());
        mockMvc.perform(get(writerUrl).header("Authorization", bearer(writerToken))).andExpect(status().isForbidden());
    }

    @Test
    void readsDoNotWriteAuditEvents() throws Exception {
        RbacFixture fixture = fixture();
        String token = adminToken(fixture);
        String supplierId = createSupplier(fixture.orgId(), token, "supplier", "Supplier");
        long before = auditCount(fixture.orgId());

        mockMvc.perform(get(BASE + fixture.orgId() + "/suppliers").header("Authorization", bearer(token)))
                .andExpect(status().isOk());
        mockMvc.perform(get(BASE + fixture.orgId() + "/suppliers/" + supplierId)
                        .header("Authorization", bearer(token)))
                .andExpect(status().isOk());
        assertThat(auditCount(fixture.orgId())).isEqualTo(before);
    }

    @Test
    void listRejectsInvalidPaginationParameters() throws Exception {
        RbacFixture fixture = fixture();
        String token = adminToken(fixture);
        String url = BASE + fixture.orgId() + "/suppliers";

        mockMvc.perform(get(url).header("Authorization", bearer(token)).param("page", "-1"))
                .andExpect(status().isBadRequest())
                .andExpect(jsonPath("$.code").value("VALIDATION_ERROR"));
        mockMvc.perform(get(url).header("Authorization", bearer(token)).param("size", "0"))
                .andExpect(status().isBadRequest())
                .andExpect(jsonPath("$.code").value("VALIDATION_ERROR"));
        mockMvc.perform(get(url).header("Authorization", bearer(token)).param("size", "101"))
                .andExpect(status().isBadRequest())
                .andExpect(jsonPath("$.code").value("VALIDATION_ERROR"));
    }

    @Test
    void listNeverLeaksSuppliersAcrossOrganizations() throws Exception {
        RbacFixture fixtureA = fixture();
        RbacFixture fixtureB = fixture();
        String tokenA = adminToken(fixtureA);
        String tokenB = adminToken(fixtureB);
        createSupplier(fixtureA.orgId(), tokenA, "alpha", "Alpha Supplies");
        createSupplier(fixtureB.orgId(), tokenB, "beta", "Beta Supplies");

        mockMvc.perform(get(BASE + fixtureA.orgId() + "/suppliers").header("Authorization", bearer(tokenA)))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.items.length()").value(1))
                .andExpect(jsonPath("$.items[0].code").value("alpha"))
                .andExpect(jsonPath("$.totalElements").value(1));
    }
}
