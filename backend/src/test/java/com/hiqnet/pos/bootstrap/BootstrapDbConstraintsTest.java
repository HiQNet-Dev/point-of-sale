package com.hiqnet.pos.bootstrap;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;

import com.hiqnet.pos.organization.domain.Organization;
import com.hiqnet.pos.rbac.domain.Role;
import com.hiqnet.pos.users.domain.User;
import java.sql.Timestamp;
import java.time.Instant;
import java.util.UUID;
import org.junit.jupiter.api.Test;
import org.springframework.dao.DataIntegrityViolationException;

/**
 * The bootstrap invariants are enforced DIRECTLY by PostgreSQL, never only by
 * Java: the singleton row (id=1 CHECK), the all-or-nothing completion CHECK and
 * the same-organization actor FKs (a bootstrap Organization A cannot reference
 * an administrator User of Organization B).
 */
class BootstrapDbConstraintsTest extends AbstractBootstrapIntegrationTest {

    @Test
    void aSecondBootstrapRowIsRejectedByTheSingletonCheck() {
        assertThatThrownBy(() -> jdbcTemplate.update(
                        "insert into installation_bootstrap (id, completed_at) values (2, ?)",
                        now()))
                .isInstanceOf(DataIntegrityViolationException.class);
    }

    @Test
    void partialCompletionStateIsRejected() {
        // completed_at set but identity columns still NULL -> CHECK violation.
        assertThatThrownBy(() -> jdbcTemplate.update(
                        "update installation_bootstrap set completed_at = ? where id = 1",
                        now()))
                .isInstanceOf(DataIntegrityViolationException.class);
    }

    @Test
    void crossOrganizationAdministratorUserIsRejectedByTheCompositeFk() {
        Organization orgA = organizationRepository.saveAndFlush(
                Organization.create("Org A", "org-a", "America/Mexico_City", "MXN", "es-MX"));
        Organization orgB = organizationRepository.saveAndFlush(
                Organization.create("Org B", "org-b", "America/Mexico_City", "MXN", "es-MX"));
        Role roleInA = roleRepository.saveAndFlush(Role.create(orgA.getId(), "role-a", "Role A"));
        User userInB = userRepository.saveAndFlush(
                User.create(orgB.getId(), uniqueEmail("cross@"), passwordEncoder.encode(PASSWORD)));

        // Bootstrap Organization A + administrator User of Organization B:
        // the composite FK (administrator_user_id, organization_id) must fail.
        assertThatThrownBy(() -> jdbcTemplate.update(
                        "update installation_bootstrap set completed_at = ?, organization_id = ?, "
                                + "administrator_user_id = ?, administrator_role_id = ? where id = 1",
                        now(),
                        orgA.getId(),
                        userInB.getId(),
                        roleInA.getId()))
                .isInstanceOf(DataIntegrityViolationException.class);
    }

    @Test
    void crossOrganizationAdministratorRoleIsRejectedByTheCompositeFk() {
        Organization orgA = organizationRepository.saveAndFlush(
                Organization.create("Org A", "org-a", "America/Mexico_City", "MXN", "es-MX"));
        Organization orgB = organizationRepository.saveAndFlush(
                Organization.create("Org B", "org-b", "America/Mexico_City", "MXN", "es-MX"));
        User userInA = userRepository.saveAndFlush(
                User.create(orgA.getId(), uniqueEmail("userA@"), passwordEncoder.encode(PASSWORD)));
        Role roleInB = roleRepository.saveAndFlush(Role.create(orgB.getId(), "role-b", "Role B"));

        assertThatThrownBy(() -> jdbcTemplate.update(
                        "update installation_bootstrap set completed_at = ?, organization_id = ?, "
                                + "administrator_user_id = ?, administrator_role_id = ? where id = 1",
                        now(),
                        orgA.getId(),
                        userInA.getId(),
                        roleInB.getId()))
                .isInstanceOf(DataIntegrityViolationException.class);
    }

    @Test
    void fullyCompletedStateWithSameOrganizationPasses() {
        Organization orgA = organizationRepository.saveAndFlush(
                Organization.create("Org A", "org-a", "America/Mexico_City", "MXN", "es-MX"));
        Role roleInA = roleRepository.saveAndFlush(Role.create(orgA.getId(), "role-a", "Role A"));
        User userInA = userRepository.saveAndFlush(
                User.create(orgA.getId(), uniqueEmail("userA@"), passwordEncoder.encode(PASSWORD)));

        int updated = jdbcTemplate.update(
                "update installation_bootstrap set completed_at = ?, organization_id = ?, "
                        + "administrator_user_id = ?, administrator_role_id = ? where id = 1",
                now(),
                orgA.getId(),
                userInA.getId(),
                roleInA.getId());
        assertThat(updated).isEqualTo(1);
        assertThatInstallationIsCompleted(orgA.getId(), userInA.getId(), roleInA.getId());
    }

    private static Timestamp now() {
        return Timestamp.from(Instant.now());
    }

    private void assertThatInstallationIsCompleted(UUID organizationId, UUID userId, UUID roleId) {
        UUID storedOrganizationId = jdbcTemplate.queryForObject(
                "select organization_id from installation_bootstrap where id = 1", UUID.class);
        assertThat(storedOrganizationId).isEqualTo(organizationId);
        Boolean completed = jdbcTemplate.queryForObject(
                "select completed_at is not null from installation_bootstrap where id = 1", Boolean.class);
        assertThat(completed).isTrue();
        // identity columns must be present (rows exist)
        assertThat(userId).isNotNull();
        assertThat(roleId).isNotNull();
    }
}