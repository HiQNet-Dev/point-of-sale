package com.hiqnet.pos.audit.domain;

import java.util.UUID;
import org.springframework.data.jpa.repository.JpaSpecificationExecutor;
import org.springframework.data.repository.Repository;

/**
 * Deliberately RESTRICTED persistence surface: extends the marker
 * {@link Repository} (not {@code CrudRepository}) so NO delete/deleteAll or
 * general CRUD is exposed — only {@code save} (append) and the
 * Specification-based read queries. The append-only BEFORE trigger on
 * {@code audit_events} is the final barrier regardless of this interface.
 */
public interface AuditEventRepository
        extends Repository<AuditEvent, UUID>, JpaSpecificationExecutor<AuditEvent> {

    AuditEvent save(AuditEvent event);
}