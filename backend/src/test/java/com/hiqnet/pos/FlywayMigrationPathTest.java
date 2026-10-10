package com.hiqnet.pos;

import static org.assertj.core.api.Assertions.assertThat;

import java.sql.Connection;
import java.sql.DriverManager;
import java.sql.PreparedStatement;
import java.sql.ResultSet;
import java.util.UUID;
import org.flywaydb.core.Flyway;
import org.flywaydb.core.api.MigrationVersion;
import org.flywaydb.core.api.output.MigrateResult;
import org.junit.jupiter.api.Test;
import org.testcontainers.containers.PostgreSQLContainer;
import org.testcontainers.junit.jupiter.Container;
import org.testcontainers.junit.jupiter.Testcontainers;

/**
 * Flyway path integrity on a REAL, empty PostgreSQL (independent of the Spring
 * contexts that reuse a Testcontainers DB across the suite):
 *
 * <ul>
 *   <li><b>Clean install</b>: V1 → V9 applies completely and validates, leaving
 *       the expected seed state (permission catalog = 25, bootstrap singleton = 1,
 *       catalog tables present).</li>
 *   <li><b>Upgrade path (V7)</b>: a database migrated up to V7 (the state before
 *       the Secure Admin Bootstrap slice) applies V8 and V9 correctly in
 *       isolation — the real production upgrade, not a recreated dataset.</li>
 *   <li><b>Upgrade path (V8)</b>: a database migrated up to V8 (the state before
 *       the Catalog Foundation slice) applies only V9, leaving existing data
 *       untouched.</li>
 * </ul>
 */
@Testcontainers
class FlywayMigrationPathTest {

    @Container
    static final PostgreSQLContainer<?> POSTGRES = new PostgreSQLContainer<>("postgres:17-alpine");

    @Test
    void cleanInstallationMigratesV1ThroughV9AndValidates() {
        Flyway flyway = flyway("clean_foundation");

        MigrateResult result = flyway.migrate();

        assertThat(result.migrationsExecuted).isEqualTo(9);
        flyway.validate();
        assertThat(countRows("clean_foundation", "permissions")).isEqualTo(25);
        assertThat(countRows("clean_foundation", "installation_bootstrap")).isEqualTo(1);
        assertThat(tableExists("clean_foundation", "categories")).isTrue();
        assertThat(tableExists("clean_foundation", "products")).isTrue();
    }

    @Test
    void upgradeFromV7AppliesV8AndV9WithoutRecreatingData() {
        // Stop the migration history at V7: the exact pre-bootstrap state.
        Flyway upToV7 = Flyway.configure()
                .dataSource(POSTGRES.getJdbcUrl(), POSTGRES.getUsername(), POSTGRES.getPassword())
                .schemas("upgrade_path")
                .target(MigrationVersion.fromVersion("7"))
                .load();
        MigrateResult migratedToV7 = upToV7.migrate();

        assertThat(migratedToV7.migrationsExecuted).isEqualTo(7);
        assertThat(tableExists("upgrade_path", "installation_bootstrap")).isFalse();
        assertThat(tableExists("upgrade_path", "categories")).isFalse();

        // A fresh Flyway run (like a new backend version) applies V8 and V9.
        Flyway toLatest = Flyway.configure()
                .dataSource(POSTGRES.getJdbcUrl(), POSTGRES.getUsername(), POSTGRES.getPassword())
                .schemas("upgrade_path")
                .load();
        MigrateResult migratedToLatest = toLatest.migrate();

        assertThat(migratedToLatest.migrationsExecuted).isEqualTo(2);
        toLatest.validate();
        assertThat(countRows("upgrade_path", "permissions")).isEqualTo(25);
        assertThat(countRows("upgrade_path", "installation_bootstrap")).isEqualTo(1);
        assertThat(tableExists("upgrade_path", "products")).isTrue();
    }

    @Test
    void upgradeFromV8AppliesV9WithoutRecreatingData() {
        // Stop the migration history at V8: the exact pre-catalog state.
        Flyway upToV8 = Flyway.configure()
                .dataSource(POSTGRES.getJdbcUrl(), POSTGRES.getUsername(), POSTGRES.getPassword())
                .schemas("upgrade_catalog")
                .target(MigrationVersion.fromVersion("8"))
                .load();
        MigrateResult migratedToV8 = upToV8.migrate();

        assertThat(migratedToV8.migrationsExecuted).isEqualTo(8);
        assertThat(countRows("upgrade_catalog", "permissions")).isEqualTo(19);
        assertThat(tableExists("upgrade_catalog", "categories")).isFalse();

        // A fresh Flyway run applies only V9.
        Flyway toLatest = Flyway.configure()
                .dataSource(POSTGRES.getJdbcUrl(), POSTGRES.getUsername(), POSTGRES.getPassword())
                .schemas("upgrade_catalog")
                .load();
        MigrateResult migratedToLatest = toLatest.migrate();

        assertThat(migratedToLatest.migrationsExecuted).isEqualTo(1);
        toLatest.validate();
        assertThat(countRows("upgrade_catalog", "permissions")).isEqualTo(25);
        assertThat(tableExists("upgrade_catalog", "categories")).isTrue();
        assertThat(tableExists("upgrade_catalog", "products")).isTrue();
    }

    @Test
    void upgradeFromV8PreservesExistingRoleGrantsAndOnlyAddsCatalogPermissions() {
        String schema = "upgrade_grants";
        // 1. Migrate to V8: the exact pre-catalog state.
        Flyway upToV8 = Flyway.configure()
                .dataSource(POSTGRES.getJdbcUrl(), POSTGRES.getUsername(), POSTGRES.getPassword())
                .schemas(schema)
                .target(MigrationVersion.fromVersion("8"))
                .load();
        assertThat(upToV8.migrate().migrationsExecuted).isEqualTo(8);
        assertThat(countRows(schema, "permissions")).isEqualTo(19);

        // 2. Simulate an existing deployment's administrator role: an organization
        //    plus a role granted EVERY permission present before V9 (19).
        UUID organizationId = UUID.randomUUID();
        UUID administratorRoleId = UUID.randomUUID();
        execute("insert into " + schema + ".organizations "
                + "(id, name, code, timezone, currency, locale, created_at, updated_at) values "
                + "('" + organizationId + "', 'Existing Org', 'existing-org', 'UTC', 'MXN', 'es-MX', now(), now())");
        execute("insert into " + schema + ".roles "
                + "(id, organization_id, code, name, created_at, updated_at) values "
                + "('" + administratorRoleId + "', '" + organizationId + "', 'administrator', 'Administrator', now(), now())");
        execute("insert into " + schema + ".role_permissions (role_id, permission_code) "
                + "select '" + administratorRoleId + "', code from " + schema + ".permissions");
        assertThat(countRows(schema, "role_permissions")).isEqualTo(19);

        // 3. Apply V9 (the real catalog upgrade).
        Flyway toLatest = Flyway.configure()
                .dataSource(POSTGRES.getJdbcUrl(), POSTGRES.getUsername(), POSTGRES.getPassword())
                .schemas(schema)
                .load();
        assertThat(toLatest.migrate().migrationsExecuted).isEqualTo(1);
        toLatest.validate();

        // 4. V9 ADDS the catalog permissions (19 -> 25) but NEVER rewrites an
        //    existing role's grants: upgrading never silently provisions access.
        assertThat(countRows(schema, "permissions")).isEqualTo(25);
        assertThat(countRows(schema, "role_permissions")).isEqualTo(19);
    }

    private static Flyway flyway(String schema) {
        return Flyway.configure()
                .dataSource(POSTGRES.getJdbcUrl(), POSTGRES.getUsername(), POSTGRES.getPassword())
                .schemas(schema)
                .load();
    }

    private static boolean tableExists(String schema, String table) {
        try (Connection connection = connect();
                PreparedStatement statement = connection.prepareStatement(
                        "select 1 from information_schema.tables where table_schema = ? and table_name = ?")) {
            statement.setString(1, schema);
            statement.setString(2, table);
            try (ResultSet resultSet = statement.executeQuery()) {
                return resultSet.next();
            }
        } catch (Exception e) {
            throw new IllegalStateException(e);
        }
    }

    private static long countRows(String schema, String table) {
        try (Connection connection = connect();
                PreparedStatement statement = connection.prepareStatement(
                        "select count(*) from " + schema + "." + table)) {
            try (ResultSet resultSet = statement.executeQuery()) {
                resultSet.next();
                return resultSet.getLong(1);
            }
        } catch (Exception e) {
            throw new IllegalStateException(e);
        }
    }

    private static Connection connect() throws Exception {
        return DriverManager.getConnection(
                POSTGRES.getJdbcUrl(), POSTGRES.getUsername(), POSTGRES.getPassword());
    }

    private static void execute(String sql) {
        try (Connection connection = connect();
                PreparedStatement statement = connection.prepareStatement(sql)) {
            statement.execute();
        } catch (Exception e) {
            throw new IllegalStateException(e);
        }
    }
}