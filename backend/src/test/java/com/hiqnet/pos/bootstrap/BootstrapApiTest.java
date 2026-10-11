package com.hiqnet.pos.bootstrap;

import static org.assertj.core.api.Assertions.assertThat;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.get;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.jsonPath;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import java.nio.charset.StandardCharsets;
import java.util.List;
import java.util.UUID;
import org.junit.jupiter.api.Test;
import org.springframework.http.MediaType;
import org.springframework.test.web.servlet.MvcResult;

/**
 * The secure bootstrap happy path and RBAC-semantics guarantees: one-shot
 * provisioning, audit events with SYSTEM actor, immediate login, full
 * organization-wide authority through NORMAL RBAC (no magic 'administrator'
 * bypass), POST /organizations still denied.
 */
class BootstrapApiTest extends AbstractBootstrapIntegrationTest {

    @Test
    void bootstrapCreatesInstallationAndAllowsLogin() throws Exception {
        String orgCode = "cafe-aurora";
        String email = uniqueEmail("admin@");
        MvcResult result = mockMvc.perform(post("/api/v1/bootstrap")
                        .header("X-Bootstrap-Token", bootstrapSecret())
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(bootstrapBody(orgCode, email, PASSWORD)))
                .andExpect(status().isCreated())
                .andExpect(jsonPath("$.organizationCode").value(orgCode))
                .andReturn();

        var response = objectMapper.readTree(result.getResponse().getContentAsString(StandardCharsets.UTF_8));
        UUID organizationId = UUID.fromString(response.get("organizationId").asText());
        UUID administratorId = UUID.fromString(response.get("administratorUserId").asText());
        UUID administratorRoleId = UUID.fromString(response.get("administratorRoleId").asText());
        // Provisioning is NOT authentication: no token material in the response.
        assertThat(response.has("accessToken")).isFalse();
        assertThat(response.has("refreshToken")).isFalse();
        assertThat(installationCompleted()).isTrue();

        // Login after bootstrap works immediately.
        String access = postLoginExpectingToken(orgCode, email, PASSWORD);
        mockMvc.perform(get("/api/v1/organizations/" + organizationId)
                        .header("Authorization", bearer(access)))
                .andExpect(status().isOk());
        mockMvc.perform(get("/api/v1/organizations/" + organizationId + "/audit-events")
                        .header("Authorization", bearer(access)))
                .andExpect(status().isOk());

        // POST /organizations remains closed even for the administrator.
        mockMvc.perform(post("/api/v1/organizations")
                        .header("Authorization", bearer(access))
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"name\":\"X\",\"code\":\"x-2\",\"timezone\":\"UTC\",\"currency\":\"MXN\",\"locale\":\"es-MX\"}"))
                .andExpect(status().isForbidden());

        // Exactly the five provisioning events, all SYSTEM (no user existed).
        List<AuditRow> rows = auditRows(organizationId);
        assertThat(rows).hasSize(5);
        assertThat(rows.stream().map(AuditRow::action))
                .containsExactlyInAnyOrder(
                        "organization.created",
                        "user.created",
                        "role.created",
                        "role.permissions.replaced",
                        "user.roles.replaced");
        assertThat(rows).allMatch(row -> "SYSTEM".equals(row.actorType()));
        assertThat(rows).allMatch(row -> row.actorUserId() == null);

        // The administrator role is granted the ENTIRE current catalog (read
        // from PostgreSQL, not a second hardcoded list).
        Long granted = jdbcTemplate.queryForObject(
                "select count(*) from role_permissions where role_id = ?", Long.class, administratorRoleId);
        Long catalog = jdbcTemplate.queryForObject("select count(*) from permissions", Long.class);
        assertThat(catalog).isEqualTo(44);
        assertThat(granted).isEqualTo(catalog);
        assertThat(administratorId).isNotNull();
    }

    @Test
    void secondBootstrapIsRejectedWithoutChangingAnything() throws Exception {
        String code = "first-org";
        mockMvc.perform(post("/api/v1/bootstrap")
                        .header("X-Bootstrap-Token", bootstrapSecret())
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(bootstrapBody(code, uniqueEmail("admin@"), PASSWORD)))
                .andExpect(status().isCreated());

        // Same VALID secret, completely different body: no changes, conflict.
        mockMvc.perform(post("/api/v1/bootstrap")
                        .header("X-Bootstrap-Token", bootstrapSecret())
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(bootstrapBody("other-org", uniqueEmail("other@"), "another-strong-password")))
                .andExpect(status().isConflict())
                .andExpect(jsonPath("$.code").value("BOOTSTRAP_ALREADY_COMPLETED"));

        Long organizations = jdbcTemplate.queryForObject("select count(*) from organizations", Long.class);
        assertThat(organizations).isEqualTo(1);
    }

    @Test
    void administratorIsARegularRbacRoleWithoutMagicBypass() throws Exception {
        String orgCode = "rbac-org";
        String email = uniqueEmail("admin@");
        mockMvc.perform(post("/api/v1/bootstrap")
                        .header("X-Bootstrap-Token", bootstrapSecret())
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(bootstrapBody(orgCode, email, PASSWORD)))
                .andExpect(status().isCreated());
        UUID organizationId = jdbcTemplate.queryForObject(
                "select id from organizations where code = ?", UUID.class, orgCode);
        UUID roleId = jdbcTemplate.queryForObject(
                "select administrator_role_id from installation_bootstrap where id = 1", UUID.class);

        String access = postLoginExpectingToken(orgCode, email, PASSWORD);
        mockMvc.perform(get("/api/v1/organizations/" + organizationId)
                        .header("Authorization", bearer(access)))
                .andExpect(status().isOk());

        // Removing a permission from the administrator role removes access
        // immediately with the SAME JWT: no bypass by role code.
        jdbcTemplate.update(
                "delete from role_permissions where role_id = ? and permission_code = 'organization.read'", roleId);
        mockMvc.perform(get("/api/v1/organizations/" + organizationId)
                        .header("Authorization", bearer(access)))
                .andExpect(status().isForbidden());

        // Deactivating the role also revokes authority, like any normal role.
        jdbcTemplate.update("update roles set status = 'INACTIVE' where id = ?", roleId);
        mockMvc.perform(get("/api/v1/organizations/" + organizationId + "/audit-events")
                        .header("Authorization", bearer(access)))
                .andExpect(status().isForbidden());
    }
}