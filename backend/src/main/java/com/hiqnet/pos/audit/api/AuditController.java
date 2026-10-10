package com.hiqnet.pos.audit.api;

import com.hiqnet.pos.audit.application.AuditEventFilter;
import com.hiqnet.pos.audit.application.AuditQueryService;
import com.hiqnet.pos.audit.domain.AuditEvent;
import com.hiqnet.pos.shared.api.PageResponse;
import com.hiqnet.pos.shared.error.BusinessException;
import com.hiqnet.pos.shared.error.ErrorCodes;
import io.swagger.v3.oas.annotations.Operation;
import io.swagger.v3.oas.annotations.media.Content;
import io.swagger.v3.oas.annotations.responses.ApiResponse;
import io.swagger.v3.oas.annotations.responses.ApiResponses;
import java.time.Instant;
import java.util.UUID;
import org.springframework.data.domain.Page;
import org.springframework.data.domain.PageRequest;
import org.springframework.data.domain.Sort;
import org.springframework.http.HttpStatus;
import org.springframework.security.access.prepost.PreAuthorize;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.PathVariable;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RequestParam;
import org.springframework.web.bind.annotation.RestController;

/**
 * READ-ONLY audit log endpoint. There is intentionally NO mutation API:
 * {@code POST/PATCH/PUT/DELETE /audit-events} are closed by
 * {@code SecurityConfig.anyRequest().denyAll()} and the append-only DB trigger
 * rejects any direct mutation. The {@code audit.read} permission is evaluated
 * ORGANIZATION-wide (a branch-scoped role containing {@code audit.read} never
 * grants access — the audit log is organization-sensitive).
 */
@RestController
@RequestMapping("/api/v1/organizations/{organizationId}/audit-events")
public class AuditController {

    private static final int DEFAULT_PAGE_SIZE = 50;
    private static final int MAX_PAGE_SIZE = 100;

    /** Deterministic order: newest first, id DESC as tiebreaker. */
    private static final Sort ORDER = Sort.by(Sort.Order.desc("occurredAt"), Sort.Order.desc("id"));

    private final AuditQueryService auditQueryService;

    public AuditController(AuditQueryService auditQueryService) {
        this.auditQueryService = auditQueryService;
    }

    @GetMapping
    @Operation(summary = "List audit events of an organization (paginated, newest first)")
    @ApiResponses({
        @ApiResponse(responseCode = "200", description = "OK", content = @Content),
        @ApiResponse(responseCode = "400", description = "Invalid paging or from > to (VALIDATION_ERROR)", content = @Content),
        @ApiResponse(responseCode = "404", description = "Organization, branch or actor user not found (ORGANIZATION_NOT_FOUND, BRANCH_NOT_FOUND, USER_NOT_FOUND)", content = @Content),
        @ApiResponse(responseCode = "403", description = "Forbidden (audit.read)", content = @Content)
    })
    @PreAuthorize("@authorizationService.hasPermission(authentication, #organizationId, 'audit.read')")
    public PageResponse<AuditResponse> list(
            @PathVariable UUID organizationId,
            @RequestParam(defaultValue = "0") int page,
            @RequestParam(defaultValue = "50") int size,
            @RequestParam(required = false) UUID branchId,
            @RequestParam(required = false) UUID actorUserId,
            @RequestParam(required = false) String action,
            @RequestParam(required = false) String entityType,
            @RequestParam(required = false) UUID entityId,
            @RequestParam(required = false) Instant from,
            @RequestParam(required = false) Instant to) {
        validatePaging(page, size);
        AuditEventFilter filter = new AuditEventFilter(branchId, actorUserId, action, entityType, entityId, from, to);
        Page<AuditEvent> events = auditQueryService.list(organizationId, filter, PageRequest.of(page, size, ORDER));
        return PageResponse.from(events, AuditResponse::from);
    }

    private static void validatePaging(int page, int size) {
        if (page < 0) {
            throw new BusinessException(HttpStatus.BAD_REQUEST, ErrorCodes.VALIDATION_ERROR, "page must be >= 0");
        }
        if (size < 1 || size > MAX_PAGE_SIZE) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST,
                    ErrorCodes.VALIDATION_ERROR,
                    "size must be between 1 and " + MAX_PAGE_SIZE);
        }
    }
}