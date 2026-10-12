package com.hiqnet.pos.organization.application;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;

import com.hiqnet.pos.AbstractRbacIntegrationTest;
import com.hiqnet.pos.organization.domain.InventoryNegativeStockPolicy;
import com.hiqnet.pos.shared.audit.AuditException;
import com.hiqnet.pos.shared.audit.AuditPort;
import com.hiqnet.pos.shared.audit.AuditRecord;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.boot.test.context.SpringBootTest;
import org.springframework.boot.test.context.TestConfiguration;
import org.springframework.context.annotation.Bean;
import org.springframework.context.annotation.Import;
import org.springframework.context.annotation.Primary;
import org.springframework.jdbc.core.JdbcTemplate;
import org.springframework.test.context.ActiveProfiles;

/**
 * An audit failure during an Organization update must roll back BOTH the
 * negative stock policy change and the optimistic-lock version bump: the
 * business mutation and its AuditEvent are one atomic commit.
 */
@ActiveProfiles("test")
@Import(OrganizationAuditRollbackTest.FailingAuditConfig.class)
@SpringBootTest
class OrganizationAuditRollbackTest extends AbstractRbacIntegrationTest {

    @Autowired
    private OrganizationService organizationService;

    @Autowired
    private JdbcTemplate jdbc;

    @Test
    void failedAuditRollsBackNegativeStockPolicyAndVersion() {
        RbacFixture fixture = fixture();
        long versionBefore = jdbc.queryForObject(
                "select version from organizations where id = ?", Long.class, fixture.orgId());

        assertThatThrownBy(() -> organizationService.update(
                        fixture.orgId(),
                        new UpdateOrganizationCommand(
                                versionBefore,
                                null,
                                null,
                                null,
                                null,
                                null,
                                InventoryNegativeStockPolicy.ALLOW_NEGATIVE)))
                .isInstanceOf(AuditException.class);

        assertThat(jdbc.queryForObject(
                        "select inventory_negative_stock_policy from organizations where id = ?",
                        String.class,
                        fixture.orgId()))
                .isEqualTo("STRICT");
        assertThat(jdbc.queryForObject(
                        "select version from organizations where id = ?", Long.class, fixture.orgId()))
                .isEqualTo(versionBefore);
    }

    @TestConfiguration
    static class FailingAuditConfig {
        @Bean
        @Primary
        AuditPort failingAuditPort() {
            return (AuditRecord record) -> {
                throw new AuditException("audit unavailable (forced test failure)");
            };
        }
    }
}
