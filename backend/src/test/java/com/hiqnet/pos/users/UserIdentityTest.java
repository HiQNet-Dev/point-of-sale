package com.hiqnet.pos.users;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;

import com.hiqnet.pos.TestcontainersConfiguration;
import com.hiqnet.pos.organization.domain.Organization;
import com.hiqnet.pos.organization.domain.OrganizationRepository;
import com.hiqnet.pos.users.domain.User;
import com.hiqnet.pos.users.domain.UserRepository;
import java.util.UUID;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.boot.test.context.SpringBootTest;
import org.springframework.context.annotation.Import;
import org.springframework.dao.DataIntegrityViolationException;
import org.springframework.security.crypto.password.PasswordEncoder;
import org.springframework.test.context.ActiveProfiles;
import org.springframework.transaction.annotation.Transactional;

/**
 * User identity invariants against a real PostgreSQL: normalization, per-
 * organization email uniqueness (never global) and password hashing.
 * Transactional: the lookup used by login takes a pessimistic row lock.
 */
@ActiveProfiles("test")
@Import({TestcontainersConfiguration.class})
@SpringBootTest
@Transactional
class UserIdentityTest {

    @Autowired
    private OrganizationRepository organizationRepository;

    @Autowired
    private UserRepository userRepository;

    @Autowired
    private PasswordEncoder passwordEncoder;

    private String createOrganization() {
        Organization org = Organization.create(
                "Org", "org-" + UUID.randomUUID().toString().substring(0, 8), "UTC", "MXN", "es-MX");
        organizationRepository.saveAndFlush(org);
        return org.getId().toString();
    }

    @Test
    void emailIsNormalizedTrimmedAndLowercase() {
        String orgId = createOrganization();
        String rawEmail = "  Admin@Example.COM  ";
        User user = User.create(UUID.fromString(orgId), rawEmail, passwordEncoder.encode("secret-" + UUID.randomUUID()));
        userRepository.saveAndFlush(user);

        assertThat(user.getEmail()).isEqualTo("admin@example.com");
        assertThat(userRepository.findByOrganizationIdAndEmail(UUID.fromString(orgId), "ADMIN@EXAMPLE.COM"))
                .isPresent();
    }

    @Test
    void sameEmailCannotRepeatWithinOrganization() {
        String orgId = createOrganization();
        userRepository.saveAndFlush(User.create(UUID.fromString(orgId), "dup@example.com", passwordEncoder.encode("a-1")));

        assertThatThrownBy(() -> userRepository.saveAndFlush(
                        User.create(UUID.fromString(orgId), "dup@example.com", passwordEncoder.encode("a-2"))))
                .isInstanceOf(DataIntegrityViolationException.class);
    }

    @Test
    void sameEmailCanExistInDifferentOrganizations() {
        String orgA = createOrganization();
        String orgB = createOrganization();
        userRepository.saveAndFlush(User.create(UUID.fromString(orgA), "shared@example.com", passwordEncoder.encode("a-1")));
        userRepository.saveAndFlush(User.create(UUID.fromString(orgB), "shared@example.com", passwordEncoder.encode("a-2")));

        assertThat(userRepository.findByOrganizationIdAndEmail(UUID.fromString(orgA), "shared@example.com")).isPresent();
        assertThat(userRepository.findByOrganizationIdAndEmail(UUID.fromString(orgB), "shared@example.com")).isPresent();
    }

    @Test
    void plaintextPasswordIsNeverPersisted() {
        String orgId = createOrganization();
        String plaintext = "K4#super-secret-password";
        User user = User.create(UUID.fromString(orgId), "plain@example.com", passwordEncoder.encode(plaintext));
        userRepository.saveAndFlush(user);

        User persisted = userRepository.findById(user.getId()).orElseThrow();
        assertThat(persisted.getPasswordHash()).isNotEqualTo(plaintext).doesNotContain(plaintext);
        assertThat(persisted.getPasswordHash()).startsWith("$2a$");
        assertThat(persisted.getPasswordHash()).hasSizeGreaterThanOrEqualTo(60);
    }
}