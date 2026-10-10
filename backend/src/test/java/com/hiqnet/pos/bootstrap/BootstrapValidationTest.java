package com.hiqnet.pos.bootstrap;

import static org.assertj.core.api.Assertions.assertThat;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.jsonPath;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import com.hiqnet.pos.organization.domain.Organization;
import org.junit.jupiter.api.Test;
import org.springframework.http.MediaType;

/**
 * Failures must not provision anything: an invalid request (missing/invalid
 * token, weak password, validated organization shape, non-pristine
 * installation) leaves the database untouched and bootstrap PENDING.
 */
class BootstrapValidationTest extends AbstractBootstrapIntegrationTest {

    @Test
    void missingTokenIsRejectedBeforeAnyProvisioning() throws Exception {
        mockMvc.perform(post("/api/v1/bootstrap")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(bootstrapBody("no-token", uniqueEmail("admin@"), PASSWORD)))
                .andExpect(status().isUnauthorized())
                .andExpect(jsonPath("$.code").value("BOOTSTRAP_INVALID_TOKEN"));

        assertThat(organizationCount()).isZero();
        assertThat(installationCompleted()).isFalse();
        assertThat(auditCountForBootstrap()).isZero();
    }

    @Test
    void invalidTokenIsRejectedWithTheSameGenericCode() throws Exception {
        mockMvc.perform(post("/api/v1/bootstrap")
                        .header("X-Bootstrap-Token", "d3Jvbmctc2VjcmV0")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(bootstrapBody("bad-token", uniqueEmail("admin@"), PASSWORD)))
                .andExpect(status().isUnauthorized())
                .andExpect(jsonPath("$.code").value("BOOTSTRAP_INVALID_TOKEN"));

        assertThat(organizationCount()).isZero();
        assertThat(installationCompleted()).isFalse();
    }

    @Test
    void malformedBase64TokenIsRejectedLikeAnyOtherInvalidToken() throws Exception {
        mockMvc.perform(post("/api/v1/bootstrap")
                        .header("X-Bootstrap-Token", "not base64 at all !!!")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(bootstrapBody("bad-base64", uniqueEmail("admin@"), PASSWORD)))
                .andExpect(status().isUnauthorized())
                .andExpect(jsonPath("$.code").value("BOOTSTRAP_INVALID_TOKEN"));
    }

    @Test
    void weakPasswordIsRejectedBeforeProvisioning() throws Exception {
        mockMvc.perform(post("/api/v1/bootstrap")
                        .header("X-Bootstrap-Token", bootstrapSecret())
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(bootstrapBody("weak-pass", uniqueEmail("admin@"), "short-1")))
                .andExpect(status().isBadRequest())
                .andExpect(jsonPath("$.code").value("WEAK_PASSWORD"));

        assertThat(organizationCount()).isZero();
        assertThat(installationCompleted()).isFalse();
    }

    @Test
    void oversizedPasswordIsRejectedBeforeProvisioning() throws Exception {
        mockMvc.perform(post("/api/v1/bootstrap")
                        .header("X-Bootstrap-Token", bootstrapSecret())
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(bootstrapBody("long-pass", uniqueEmail("admin@"), "p".repeat(129))))
                .andExpect(status().isBadRequest())
                .andExpect(jsonPath("$.code").value("WEAK_PASSWORD"));

        assertThat(organizationCount()).isZero();
    }

    @Test
    void invalidOrganizationShapeIsRejectedBeforeProvisioning() throws Exception {
        mockMvc.perform(post("/api/v1/bootstrap")
                        .header("X-Bootstrap-Token", bootstrapSecret())
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("""
                                {"organization":{"name":"X","code":"NOT A SLUG","timezone":"UTC","currency":"MXN","locale":"es-MX"},"administrator":{"email":"%s","password":"%s"}}
                                """.formatted(uniqueEmail("admin@"), PASSWORD)))
                .andExpect(status().isBadRequest())
                .andExpect(jsonPath("$.code").value("VALIDATION_ERROR"));

        assertThat(organizationCount()).isZero();
        assertThat(installationCompleted()).isFalse();
    }

    @Test
    void bootstrapIsRejectedWhenOrganizationsAlreadyExist() throws Exception {
        organizationRepository.saveAndFlush(Organization.create(
                "Existing", "existing-org", "America/Mexico_City", "MXN", "es-MX"));

        mockMvc.perform(post("/api/v1/bootstrap")
                        .header("X-Bootstrap-Token", bootstrapSecret())
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(bootstrapBody("pristine-guard", uniqueEmail("admin@"), PASSWORD)))
                .andExpect(status().isConflict())
                .andExpect(jsonPath("$.code").value("BOOTSTRAP_NOT_ALLOWED"));

        // Nothing new was provisioned.
        assertThat(organizationCount()).isEqualTo(1);
        assertThat(installationCompleted()).isFalse();
    }

    private long organizationCount() {
        return organizationRepository.count();
    }

    private long auditCountForBootstrap() {
        Long count = jdbcTemplate.queryForObject("select count(*) from audit_events", Long.class);
        return count == null ? 0 : count;
    }
}