package com.hiqnet.pos.audit.application;

import com.hiqnet.pos.audit.domain.AuditEvent;
import com.hiqnet.pos.audit.domain.AuditEventRepository;
import com.hiqnet.pos.branch.domain.BranchRepository;
import com.hiqnet.pos.organization.domain.OrganizationRepository;
import com.hiqnet.pos.shared.error.BusinessException;
import com.hiqnet.pos.shared.error.ErrorCodes;
import com.hiqnet.pos.users.domain.UserRepository;
import jakarta.persistence.criteria.Predicate;
import java.util.ArrayList;
import java.util.List;
import java.util.UUID;
import org.springframework.data.domain.Page;
import org.springframework.data.domain.Pageable;
import org.springframework.data.jpa.domain.Specification;
import org.springframework.http.HttpStatus;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

/**
 * Read side of the audit log (the only public capability: append-only, no
 * mutation API exists). Scoping and isolation never rely on the caller: the
 * query is ALWAYS anchored on {@code organizationId} and every optional
 * {@code branchId}/{@code actorUserId} filter is validated against that
 * organization (404 when foreign), so a cross-organization id can never leak a
 * filtered zero result.
 */
@Service
public class AuditQueryService {

    private final AuditEventRepository auditEventRepository;
    private final OrganizationRepository organizationRepository;
    private final BranchRepository branchRepository;
    private final UserRepository userRepository;

    public AuditQueryService(
            AuditEventRepository auditEventRepository,
            OrganizationRepository organizationRepository,
            BranchRepository branchRepository,
            UserRepository userRepository) {
        this.auditEventRepository = auditEventRepository;
        this.organizationRepository = organizationRepository;
        this.branchRepository = branchRepository;
        this.userRepository = userRepository;
    }

    @Transactional(readOnly = true)
    public Page<AuditEvent> list(UUID organizationId, AuditEventFilter filter, Pageable pageable) {
        if (!organizationRepository.existsById(organizationId)) {
            throw new BusinessException(
                    HttpStatus.NOT_FOUND, ErrorCodes.ORGANIZATION_NOT_FOUND, "Organization not found");
        }
        if (filter.branchId() != null
                && branchRepository.findByIdAndOrganizationId(filter.branchId(), organizationId).isEmpty()) {
            throw new BusinessException(
                    HttpStatus.NOT_FOUND, ErrorCodes.BRANCH_NOT_FOUND, "Branch not found");
        }
        if (filter.actorUserId() != null
                && userRepository.findByIdAndOrganizationId(filter.actorUserId(), organizationId).isEmpty()) {
            throw new BusinessException(
                    HttpStatus.NOT_FOUND, ErrorCodes.USER_NOT_FOUND, "User not found");
        }
        if (filter.from() != null && filter.to() != null && filter.from().isAfter(filter.to())) {
            throw new BusinessException(
                    HttpStatus.BAD_REQUEST,
                    ErrorCodes.VALIDATION_ERROR,
                    "from must not be after to");
        }
        return auditEventRepository.findAll(buildSpecification(organizationId, filter), pageable);
    }

    private static Specification<AuditEvent> buildSpecification(UUID organizationId, AuditEventFilter filter) {
        return (root, query, cb) -> {
            List<Predicate> predicates = new ArrayList<>();
            predicates.add(cb.equal(root.get("organizationId"), organizationId));
            if (filter.branchId() != null) {
                predicates.add(cb.equal(root.get("branchId"), filter.branchId()));
            }
            if (filter.actorUserId() != null) {
                predicates.add(cb.equal(root.get("actorUserId"), filter.actorUserId()));
            }
            if (filter.action() != null && !filter.action().isBlank()) {
                predicates.add(cb.equal(root.get("action"), filter.action().trim()));
            }
            if (filter.entityType() != null && !filter.entityType().isBlank()) {
                predicates.add(cb.equal(root.get("entityType"), filter.entityType().trim()));
            }
            if (filter.entityId() != null) {
                predicates.add(cb.equal(root.get("entityId"), filter.entityId()));
            }
            if (filter.from() != null) {
                predicates.add(cb.greaterThanOrEqualTo(root.get("occurredAt"), filter.from()));
            }
            if (filter.to() != null) {
                predicates.add(cb.lessThan(root.get("occurredAt"), filter.to()));
            }
            return cb.and(predicates.toArray(new Predicate[0]));
        };
    }
}