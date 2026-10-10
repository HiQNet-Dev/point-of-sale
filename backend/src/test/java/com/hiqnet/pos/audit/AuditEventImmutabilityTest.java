package com.hiqnet.pos.audit;

import static org.assertj.core.api.Assertions.assertThatThrownBy;

import com.hiqnet.pos.audit.domain.AuditActorType;
import com.hiqnet.pos.audit.domain.AuditEvent;
import com.hiqnet.pos.audit.domain.AuditEventRepository;
import java.time.Instant;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.UUID;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;

/**
 * {@link AuditEvent} exposes NO mutable state: {@code getMetadata()} returns a
 * deep immutable view both for an entity built in memory and for one hydrated
 * by Hibernate from the JSONB column (whose nested Maps/Lists are mutable
 * until wrapped). The JPA-level guarantee is reinforced by {@code @Immutable};
 * PostgreSQL's append-only trigger remains the final authority.
 */
class AuditEventImmutabilityTest extends AbstractAuditIntegrationTest {

    @Autowired
    private AuditEventRepository auditEventRepository;

    @Test
    void createdEventMetadataIsDeeplyImmutable() {
        Map<String, Object> metadata = new LinkedHashMap<>();
        metadata.put("changedFields", new ArrayList<>(List.of("name")));
        metadata.put("before", Map.of("status", "ACTIVE"));

        AuditEvent event = AuditEvent.create(
                UUID.randomUUID(),
                null,
                AuditActorType.USER,
                UUID.randomUUID(),
                "jti-1",
                "user.updated",
                "user",
                UUID.randomUUID(),
                null,
                metadata,
                Instant.now());

        Map<String, Object> view = event.getMetadata();
        assertThatThrownBy(() -> view.put("x", "y")).isInstanceOf(UnsupportedOperationException.class);
        assertThatThrownBy(() -> ((java.util.List) view.get("changedFields")).add("x"))
                .isInstanceOf(UnsupportedOperationException.class);
        assertThatThrownBy(() -> ((java.util.Map) view.get("before")).put("k", "v"))
                .isInstanceOf(UnsupportedOperationException.class);
    }

    @Test
    void hydratedEventMetadataIsDeeplyImmutable() {
        RbacFixture fx = fixture();
        Map<String, Object> metadata = new LinkedHashMap<>();
        metadata.put("changedFields", new ArrayList<>(List.of("name")));

        auditEventRepository.save(AuditEvent.create(
                fx.orgId(),
                null,
                AuditActorType.USER,
                fx.userId(),
                "jti-1",
                "user.updated",
                "user",
                fx.userId(),
                null,
                metadata,
                Instant.now()));

        List<AuditEvent> events = auditEventRepository.findAll(
                (root, query, cb) -> cb.equal(root.get("organizationId"), fx.orgId()));
        AuditEvent hydrated = events.get(0);

        Map<String, Object> view = hydrated.getMetadata();
        assertThatThrownBy(() -> view.put("x", "y")).isInstanceOf(UnsupportedOperationException.class);
        assertThatThrownBy(() -> ((java.util.List) view.get("changedFields")).add("x"))
                .isInstanceOf(UnsupportedOperationException.class);
    }
}