package com.hiqnet.pos;

import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.delete;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.get;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.patch;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.put;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.jsonPath;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import java.util.UUID;
import org.junit.jupiter.api.Test;
import org.springframework.http.MediaType;

/**
 * Unsupported methods must remain closed by deny-by-default. Every route below
 * has NO matcher in {@code SecurityConfig} (the matchers are method-specific),
 * so even a user holding EVERY current permission gets 403 — and an anonymous
 * caller gets 401. This is the safety net that keeps new/forgotten methods from
 * opening accidentally.
 */
class UnsupportedMethodIntegrationTest extends AbstractRbacIntegrationTest {

    @Test
    void destructiveAndUnmappedMethodsAreDeniedEvenWithAllPermissions() throws Exception {
        RbacFixture fx = fixture();
        String token = adminToken(fx);
        UUID orgId = fx.orgId();
        UUID userId = fx.userId();
        UUID randomId = UUID.randomUUID();

        // No physical delete exists for admin resources.
        mockMvc.perform(delete("/api/v1/organizations/{org}/users/{user}", orgId, userId)
                        .header("Authorization", bearer(token)))
                .andExpect(status().isForbidden())
                .andExpect(jsonPath("$.code").value("FORBIDDEN"));
        mockMvc.perform(delete("/api/v1/organizations/{org}/roles/{role}", orgId, randomId)
                        .header("Authorization", bearer(token)))
                .andExpect(status().isForbidden());
        mockMvc.perform(delete("/api/v1/organizations/{org}/branches/{branch}", orgId, randomId)
                        .header("Authorization", bearer(token)))
                .andExpect(status().isForbidden());
        mockMvc.perform(delete("/api/v1/organizations/{org}/branches/{branch}/memberships/{user}",
                        orgId, randomId, userId)
                        .header("Authorization", bearer(token)))
                .andExpect(status().isForbidden());

        // Audit is read-only: no mutation API exists.
        for (var request : new org.springframework.test.web.servlet.request.MockHttpServletRequestBuilder[] {
            post("/api/v1/organizations/{org}/audit-events", orgId),
            put("/api/v1/organizations/{org}/audit-events", orgId),
            patch("/api/v1/organizations/{org}/audit-events", orgId),
            delete("/api/v1/organizations/{org}/audit-events", orgId)
        }) {
            mockMvc.perform(request.header("Authorization", bearer(token)))
                    .andExpect(status().isForbidden());
        }

        // Bootstrap has no status/read/reset surface.
        mockMvc.perform(get("/api/v1/bootstrap").header("Authorization", bearer(token)))
                .andExpect(status().isForbidden());
        mockMvc.perform(delete("/api/v1/bootstrap").header("Authorization", bearer(token)))
                .andExpect(status().isForbidden());

        // Unsupported administrative create (organization collection) stays closed.
        mockMvc.perform(post("/api/v1/organizations")
                        .header("Authorization", bearer(token))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"name\":\"X\",\"code\":\"x\",\"timezone\":\"UTC\",\"currency\":\"MXN\",\"locale\":\"es-MX\"}"))
                .andExpect(status().isForbidden());
    }

    @Test
    void anonymousCallersGetUnauthorizedOnUnmappedMethods() throws Exception {
        UUID orgId = UUID.randomUUID();
        mockMvc.perform(delete("/api/v1/organizations/{org}/users/{user}", orgId, UUID.randomUUID()))
                .andExpect(status().isUnauthorized())
                .andExpect(jsonPath("$.code").value("AUTHENTICATION_REQUIRED"));
    }
}