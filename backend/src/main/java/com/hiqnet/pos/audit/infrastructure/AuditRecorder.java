package com.hiqnet.pos.audit.infrastructure;

import com.hiqnet.pos.audit.domain.AuditActorType;
import com.hiqnet.pos.audit.domain.AuditEvent;
import com.hiqnet.pos.audit.domain.AuditEventRepository;
import com.hiqnet.pos.shared.audit.AuditException;
import com.hiqnet.pos.shared.audit.AuditMetadata;
import com.hiqnet.pos.shared.audit.AuditPort;
import com.hiqnet.pos.shared.audit.AuditRecord;
import java.nio.charset.StandardCharsets;
import java.time.Clock;
import java.time.Instant;
import java.util.Map;
import java.util.UUID;
import org.springframework.security.core.Authentication;
import org.springframework.security.core.context.SecurityContextHolder;
import org.springframework.security.oauth2.jwt.Jwt;
import org.springframework.stereotype.Component;
import org.springframework.transaction.annotation.Propagation;
import org.springframework.transaction.annotation.Transactional;
import tools.jackson.databind.ObjectMapper;

/**
 * Runtime implementation of {@link AuditPort} (module {@code audit ->
 * shared.audit}; business modules never see this class).
 *
 * <ul>
 *   <li><b>Actor resolution is central:</b> the acting {@code USER} (JWT
 *       {@code sub} → UUID) and the access-token {@code jti} come from the
 *       {@link SecurityContextHolder}; internal invocations without an
 *       authenticated user become {@code SYSTEM}. Business services never
 *       import Spring Security for auditing.</li>
 *   <li><b>Strong atomicity:</b> {@code record} REQUIRES an existing
 *       transaction ({@code Propagation.MANDATORY}) — the audit insert commits
 *       or rolls back together with the business mutation. Never asynchrony,
 *       never {@code REQUIRES_NEW}.</li>
 *   <li><b>Metadata safety and size:</b> sensitive keys are already rejected by
 *       {@link AuditMetadata} at construction; the serialized JSON length is
 *       bounded here (16 KiB) before the insert. Any failure throws
 *       {@link AuditException} → the business transaction rolls back.</li>
 * </ul>
 */
@Component
public class AuditRecorder implements AuditPort {

    private final AuditEventRepository auditEventRepository;
    private final ObjectMapper objectMapper;
    private final Clock clock;

    public AuditRecorder(
            AuditEventRepository auditEventRepository, ObjectMapper objectMapper, Clock clock) {
        this.auditEventRepository = auditEventRepository;
        this.objectMapper = objectMapper;
        this.clock = clock;
    }

    @Override
    @Transactional(propagation = Propagation.MANDATORY)
    public void record(AuditRecord record) {
        ActorContext actor = resolveActor();
        Map<String, Object> metadata = serializeSafe(record.metadata());
        AuditEvent event = AuditEvent.create(
                record.organizationId(),
                record.branchId(),
                actor.type(),
                actor.userId(),
                actor.jti(),
                record.action(),
                record.entityType(),
                record.entityId(),
                record.reason(),
                metadata,
                clock.instant());
        auditEventRepository.save(event);
    }

    private Map<String, Object> serializeSafe(AuditMetadata metadata) {
        if (metadata == null) {
            return null;
        }
        Map<String, Object> values = metadata.asMap();
        if (values.isEmpty()) {
            return null;
        }
        try {
            String json = objectMapper.writeValueAsString(values);
            if (json.getBytes(StandardCharsets.UTF_8).length > AuditMetadata.MAX_SERIALIZED_BYTES) {
                throw new AuditException(
                        "metadata exceeds " + AuditMetadata.MAX_SERIALIZED_BYTES + " serialized bytes");
            }
            return values;
        } catch (AuditException e) {
            throw e;
        } catch (Exception e) {
            throw new AuditException("metadata could not be serialized: " + e.getMessage());
        }
    }

    private ActorContext resolveActor() {
        Authentication authentication = SecurityContextHolder.getContext().getAuthentication();
        if (authentication != null && authentication.getPrincipal() instanceof Jwt jwt) {
            return new ActorContext(AuditActorType.USER, parseSubject(jwt.getSubject()), jwt.getId());
        }
        // Internal invocation (bootstrap/maintenance/test without security):
        // SYSTEM events never carry a user id (DB CHECK enforces the pair).
        return new ActorContext(AuditActorType.SYSTEM, null, null);
    }

    private static UUID parseSubject(String subject) {
        try {
            return UUID.fromString(subject);
        } catch (IllegalArgumentException e) {
            // Unreachable with the configured resource server (JWT validation
            // requires sub to be a UUID); treat it as an audit invariant break.
            throw new AuditException("authenticated JWT has a non-UUID subject: " + subject);
        }
    }

    private record ActorContext(AuditActorType type, UUID userId, String jti) {}
}