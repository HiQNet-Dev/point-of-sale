package com.hiqnet.pos.organization.domain;

import java.util.Optional;
import java.util.UUID;
import org.springframework.data.jpa.repository.JpaRepository;

public interface OrganizationRepository extends JpaRepository<Organization, UUID> {

    boolean existsByCode(String code);

    Optional<Organization> findByCode(String code);

    Optional<Organization> findById(UUID id);
}