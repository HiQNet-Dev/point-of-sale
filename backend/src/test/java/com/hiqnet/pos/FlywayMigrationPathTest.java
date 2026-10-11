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
 * <li><b>Clean install</b>: V1 → V15 applies completely and validates, leaving
 *       the expected seed state (permission catalog = 45, bootstrap singleton = 1,
 *       catalog/modifier/cash-register/cash-shift/sales tables present).</li>
 *   <li><b>Upgrade path (V7)</b>: a database migrated up to V7 (the state before
 *       the Secure Admin Bootstrap slice) applies V8 through V12 correctly
 *       in isolation — the real production upgrade, not a recreated dataset.</li>
 *   <li><b>Upgrade path (V8)</b>: a database migrated up to V8 applies V9 through
 *       V12, leaving existing data untouched.</li>
 *   <li><b>Grant preservation</b>: upgrading an existing deployment ADDS new
 *       permission codes but NEVER rewrites an existing role's grants (upgrade
 *       never silently provisions access).</li>
 * </ul>
 */
@Testcontainers
class FlywayMigrationPathTest {

    @Container
    static final PostgreSQLContainer<?> POSTGRES = new PostgreSQLContainer<>("postgres:17-alpine");

    @Test
    void cleanInstallationMigratesV1ThroughV15AndValidates() {
        Flyway flyway = flyway("clean_foundation");

        MigrateResult result = flyway.migrate();

        assertThat(result.migrationsExecuted).isEqualTo(15);
        flyway.validate();
        assertThat(countRows("clean_foundation", "permissions")).isEqualTo(45);
        assertThat(countRows("clean_foundation", "installation_bootstrap")).isEqualTo(1);
        assertThat(tableExists("clean_foundation", "categories")).isTrue();
        assertThat(tableExists("clean_foundation", "products")).isTrue();
        assertThat(tableExists("clean_foundation", "modifier_groups")).isTrue();
        assertThat(tableExists("clean_foundation", "modifier_options")).isTrue();
        assertThat(tableExists("clean_foundation", "product_modifier_groups")).isTrue();
        assertThat(tableExists("clean_foundation", "cash_registers")).isTrue();
        assertThat(tableExists("clean_foundation", "cash_shifts")).isTrue();
        assertThat(tableExists("clean_foundation", "sales")).isTrue();
        assertThat(tableExists("clean_foundation", "sale_items")).isTrue();
        assertThat(tableExists("clean_foundation", "sale_item_modifiers")).isTrue();
                assertThat(tableExists("clean_foundation", "payments")).isTrue();
    }

    @Test
    void upgradeFromV7AppliesV8ThroughV11WithoutRecreatingData() {
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
        assertThat(tableExists("upgrade_path", "modifier_groups")).isFalse();
        assertThat(tableExists("upgrade_path", "cash_registers")).isFalse();

        // A fresh Flyway run applies V8 through V15.
        Flyway toLatest = Flyway.configure()
                .dataSource(POSTGRES.getJdbcUrl(), POSTGRES.getUsername(), POSTGRES.getPassword())
                .schemas("upgrade_path")
                .load();
        MigrateResult migratedToLatest = toLatest.migrate();

        assertThat(migratedToLatest.migrationsExecuted).isEqualTo(8);
        toLatest.validate();
        assertThat(countRows("upgrade_path", "permissions")).isEqualTo(45);
        assertThat(countRows("upgrade_path", "installation_bootstrap")).isEqualTo(1);
        assertThat(tableExists("upgrade_path", "products")).isTrue();
        assertThat(tableExists("upgrade_path", "modifier_groups")).isTrue();
        assertThat(tableExists("upgrade_path", "cash_registers")).isTrue();
        assertThat(tableExists("upgrade_path", "cash_shifts")).isTrue();
    }

    @Test
    void upgradeFromV8AppliesV9ThroughV11WithoutRecreatingData() {
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

        // A fresh Flyway run applies only V9 through V15.
        Flyway toLatest = Flyway.configure()
                .dataSource(POSTGRES.getJdbcUrl(), POSTGRES.getUsername(), POSTGRES.getPassword())
                .schemas("upgrade_catalog")
                .load();
        MigrateResult migratedToLatest = toLatest.migrate();

        assertThat(migratedToLatest.migrationsExecuted).isEqualTo(7);
        toLatest.validate();
        assertThat(countRows("upgrade_catalog", "permissions")).isEqualTo(45);
        assertThat(tableExists("upgrade_catalog", "categories")).isTrue();
        assertThat(tableExists("upgrade_catalog", "products")).isTrue();
        assertThat(tableExists("upgrade_catalog", "modifier_groups")).isTrue();
        assertThat(tableExists("upgrade_catalog", "cash_registers")).isTrue();
        assertThat(tableExists("upgrade_catalog", "cash_shifts")).isTrue();
    }

    @Test
    void upgradeFromV8PreservesExistingRoleGrantsAndOnlyAddsCatalogModifierAndCashRegisterPermissions() {
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

        // 3. Apply V9 through V15, including operational catalog permission.
        Flyway toLatest = Flyway.configure()
                .dataSource(POSTGRES.getJdbcUrl(), POSTGRES.getUsername(), POSTGRES.getPassword())
                .schemas(schema)
                .load();
        assertThat(toLatest.migrate().migrationsExecuted).isEqualTo(7);
        toLatest.validate();

        // 4. The migrations add catalog through operational POS permission (19 -> 45) but
        //    NEVER rewrite an existing role's grants.
        assertThat(countRows(schema, "permissions")).isEqualTo(45);
        assertThat(countRows(schema, "role_permissions")).isEqualTo(19);
    }

    @Test
    void upgradeFromV9PreservesExistingRoleGrantsAndOnlyAddsModifierAndCashRegisterPermissions() {
        String schema = "upgrade_modifiers";
        // 1. Migrate to V9: the exact pre-modifiers state.
        Flyway upToV9 = Flyway.configure()
                .dataSource(POSTGRES.getJdbcUrl(), POSTGRES.getUsername(), POSTGRES.getPassword())
                .schemas(schema)
                .target(MigrationVersion.fromVersion("9"))
                .load();
        assertThat(upToV9.migrate().migrationsExecuted).isEqualTo(9);
        assertThat(countRows(schema, "permissions")).isEqualTo(25);
        assertThat(tableExists(schema, "modifier_groups")).isFalse();

        // 2. Existing deployment: an organization plus a role granted EVERY
        //    permission present before V10 (25).
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
        assertThat(countRows(schema, "role_permissions")).isEqualTo(25);

        // 3. Apply V10 through V15 without rewriting the existing role.
        Flyway toLatest = Flyway.configure()
                .dataSource(POSTGRES.getJdbcUrl(), POSTGRES.getUsername(), POSTGRES.getPassword())
                .schemas(schema)
                .load();
        assertThat(toLatest.migrate().migrationsExecuted).isEqualTo(6);
        toLatest.validate();

        // 4. V10 through V15 add permissions but never rewrite existing grants.
        assertThat(countRows(schema, "permissions")).isEqualTo(45);
        assertThat(countRows(schema, "role_permissions")).isEqualTo(25);
        assertThat(tableExists(schema, "modifier_groups")).isTrue();
        assertThat(tableExists(schema, "modifier_options")).isTrue();
        assertThat(tableExists(schema, "product_modifier_groups")).isTrue();
        assertThat(tableExists(schema, "cash_registers")).isTrue();
        assertThat(tableExists(schema, "cash_shifts")).isTrue();
    }

    @Test
    void upgradeFromV10PreservesExistingRoleGrantsAndOnlyAddsCashRegisterPermissions() {
        String schema = "upgrade_registers";
        // 1. Migrate to V10: the exact pre-cash-registers state.
        Flyway upToV10 = Flyway.configure()
                .dataSource(POSTGRES.getJdbcUrl(), POSTGRES.getUsername(), POSTGRES.getPassword())
                .schemas(schema)
                .target(MigrationVersion.fromVersion("10"))
                .load();
        assertThat(upToV10.migrate().migrationsExecuted).isEqualTo(10);
        assertThat(countRows(schema, "permissions")).isEqualTo(32);
        assertThat(tableExists(schema, "cash_registers")).isFalse();

        // 2. Existing deployment: an organization plus a role granted EVERY
        //    permission present before V11 (32).
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
        assertThat(countRows(schema, "role_permissions")).isEqualTo(32);

        // 3. Apply V11 through V15 without rewriting the existing role.
        Flyway toLatest = Flyway.configure()
                .dataSource(POSTGRES.getJdbcUrl(), POSTGRES.getUsername(), POSTGRES.getPassword())
                .schemas(schema)
                .load();
        assertThat(toLatest.migrate().migrationsExecuted).isEqualTo(5);
        toLatest.validate();

        // 4. V11 through V15 add permissions but never rewrite existing grants.
        assertThat(countRows(schema, "permissions")).isEqualTo(45);
        assertThat(countRows(schema, "role_permissions")).isEqualTo(32);
        assertThat(tableExists(schema, "cash_registers")).isTrue();
        assertThat(tableExists(schema, "cash_shifts")).isTrue();
    }

    @Test
    void upgradeFromV11PreservesExistingRoleGrantsAndOnlyAddsCashShiftPermissions() {
        String schema = "upgrade_shifts";
        // 1. Migrate to V11: the exact pre-cash-shifts state.
        Flyway upToV11 = Flyway.configure()
                .dataSource(POSTGRES.getJdbcUrl(), POSTGRES.getUsername(), POSTGRES.getPassword())
                .schemas(schema)
                .target(MigrationVersion.fromVersion("11"))
                .load();
        assertThat(upToV11.migrate().migrationsExecuted).isEqualTo(11);
        assertThat(countRows(schema, "permissions")).isEqualTo(35);
        assertThat(tableExists(schema, "cash_registers")).isTrue();
        assertThat(tableExists(schema, "cash_shifts")).isFalse();

        // 2. Existing deployment: an organization plus a role granted EVERY
        //    permission present before V12 (35).
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
        assertThat(countRows(schema, "role_permissions")).isEqualTo(35);

        // 3. Apply V12 through V15 without rewriting the existing role.
        Flyway toLatest = Flyway.configure()
                .dataSource(POSTGRES.getJdbcUrl(), POSTGRES.getUsername(), POSTGRES.getPassword())
                .schemas(schema)
                .load();
        assertThat(toLatest.migrate().migrationsExecuted).isEqualTo(4);
        toLatest.validate();

        // 4. V12-V15 add permissions but NEVER rewrite existing grants.
        assertThat(countRows(schema, "permissions")).isEqualTo(45);
        assertThat(countRows(schema, "role_permissions")).isEqualTo(35);
        assertThat(tableExists(schema, "cash_registers")).isTrue();
        assertThat(tableExists(schema, "cash_shifts")).isTrue();
    }

    @Test
    void upgradeFromV12AddsSaleAndCheckoutPermissionsWithoutGrantingExistingRoles() {
        String schema = "upgrade_sales";
        Flyway upToV12 = Flyway.configure()
                .dataSource(POSTGRES.getJdbcUrl(), POSTGRES.getUsername(), POSTGRES.getPassword())
                .schemas(schema)
                .target(MigrationVersion.fromVersion("12"))
                .load();
        assertThat(upToV12.migrate().migrationsExecuted).isEqualTo(12);
        UUID organizationId = UUID.randomUUID();
        UUID roleId = UUID.randomUUID();
        execute("insert into " + schema + ".organizations "
                + "(id, name, code, timezone, currency, locale, created_at, updated_at) values "
                + "('" + organizationId + "', 'Existing Org', 'existing-org', 'UTC', 'MXN', 'es-MX', now(), now())");
        execute("insert into " + schema + ".roles "
                + "(id, organization_id, code, name, created_at, updated_at) values "
                + "('" + roleId + "', '" + organizationId + "', 'administrator', 'Administrator', now(), now())");
        execute("insert into " + schema + ".role_permissions (role_id, permission_code) "
                + "select '" + roleId + "', code from " + schema + ".permissions");
        assertThat(countRows(schema, "role_permissions")).isEqualTo(38);

        Flyway toLatest = Flyway.configure()
                .dataSource(POSTGRES.getJdbcUrl(), POSTGRES.getUsername(), POSTGRES.getPassword())
                .schemas(schema)
                .load();
        assertThat(toLatest.migrate().migrationsExecuted).isEqualTo(3);
        toLatest.validate();
        assertThat(countRows(schema, "permissions")).isEqualTo(45);
        assertThat(countRows(schema, "role_permissions")).isEqualTo(38);
        assertThat(tableExists(schema, "sales")).isTrue();
        assertThat(tableExists(schema, "payments")).isTrue();
    }

    @Test
    void upgradeFromV13AddsOnlyCheckoutPermissionsAndPaymentSchemaWithoutChangingExistingGrants() {
        String schema = "upgrade_checkout";
        Flyway upToV13 = Flyway.configure()
                .dataSource(POSTGRES.getJdbcUrl(), POSTGRES.getUsername(), POSTGRES.getPassword())
                .schemas(schema)
                .target(MigrationVersion.fromVersion("13"))
                .load();
        assertThat(upToV13.migrate().migrationsExecuted).isEqualTo(13);
        assertThat(countRows(schema, "permissions")).isEqualTo(42);
        UUID organizationId = UUID.randomUUID();
        UUID roleId = UUID.randomUUID();
        execute("insert into " + schema + ".organizations "
                + "(id, name, code, timezone, currency, locale, created_at, updated_at) values "
                + "('" + organizationId + "', 'Existing Org', 'existing-checkout', 'UTC', 'MXN', 'es-MX', now(), now())");
        execute("insert into " + schema + ".roles "
                + "(id, organization_id, code, name, created_at, updated_at) values "
                + "('" + roleId + "', '" + organizationId + "', 'existing-role', 'Existing role', now(), now())");
        execute("insert into " + schema + ".role_permissions (role_id, permission_code) "
                + "select '" + roleId + "', code from " + schema + ".permissions");
        assertThat(countRows(schema, "role_permissions")).isEqualTo(42);

        Flyway toV14 = Flyway.configure()
                .dataSource(POSTGRES.getJdbcUrl(), POSTGRES.getUsername(), POSTGRES.getPassword())
                .schemas(schema)
                .target(MigrationVersion.fromVersion("14"))
                .load();
        assertThat(toV14.migrate().migrationsExecuted).isEqualTo(1);
        toV14.validate();
        assertThat(countRows(schema, "permissions")).isEqualTo(44);
        assertThat(countRows(schema, "role_permissions")).isEqualTo(42);
        assertThat(tableExists(schema, "payments")).isTrue();
        assertThat(columnExists(schema, "sales", "completed_at")).isTrue();
    }

    @Test
    void upgradeFromV14AddsOperationalCatalogPermissionWithoutChangingExistingGrants() {
        String schema = "upgrade_pos_catalog";
        Flyway upToV14 = Flyway.configure()
                .dataSource(POSTGRES.getJdbcUrl(), POSTGRES.getUsername(), POSTGRES.getPassword())
                .schemas(schema)
                .target(MigrationVersion.fromVersion("14"))
                .load();
        assertThat(upToV14.migrate().migrationsExecuted).isEqualTo(14);
        assertThat(countRows(schema, "permissions")).isEqualTo(44);

        UUID organizationId = UUID.randomUUID();
        UUID roleId = UUID.randomUUID();
        execute("insert into " + schema + ".organizations "
                + "(id, name, code, timezone, currency, locale, created_at, updated_at) values "
                + "('" + organizationId + "', 'Existing Org', 'existing-pos-catalog', 'UTC', 'MXN', 'es-MX', now(), now())");
        execute("insert into " + schema + ".roles "
                + "(id, organization_id, code, name, created_at, updated_at) values "
                + "('" + roleId + "', '" + organizationId + "', 'existing-role', 'Existing role', now(), now())");
        execute("insert into " + schema + ".role_permissions (role_id, permission_code) "
                + "select '" + roleId + "', code from " + schema + ".permissions");

        Flyway toV15 = Flyway.configure()
                .dataSource(POSTGRES.getJdbcUrl(), POSTGRES.getUsername(), POSTGRES.getPassword())
                .schemas(schema)
                .load();
        assertThat(toV15.migrate().migrationsExecuted).isEqualTo(1);
        toV15.validate();
        assertThat(countRows(schema, "permissions")).isEqualTo(45);
        assertThat(countRows(schema, "role_permissions")).isEqualTo(44);
    }

    private static boolean columnExists(String schema, String table, String column) {
        try (Connection connection = connect();
                PreparedStatement statement = connection.prepareStatement(
                        "select 1 from information_schema.columns where table_schema = ? and table_name = ? and column_name = ?")) {
            statement.setString(1, schema);
            statement.setString(2, table);
            statement.setString(3, column);
            try (ResultSet resultSet = statement.executeQuery()) {
                return resultSet.next();
            }
        } catch (Exception e) {
            throw new IllegalStateException(e);
        }
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
