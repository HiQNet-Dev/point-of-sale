package com.hiqnet.pos.organization.api;

import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.get;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.patch;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.jsonPath;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import com.hiqnet.pos.AbstractRbacIntegrationTest;
import java.util.UUID;
import org.junit.jupiter.api.Test;
import org.springframework.http.MediaType;

/**
 * Organization GET/PATCH under the REAL security chain + RBAC: requires a valid
 * JWT and the organization.read/organization.update permission; PATCH is
 * partial with a required expectedVersion (stale writes -> 409).
 */
class OrganizationAdminApiTest extends AbstractRbacIntegrationTest {

    @Test
    void getOrganizationWithPermissionReturns200() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);

        mockMvc.perform(get("/api/v1/organizations/" + fx.orgId()).header("Authorization", bearer(token)))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.id").value(fx.orgId().toString()))
                .andExpect(jsonPath("$.code").value(fx.orgCode()));
    }

    @Test
    void getOrganizationWithoutPermissionIsForbidden() throws Exception {
        RbacFixture fx = fixture();
        String token = login(fx);

        mockMvc.perform(get("/api/v1/organizations/" + fx.orgId()).header("Authorization", bearer(token)))
                .andExpect(status().isForbidden())
                .andExpect(jsonPath("$.code").value("FORBIDDEN"));
    }

    @Test
    void getUnknownOrganizationIsForbiddenAtTheAuthorizationBoundary() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);

        // The actor has permissions only in its own organization: an unknown
        // organization id is an authorization-scope violation (403) before any
        // resource-existence lookup could ever answer 404.
        mockMvc.perform(get("/api/v1/organizations/" + UUID.randomUUID()).header("Authorization", bearer(token)))
                .andExpect(status().isForbidden())
                .andExpect(jsonPath("$.code").value("FORBIDDEN"));
    }

    @Test
    void patchOrganizationUpdatesPermittedFieldsAndBumpsVersion() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);

        mockMvc.perform(patch("/api/v1/organizations/" + fx.orgId())
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"name\":\"Cafe Renovado\","
                                + "\"timezone\":\"America/Argentina/Buenos_Aires\",\"currency\":\"USD\",\"locale\":\"en-US\"}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.name").value("Cafe Renovado"))
                .andExpect(jsonPath("$.timezone").value("America/Argentina/Buenos_Aires"))
                .andExpect(jsonPath("$.currency").value("USD"))
                .andExpect(jsonPath("$.locale").value("en-US"))
                .andExpect(jsonPath("$.version").value(1));

        // code is immutable, even in PATCH.
        mockMvc.perform(get("/api/v1/organizations/" + fx.orgId()).header("Authorization", bearer(token)))
                .andExpect(jsonPath("$.code").value(fx.orgCode()));
    }

    @Test
    void patchOrganizationEmptyBodyIsNoOp() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);

        mockMvc.perform(patch("/api/v1/organizations/" + fx.orgId())
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.version").value(0));
    }

    @Test
    void patchOrganizationDeactivatesAndReactivationIsOutsideTheApiOnceInactive() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);

        mockMvc.perform(patch("/api/v1/organizations/" + fx.orgId())
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"status\":\"INACTIVE\"}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.status").value("INACTIVE"));

        // Once the organization is INACTIVE it is no longer operational: the
        // authorization query rejects it, so even reactivation through this API
        // is denied (403). Reactivation belongs to bootstrap / the future
        // Control Plane, not to a permission.
        mockMvc.perform(patch("/api/v1/organizations/" + fx.orgId())
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":1,\"status\":\"ACTIVE\"}"))
                .andExpect(status().isForbidden())
                .andExpect(jsonPath("$.code").value("FORBIDDEN"));
    }

    @Test
    void patchOrganizationTrimsNameWhitespace() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);

        mockMvc.perform(patch("/api/v1/organizations/" + fx.orgId())
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"name\":\"  Cafe Renovado  \"}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.name").value("Cafe Renovado"));
    }

    @Test
    void patchOrganizationRejectsInvalidTimezone() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);

        mockMvc.perform(patch("/api/v1/organizations/" + fx.orgId())
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"timezone\":\"Not/AZone\"}"))
                .andExpect(status().isBadRequest())
                .andExpect(jsonPath("$.code").value("INVALID_TIMEZONE"));
    }

    @Test
    void patchOrganizationWithoutPermissionIsForbidden() throws Exception {
        RbacFixture fx = fixture();
        String token = login(fx);

        mockMvc.perform(patch("/api/v1/organizations/" + fx.orgId())
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"name\":\"Hack\"}"))
                .andExpect(status().isForbidden())
                .andExpect(jsonPath("$.code").value("FORBIDDEN"));
    }

    @Test
    void patchOrganizationRequiresExpectedVersion() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);

        mockMvc.perform(patch("/api/v1/organizations/" + fx.orgId())
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"name\":\"No Version\"}"))
                .andExpect(status().isBadRequest())
                .andExpect(jsonPath("$.code").value("VALIDATION_ERROR"));
    }

    @Test
    void patchOrganizationStaleExpectedVersionIs409() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);

        mockMvc.perform(patch("/api/v1/organizations/" + fx.orgId())
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"name\":\"Version 1\"}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.version").value(1));

        mockMvc.perform(patch("/api/v1/organizations/" + fx.orgId())
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"expectedVersion\":0,\"name\":\"Stale\"}"))
                .andExpect(status().isConflict())
                .andExpect(jsonPath("$.code").value("CONCURRENT_MODIFICATION"));
    }
}