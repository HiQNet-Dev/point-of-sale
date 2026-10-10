package com.hiqnet.pos;

import static org.assertj.core.api.Assertions.assertThat;

import java.sql.Connection;
import java.sql.DriverManager;
import java.sql.PreparedStatement;
import java.sql.ResultSet;
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
 *   <li><b>Clean install</b>: V1 → V8 applies completely and validates, leaving
 *       the expected seed state (permission catalog = 19, bootstrap singleton = 1).</li>
 *   <li><b>Upgrade path</b>: a database migrated up to V7 (the state before the
 *       Secure Admin Bootstrap slice) applies V8 correctly in isolation — the
 *       real production upgrade, not a recreated dataset.</li>
 * </ul>
 */
@Testcontainers
class FlywayMigrationPathTest {

    @Container
    static final PostgreSQLContainer<?> POSTGRES = new PostgreSQLContainer<>("postgres:17-alpine");

    @Test
    void cleanInstallationMigratesV1ThroughV8AndValidates() {
        Flyway flyway = flyway("clean_foundation");

        MigrateResult result = flyway.migrate();

        assertThat(result.migrationsExecuted).isEqualTo(8);
        flyway.validate();
        assertThat(countRows("clean_foundation", "permissions")).isEqualTo(19);
        assertThat(countRows("clean_foundation", "installation_bootstrap")).isEqualTo(1);
    }

    @Test
    void upgradeFromV7AppliesV8WithoutRecreatingData() {
        // Stop the migration history at V7: the exact pre-bootstrap state.
        Flyway upToV7 = Flyway.configure()
                .dataSource(POSTGRES.getJdbcUrl(), POSTGRES.getUsername(), POSTGRES.getPassword())
                .schemas("upgrade_path")
                .target(MigrationVersion.fromVersion("7"))
                .load();
        MigrateResult migratedToV7 = upToV7.migrate();

        assertThat(migratedToV7.migrationsExecuted).isEqualTo(7);
        assertThat(tableExists("upgrade_path", "installation_bootstrap")).isFalse();

        // A fresh Flyway run (like a new backend version) applies only V8.
        Flyway toLatest = Flyway.configure()
                .dataSource(POSTGRES.getJdbcUrl(), POSTGRES.getUsername(), POSTGRES.getPassword())
                .schemas("upgrade_path")
                .load();
        MigrateResult migratedToLatest = toLatest.migrate();

        assertThat(migratedToLatest.migrationsExecuted).isEqualTo(1);
        toLatest.validate();
        assertThat(countRows("upgrade_path", "permissions")).isEqualTo(19);
        assertThat(countRows("upgrade_path", "installation_bootstrap")).isEqualTo(1);
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
}