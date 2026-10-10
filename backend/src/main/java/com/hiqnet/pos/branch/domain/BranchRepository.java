package com.hiqnet.pos.branch.domain;

import java.util.List;
import java.util.Optional;
import java.util.UUID;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.data.jpa.repository.JpaSpecificationExecutor;

public interface BranchRepository extends JpaRepository<Branch, UUID>, JpaSpecificationExecutor<Branch> {

    /** Scoped lookup: prevents accidental cross-organization access (IDOR). */
    Optional<Branch> findByIdAndOrganizationId(UUID id, UUID organizationId);

    List<Branch> findAllByOrganizationId(UUID organizationId);

    boolean existsByOrganizationIdAndCode(UUID organizationId, String code);
}