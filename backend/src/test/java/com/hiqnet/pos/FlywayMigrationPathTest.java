package com.hiqnet.pos;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;

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
 * <li><b>Clean install</b>: V1 → V21 applies completely and validates, leaving
 *       the expected seed state (permission catalog = 51, bootstrap singleton = 1,
 *       catalog/modifier/cash-register/cash-shift/sales and inventory-item tables present).</li>
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
    void cleanInstallationMigratesV1ThroughV21AndValidates() {
        Flyway flyway = flyway("clean_foundation");

        MigrateResult result = flyway.migrate();

        assertThat(result.migrationsExecuted).isEqualTo(21);
        flyway.validate();
        assertThat(countRows("clean_foundation", "permissions")).isEqualTo(51);
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
        assertThat(tableExists("clean_foundation", "branch_inventory")).isTrue();
        assertThat(tableExists("clean_foundation", "inventory_movements")).isTrue();
        assertThat(tableExists("clean_foundation", "inventory_items")).isTrue();
        assertThat(tableExists("clean_foundation", "product_inventory_recipes")).isTrue();
        assertThat(tableExists("clean_foundation", "product_inventory_recipe_components")).isTrue();
        assertThat(tableExists("clean_foundation", "modifier_option_inventory_configs")).isTrue();
        assertThat(tableExists("clean_foundation", "modifier_option_inventory_effects")).isTrue();
        assertThat(tableExists("clean_foundation", "sale_item_inventory_components")).isTrue();
        assertThat(columnExists("clean_foundation", "organizations", "inventory_negative_stock_policy"))
                .isTrue();
        assertThat(columnExists("clean_foundation", "inventory_movements", "sale_id")).isTrue();
    }

    @Test
    void upgradeFromV7AppliesV8ThroughV15WithoutRecreatingData() {
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
                .target(MigrationVersion.fromVersion("15"))
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
    void upgradeFromV8AppliesV9ThroughV15WithoutRecreatingData() {
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
                .target(MigrationVersion.fromVersion("15"))
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
                .target(MigrationVersion.fromVersion("15"))
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
                .target(MigrationVersion.fromVersion("15"))
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
                .target(MigrationVersion.fromVersion("15"))
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
                .target(MigrationVersion.fromVersion("15"))
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
                .target(MigrationVersion.fromVersion("15"))
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
                .target(MigrationVersion.fromVersion("15"))
                .load();
        assertThat(toV15.migrate().migrationsExecuted).isEqualTo(1);
        toV15.validate();
        assertThat(countRows(schema, "permissions")).isEqualTo(45);
        assertThat(countRows(schema, "role_permissions")).isEqualTo(44);
    }

    @Test
    void upgradeFromV15AddsOnlyInventoryItemPermissionsWithoutChangingExistingGrants() {
        String schema = "upgrade_inventory_items";
        Flyway upToV15 = Flyway.configure()
                .dataSource(POSTGRES.getJdbcUrl(), POSTGRES.getUsername(), POSTGRES.getPassword())
                .schemas(schema)
                .target(MigrationVersion.fromVersion("15"))
                .load();
        assertThat(upToV15.migrate().migrationsExecuted).isEqualTo(15);
        assertThat(countRows(schema, "permissions")).isEqualTo(45);

        UUID organizationId = UUID.randomUUID();
        UUID roleId = UUID.randomUUID();
        execute("insert into " + schema + ".organizations "
                + "(id, name, code, timezone, currency, locale, created_at, updated_at) values "
                + "('" + organizationId + "', 'Existing Org', 'existing-inventory', 'UTC', 'MXN', 'es-MX', now(), now())");
        execute("insert into " + schema + ".roles "
                + "(id, organization_id, code, name, created_at, updated_at) values "
                + "('" + roleId + "', '" + organizationId + "', 'existing-role', 'Existing role', now(), now())");
        execute("insert into " + schema + ".role_permissions (role_id, permission_code) "
                + "select '" + roleId + "', code from " + schema + ".permissions");

        Flyway toV16 = Flyway.configure()
                .dataSource(POSTGRES.getJdbcUrl(), POSTGRES.getUsername(), POSTGRES.getPassword())
                .schemas(schema)
                .target(MigrationVersion.fromVersion("16"))
                .load();
        assertThat(toV16.migrate().migrationsExecuted).isEqualTo(1);
        toV16.validate();
        assertThat(countRows(schema, "permissions")).isEqualTo(47);
        assertThat(countRows(schema, "role_permissions")).isEqualTo(45);
        assertThat(countRows(schema, "inventory_items")).isZero();
    }

    @Test
    void upgradeFromV16AddsOnlyBranchStockPermissionsAndPreservesExistingRoleGrants() {
        String schema = "upgrade_branch_inventory";
        Flyway upToV16 = Flyway.configure()
                .dataSource(POSTGRES.getJdbcUrl(), POSTGRES.getUsername(), POSTGRES.getPassword())
                .schemas(schema)
                .target(MigrationVersion.fromVersion("16"))
                .load();
        assertThat(upToV16.migrate().migrationsExecuted).isEqualTo(16);
        assertThat(countRows(schema, "permissions")).isEqualTo(47);
        UUID organizationId = UUID.randomUUID();
        UUID roleId = UUID.randomUUID();
        execute("insert into " + schema + ".organizations (id,name,code,timezone,currency,locale,created_at,updated_at) values ('" + organizationId + "','Existing Org','existing-stock','UTC','MXN','es-MX',now(),now())");
        execute("insert into " + schema + ".roles (id,organization_id,code,name,created_at,updated_at) values ('" + roleId + "','" + organizationId + "','existing-role','Existing role',now(),now())");
        execute("insert into " + schema + ".role_permissions (role_id,permission_code) select '" + roleId + "',code from " + schema + ".permissions");
        assertThat(countRows(schema, "role_permissions")).isEqualTo(47);

        Flyway latest = Flyway.configure()
                .dataSource(POSTGRES.getJdbcUrl(), POSTGRES.getUsername(), POSTGRES.getPassword())
                .schemas(schema)
                .target(MigrationVersion.fromVersion("17"))
                .load();
        assertThat(latest.migrate().migrationsExecuted).isEqualTo(1);
        latest.validate();
        assertThat(countRows(schema, "permissions")).isEqualTo(49);
        assertThat(countRows(schema, "role_permissions")).isEqualTo(47);
        assertThat(tableExists(schema, "branch_inventory")).isTrue();
        assertThat(tableExists(schema, "inventory_movements")).isTrue();
    }

    @Test
    void upgradeFromV17AddsOnlyRecipePermissionsAndPreservesExistingRoleGrants() {
        String schema = "upgrade_inventory_recipes";
        Flyway upToV17 = Flyway.configure()
                .dataSource(POSTGRES.getJdbcUrl(), POSTGRES.getUsername(), POSTGRES.getPassword())
                .schemas(schema)
                .target(MigrationVersion.fromVersion("17"))
                .load();
        assertThat(upToV17.migrate().migrationsExecuted).isEqualTo(17);
        assertThat(countRows(schema, "permissions")).isEqualTo(49);

        UUID organizationId = UUID.randomUUID();
        UUID roleId = UUID.randomUUID();
        execute("insert into " + schema + ".organizations (id,name,code,timezone,currency,locale,created_at,updated_at) values ('" + organizationId + "','Existing Org','existing-recipes','UTC','MXN','es-MX',now(),now())");
        execute("insert into " + schema + ".roles (id,organization_id,code,name,created_at,updated_at) values ('" + roleId + "','" + organizationId + "','existing-role','Existing role',now(),now())");
        execute("insert into " + schema + ".role_permissions (role_id,permission_code) select '" + roleId + "',code from " + schema + ".permissions");

        Flyway throughV18 = Flyway.configure()
                .dataSource(POSTGRES.getJdbcUrl(), POSTGRES.getUsername(), POSTGRES.getPassword())
                .schemas(schema)
                .target(MigrationVersion.fromVersion("18"))
                .load();
        assertThat(throughV18.migrate().migrationsExecuted).isEqualTo(1);
        throughV18.validate();
        assertThat(countRows(schema, "permissions")).isEqualTo(51);
        assertThat(countRows(schema, "role_permissions")).isEqualTo(49);
        assertThat(tableExists(schema, "product_inventory_recipes")).isTrue();
        assertThat(tableExists(schema, "product_inventory_recipe_components")).isTrue();
        assertThat(tableExists(schema, "modifier_option_inventory_configs")).isTrue();
        assertThat(tableExists(schema, "modifier_option_inventory_effects")).isTrue();

        UUID userId = UUID.randomUUID();
        UUID branchId = UUID.randomUUID();
        UUID registerId = UUID.randomUUID();
        UUID shiftId = UUID.randomUUID();
        UUID productId = UUID.randomUUID();
        UUID saleId = UUID.randomUUID();
        UUID saleItemId = UUID.randomUUID();
        execute("insert into " + schema + ".users (id,organization_id,email,password_hash,created_at,updated_at) values ('"
                + userId + "','" + organizationId + "','legacy@example.test','unused',now(),now())");
        execute("insert into " + schema + ".branches (id,organization_id,name,code,created_at,updated_at) values ('"
                + branchId + "','" + organizationId + "','Legacy branch','legacy-branch',now(),now())");
        execute("insert into " + schema + ".cash_registers (id,organization_id,branch_id,code,name,created_at,updated_at) values ('"
                + registerId + "','" + organizationId + "','" + branchId + "','legacy-register','Legacy register',now(),now())");
        execute("insert into " + schema + ".cash_shifts (id,organization_id,branch_id,cash_register_id,opened_by_user_id,status,opening_amount,opened_at,created_at,updated_at) values ('"
                + shiftId + "','" + organizationId + "','" + branchId + "','" + registerId + "','" + userId + "','OPEN',0,now(),now(),now())");
        execute("insert into " + schema + ".products (id,organization_id,sku,name,unit_price,created_at,updated_at) values ('"
                + productId + "','" + organizationId + "','LEGACY','Legacy product',1,now(),now())");
        execute("insert into " + schema + ".sales (id,organization_id,branch_id,cash_register_id,cash_shift_id,created_by_user_id,status,total_amount,created_at,updated_at) values ('"
                + saleId + "','" + organizationId + "','" + branchId + "','" + registerId + "','" + shiftId + "','" + userId + "','OPEN',1,now(),now())");
        execute("insert into " + schema + ".sale_items (id,organization_id,sale_id,product_id,product_sku_snapshot,product_name_snapshot,base_unit_price,modifier_unit_delta,effective_unit_price,quantity,line_total,status,position,created_at,updated_at) values ('"
                + saleItemId + "','" + organizationId + "','" + saleId + "','" + productId + "','LEGACY','Legacy product',1,0,1,1,1,'ACTIVE',0,now(),now())");
        assertThat(countRows(schema, "sale_items")).isEqualTo(1);

        Flyway latest = Flyway.configure()
                .dataSource(POSTGRES.getJdbcUrl(), POSTGRES.getUsername(), POSTGRES.getPassword())
                .schemas(schema)
                .load();
        assertThat(latest.migrate().migrationsExecuted).isEqualTo(3);
        latest.validate();
        assertThat(countRows(schema, "permissions")).isEqualTo(51);
        assertThat(countRows(schema, "role_permissions")).isEqualTo(49);
        assertThat(tableExists(schema, "sale_item_inventory_components")).isTrue();
        assertThat(countRows(schema, "sale_item_inventory_components")).isZero();
        assertThat(columnExists(schema, "organizations", "inventory_negative_stock_policy"))
                .isTrue();
        assertThat(columnExists(schema, "inventory_movements", "sale_id")).isTrue();
    }

    @Test
    void upgradeFromV19AddsNegativeStockPolicyColumnWithStrictDefaultWithoutChangingGrants() {
        String schema = "upgrade_negative_stock_policy";
        // 1. Migrate to V19: the exact pre-policy state.
        Flyway upToV19 = Flyway.configure()
                .dataSource(POSTGRES.getJdbcUrl(), POSTGRES.getUsername(), POSTGRES.getPassword())
                .schemas(schema)
                .target(MigrationVersion.fromVersion("19"))
                .load();
        assertThat(upToV19.migrate().migrationsExecuted).isEqualTo(19);
        assertThat(countRows(schema, "permissions")).isEqualTo(51);
        assertThat(columnExists(schema, "organizations", "inventory_negative_stock_policy")).isFalse();

        // 2. Existing deployment: one organization plus a role granted EVERY
        //    permission present before V20 (51).
        UUID organizationId = UUID.randomUUID();
        UUID roleId = UUID.randomUUID();
        execute("insert into " + schema + ".organizations (id,name,code,timezone,currency,locale,created_at,updated_at) values ('"
                + organizationId + "','Existing Org','existing-policy','UTC','MXN','es-MX',now(),now())");
        execute("insert into " + schema + ".roles (id,organization_id,code,name,created_at,updated_at) values ('"
                + roleId + "','" + organizationId + "','existing-role','Existing role',now(),now())");
        execute("insert into " + schema + ".role_permissions (role_id,permission_code) select '" + roleId + "',code from " + schema + ".permissions");
        assertThat(countRows(schema, "role_permissions")).isEqualTo(51);

        // 3. Apply V20.
        Flyway toV20 = Flyway.configure()
                .dataSource(POSTGRES.getJdbcUrl(), POSTGRES.getUsername(), POSTGRES.getPassword())
                .schemas(schema)
                .target(MigrationVersion.fromVersion("20"))
                .load();
        assertThat(toV20.migrate().migrationsExecuted).isEqualTo(1);
        toV20.validate();

        // 4. V20 adds the column (STRICT default for existing rows) without
        //    touching the permission catalog or existing role grants.
        assertThat(countRows(schema, "permissions")).isEqualTo(51);
        assertThat(countRows(schema, "role_permissions")).isEqualTo(51);
        assertThat(columnExists(schema, "organizations", "inventory_negative_stock_policy")).isTrue();
        assertThat(valueOfString(schema, "organizations", "inventory_negative_stock_policy", organizationId))
                .isEqualTo("STRICT");

        // 5. The CHECK rejects an unknown value and NOT NULL rejects NULL on
        //    direct SQL.
        assertThatThrownBy(() -> execute("update " + schema + ".organizations set inventory_negative_stock_policy = 'INVALID' where id = '"
                        + organizationId + "'"))
                .isInstanceOf(IllegalStateException.class);
        assertThatThrownBy(() -> execute("insert into " + schema + ".organizations (id,name,code,timezone,currency,locale,inventory_negative_stock_policy,created_at,updated_at) values ('"
                        + UUID.randomUUID() + "','Null Policy','null-policy','UTC','MXN','es-MX',null,now(),now())"))
                .isInstanceOf(IllegalStateException.class);
    }

    @Test
    void upgradeFromV20AddsSaleConsumptionLinkageWithoutChangingGrants() {
        String schema = "upgrade_sale_consumption";
        // 1. Migrate to V20: the exact pre-consumption state.
        Flyway upToV20 = Flyway.configure()
                .dataSource(POSTGRES.getJdbcUrl(), POSTGRES.getUsername(), POSTGRES.getPassword())
                .schemas(schema)
                .target(MigrationVersion.fromVersion("20"))
                .load();
        assertThat(upToV20.migrate().migrationsExecuted).isEqualTo(20);
        assertThat(columnExists(schema, "inventory_movements", "sale_id")).isFalse();

        // 2. Existing deployment: an organization plus a role granted EVERY permission (51).
        UUID organizationId = UUID.randomUUID();
        UUID roleId = UUID.randomUUID();
        execute("insert into " + schema + ".organizations (id,name,code,timezone,currency,locale,created_at,updated_at) values ('"
                + organizationId + "','Existing Org','existing-consumption','UTC','MXN','es-MX',now(),now())");
        execute("insert into " + schema + ".roles (id,organization_id,code,name,created_at,updated_at) values ('"
                + roleId + "','" + organizationId + "','existing-role','Existing role',now(),now())");
        execute("insert into " + schema + ".role_permissions (role_id,permission_code) select '" + roleId + "',code from " + schema + ".permissions");
        assertThat(countRows(schema, "role_permissions")).isEqualTo(51);

        // 3. Apply V21.
        Flyway toV21 = Flyway.configure()
                .dataSource(POSTGRES.getJdbcUrl(), POSTGRES.getUsername(), POSTGRES.getPassword())
                .schemas(schema)
                .target(MigrationVersion.fromVersion("21"))
                .load();
        assertThat(toV21.migrate().migrationsExecuted).isEqualTo(1);
        toV21.validate();

        // 4. V21 adds the sale linkage without touching permissions or existing grants.
        assertThat(countRows(schema, "permissions")).isEqualTo(51);
        assertThat(countRows(schema, "role_permissions")).isEqualTo(51);
        assertThat(columnExists(schema, "inventory_movements", "sale_id")).isTrue();

        // 5. Movement coupling: OPENING stays unlinked; SALE_CONSUMPTION must be
        //    negative, linked, and unique per sale+item.
        UUID userId = UUID.randomUUID();
        UUID branchId = UUID.randomUUID();
        UUID registerId = UUID.randomUUID();
        UUID shiftId = UUID.randomUUID();
        UUID itemId = UUID.randomUUID();
        UUID saleId = UUID.randomUUID();
        execute("insert into " + schema + ".users (id,organization_id,email,password_hash,created_at,updated_at) values ('"
                + userId + "','" + organizationId + "','consumption@example.test','unused',now(),now())");
        execute("insert into " + schema + ".branches (id,organization_id,name,code,created_at,updated_at) values ('"
                + branchId + "','" + organizationId + "','Branch','consumption-branch',now(),now())");
        execute("insert into " + schema + ".cash_registers (id,organization_id,branch_id,code,name,created_at,updated_at) values ('"
                + registerId + "','" + organizationId + "','" + branchId + "','reg','Register',now(),now())");
        execute("insert into " + schema + ".cash_shifts (id,organization_id,branch_id,cash_register_id,opened_by_user_id,status,opening_amount,opened_at,created_at,updated_at) values ('"
                + shiftId + "','" + organizationId + "','" + branchId + "','" + registerId + "','" + userId + "','OPEN',0,now(),now(),now())");
        execute("insert into " + schema + ".inventory_items (id,organization_id,code,name,base_unit,fractional_unit_allowed,created_at,updated_at) values ('"
                + itemId + "','" + organizationId + "','consumption-item','Consumption item','GRAM',false,now(),now())");
        execute("insert into " + schema + ".sales (id,organization_id,branch_id,cash_register_id,cash_shift_id,created_by_user_id,status,total_amount,created_at,updated_at) values ('"
                + saleId + "','" + organizationId + "','" + branchId + "','" + registerId + "','" + shiftId + "','" + userId + "','OPEN',1,now(),now())");

        String movement = "insert into " + schema + ".inventory_movements "
                + "(id,organization_id,branch_id,inventory_item_id,type,quantity_delta,resulting_quantity,reason,actor_user_id,sale_id,occurred_at) values ";
        execute(movement + "('" + UUID.randomUUID() + "','" + organizationId + "','" + branchId + "','" + itemId
                + "','OPENING',5,5,null,'" + userId + "',null,now())");
        assertThat(countRows(schema, "inventory_movements")).isEqualTo(1);

        assertThatThrownBy(() -> execute(movement + "('" + UUID.randomUUID() + "','" + organizationId + "','" + branchId + "','" + itemId
                + "','SALE_CONSUMPTION',-1,4,null,'" + userId + "',null,now())"))
                .isInstanceOf(IllegalStateException.class);
        execute(movement + "('" + UUID.randomUUID() + "','" + organizationId + "','" + branchId + "','" + itemId
                + "','SALE_CONSUMPTION',-1,4,null,'" + userId + "','" + saleId + "',now())");
        assertThatThrownBy(() -> execute(movement + "('" + UUID.randomUUID() + "','" + organizationId + "','" + branchId + "','" + itemId
                + "','SALE_CONSUMPTION',-1,3,null,'" + userId + "','" + saleId + "',now())"))
                .isInstanceOf(IllegalStateException.class);
        assertThatThrownBy(() -> execute(movement + "('" + UUID.randomUUID() + "','" + organizationId + "','" + branchId + "','" + itemId
                + "','ADJUSTMENT',-2,2,'correction','" + userId + "','" + saleId + "',now())"))
                .isInstanceOf(IllegalStateException.class);
    }

    private static String valueOfString(String schema, String table, String column, UUID id) {
        try (Connection connection = connect();
                PreparedStatement statement = connection.prepareStatement(
                        "select " + column + " from " + schema + "." + table + " where id = ?")) {
            statement.setObject(1, id);
            try (ResultSet resultSet = statement.executeQuery()) {
                resultSet.next();
                return resultSet.getString(1);
            }
        } catch (Exception e) {
            throw new IllegalStateException(e);
        }
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
