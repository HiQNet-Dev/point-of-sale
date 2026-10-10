package com.hiqnet.pos.bootstrap;

import com.hiqnet.pos.audit.AbstractAuditIntegrationTest;
import com.hiqnet.pos.shared.config.BootstrapProperties;
import org.junit.jupiter.api.BeforeEach;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.jdbc.core.JdbcTemplate;

/**
 * Shared base for secure-bootstrap integration suites. Runs the REAL security
 * chain (bootstrap enabled via the test profile secret) with a real PostgreSQL.
 *
 * <p>Bootstrap state is GLOBAL (singleton row) and the provisioning guard
 * requires a pristine installation (zero organizations), so every test method
 * resets the database. {@code TRUNCATE ... CASCADE} is a schema-level reset:
 * it does NOT fire the append-only row trigger of {@code audit_events}
 * (UPDATE/DELETE remain rejected by PostgreSQL, verified separately), and the
 * permission catalog — which has no FK to organizations — survives. The suite
 * runs sequentially, so the reset cannot race other test classes.</p>
 */
public abstract class AbstractBootstrapIntegrationTest extends AbstractAuditIntegrationTest {

    @Autowired
    protected JdbcTemplate jdbcTemplate;

    @Autowired
    protected BootstrapProperties bootstrapProperties;

    @BeforeEach
    void resetInstallationState() {
        jdbcTemplate.execute("TRUNCATE TABLE organizations CASCADE");
        jdbcTemplate.execute("INSERT INTO installation_bootstrap (id) VALUES (1)");
    }

    /** The test-profile bootstrap secret (Base64) used as X-Bootstrap-Token. */
    protected String bootstrapSecret() {
        return bootstrapProperties.secret();
    }

    protected String bootstrapBody(String organizationCode, String email, String password) {
        return """
                {"organization":{"name":"Cafe Aurora","code":"%s","timezone":"America/Mexico_City","currency":"MXN","locale":"es-MX"},"administrator":{"email":"%s","password":"%s"}}
                """.formatted(organizationCode, email, password);
    }

    protected boolean installationCompleted() {
        Boolean completed = jdbcTemplate.queryForObject(
                "select completed_at is not null from installation_bootstrap where id = 1", Boolean.class);
        return Boolean.TRUE.equals(completed);
    }
}