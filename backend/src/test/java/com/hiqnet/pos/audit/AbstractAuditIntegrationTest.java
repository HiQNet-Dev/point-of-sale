package com.hiqnet.pos.audit;

import com.hiqnet.pos.AbstractRbacIntegrationTest;
import com.hiqnet.pos.shared.audit.AuditPort;
import java.sql.ResultSet;
import java.sql.SQLException;
import java.time.Instant;
import java.util.List;
import java.util.Map;
import java.util.UUID;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.jdbc.core.JdbcTemplate;
import org.springframework.security.oauth2.jwt.JwtDecoder;
import org.springframework.transaction.PlatformTransactionManager;
import tools.jackson.core.type.TypeReference;

/**
 * Base for audit suites: reads the append-only history directly through
 * JDBC (not the read API) so assertions are independent of the HTTP layer.
 */
public abstract class AbstractAuditIntegrationTest extends AbstractRbacIntegrationTest {

    @Autowired
    protected JdbcTemplate jdbcTemplate;

    @Autowired
    protected JwtDecoder jwtDecoder;

    @Autowired
    protected AuditPort auditPort;

    @Autowired
    protected PlatformTransactionManager transactionManager;

    protected long auditCount(UUID organizationId) {
        Long count = jdbcTemplate.queryForObject(
                "select count(*) from audit_events where organization_id = ?", Long.class, organizationId);
        return count == null ? 0 : count;
    }

    /** Newest event of the organization (deterministic ordering). */
    protected AuditRow latestAudit(UUID organizationId) {
        List<AuditRow> rows = jdbcTemplate.query(
                """
                select id, organization_id, branch_id, actor_type, actor_user_id, token_jti,
                       action, entity_type, entity_id, reason, metadata, occurred_at
                from audit_events
                where organization_id = ?
                order by occurred_at desc, id desc
                limit 1
                """,
                AbstractAuditIntegrationTest::mapRow,
                organizationId);
        return rows.get(0);
    }

    protected List<AuditRow> auditRows(UUID organizationId) {
        return jdbcTemplate.query(
                """
                select id, organization_id, branch_id, actor_type, actor_user_id, token_jti,
                       action, entity_type, entity_id, reason, metadata, occurred_at
                from audit_events
                where organization_id = ?
                order by occurred_at desc, id desc
                """,
                AbstractAuditIntegrationTest::mapRow,
                organizationId);
    }

    protected String jtiOf(String accessToken) {
        return jwtDecoder.decode(accessToken).getId();
    }

    /** Parses the JSONB metadata of a row for semantic assertions. */
    protected Map<String, Object> metadataOf(AuditRow row) {
        if (row.metadata() == null) {
            return Map.of();
        }
        try {
            return objectMapper.readValue(row.metadata(), new TypeReference<Map<String, Object>>() {});
        } catch (Exception e) {
            throw new IllegalStateException("audit metadata is not valid JSON", e);
        }
    }

    private static AuditRow mapRow(ResultSet rs, int rowNum) throws SQLException {
        java.sql.Timestamp occurredAt = rs.getTimestamp("occurred_at");
        Object metadata = rs.getObject("metadata");
        return new AuditRow(
                rs.getObject("id", UUID.class),
                rs.getObject("organization_id", UUID.class),
                rs.getObject("branch_id", UUID.class),
                rs.getString("actor_type"),
                rs.getObject("actor_user_id", UUID.class),
                rs.getString("token_jti"),
                rs.getString("action"),
                rs.getString("entity_type"),
                rs.getObject("entity_id", UUID.class),
                rs.getString("reason"),
                metadata == null ? null : metadata.toString(),
                occurredAt == null ? null : occurredAt.toInstant());
    }

    /** Plain view of one audit row (never a JPA entity; the history is append-only). */
    protected record AuditRow(
            UUID id,
            UUID organizationId,
            UUID branchId,
            String actorType,
            UUID actorUserId,
            String tokenJti,
            String action,
            String entityType,
            UUID entityId,
            String reason,
            String metadata,
            Instant occurredAt) {}
}