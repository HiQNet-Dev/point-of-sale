import java.math.BigDecimal;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.attribute.PosixFilePermissions;
import java.sql.*;
import java.time.OffsetDateTime;
import java.util.*;
import org.flywaydb.core.Flyway;
import org.flywaydb.core.api.MigrationVersion;

/** Source-file runner: only the owned PostgreSQL source, never the application server. */
class SourceFixture {

	static final Set<String> TABLES = Set.of(
		"app_metadata",
		"organizations",
		"branches",
		"users",
		"refresh_sessions",
		"permissions",
		"roles",
		"role_permissions",
		"user_organization_roles",
		"branch_memberships",
		"branch_membership_roles",
		"audit_events",
		"installation_bootstrap",
		"categories",
		"products",
		"modifier_groups",
		"modifier_options",
		"product_modifier_groups",
		"cash_registers",
		"cash_shifts",
		"sales",
		"sale_items",
		"sale_item_modifiers",
		"payments",
		"inventory_items",
		"branch_inventory",
		"inventory_movements",
		"product_inventory_recipes",
		"product_inventory_recipe_components",
		"modifier_option_inventory_configs",
		"modifier_option_inventory_effects",
		"sale_item_inventory_components",
		"suppliers",
		"purchase_orders",
		"purchase_order_lines",
		"purchase_receipts",
		"purchase_receipt_lines"
	);

	static String env(String key) {
		String value = System.getenv(key);
		if (value == null || value.isEmpty()) throw new IllegalStateException(
			"missing lab environment"
		);
		return value;
	}

	public static void main(String[] args) {
		String stage = "self-test";
		try {
			selfTest();
			int port = Integer.parseInt(env("LAB_PORT"));
			if (port < 1 || port > 65535) throw new IllegalArgumentException(
				"port"
			);
			String base = "jdbc:postgresql://127.0.0.1:" + port + "/";
			Properties properties = new Properties();
			properties.setProperty("user", "lab_admin");
			properties.setProperty("password", env("LAB_PASSWORD"));
			properties.setProperty("connectTimeout", "5");
			properties.setProperty("socketTimeout", "120");
			properties.setProperty(
				"options",
				"-c timezone=UTC -c statement_timeout=120000"
			);
			stage = "create-source";
			try (
				Connection admin = DriverManager.getConnection(
					base + "postgres",
					properties
				);
				Statement statement = admin.createStatement()
			) {
				try (
					ResultSet rs = statement.executeQuery(
						"SHOW server_version_num"
					)
				) {
					rs.next();
					if (
						rs.getInt(1) / 10000 != 17
					) throw new IllegalStateException("requires PostgreSQL 17");
				}
				try (
					ResultSet rs = statement.executeQuery(
						"SELECT rolpassword LIKE 'SCRAM-SHA-256$%' FROM pg_authid WHERE rolname = current_user"
					)
				) {
					rs.next();
					if (!rs.getBoolean(1)) throw new IllegalStateException(
						"requires SCRAM"
					);
				}
				statement.execute(
					"CREATE DATABASE source TEMPLATE template0 ENCODING 'UTF8'"
				);
			}
			String url = base + "source";
			stage = "flyway";
			Flyway flyway = Flyway.configure()
				.dataSource(url, "lab_admin", env("LAB_PASSWORD"))
				.locations("filesystem:" + env("LAB_MIGRATIONS"))
				.target(MigrationVersion.fromVersion("25"))
				.cleanDisabled(true)
				.loggers("slf4j")
				.load();
			if (
				flyway.migrate().migrationsExecuted != 25
			) throw new IllegalStateException("migration count");
			flyway.validate();
			if (
				flyway.migrate().migrationsExecuted != 0
			) throw new IllegalStateException("non-idempotent Flyway");
			Path work = Path.of(env("LAB_WORK"));
			stage = "seed";
			try (
				Connection source = DriverManager.getConnection(url, properties)
			) {
				source.setAutoCommit(false);
				try (Statement statement = source.createStatement()) {
					statement.execute(
						Files.readString(
							Path.of(env("LAB_SEED")),
							StandardCharsets.UTF_8
						)
					);
				}
				source.commit(); // Includes deferred ACTIVE recipe component constraints.
				write(work.resolve("history.json"), history(source));
				source.rollback();
				stage = "export";
				source.setTransactionIsolation(
					Connection.TRANSACTION_REPEATABLE_READ
				);
				source.setReadOnly(true);
				Object bundle = export(source);
				Object repeated = export(source); // Same read-only transaction/snapshot, no reseed.
				if (
					!json(bundle).equals(json(repeated))
				) throw new IllegalStateException("snapshot mismatch");
				verifyFixture(bundle);
				write(work.resolve("bundle.json"), bundle);
				write(work.resolve("repeat.json"), repeated);
				source.rollback();
			}
		} catch (Exception | LinkageError | ServiceConfigurationError error) {
			// Flyway/JDBC exception text can contain SQL, URLs or fixture credentials.
			System.err.println(
				"source fixture failed at " +
					stage +
					" class=" +
					error.getClass().getSimpleName()
			);
			for (
				Throwable cause = error;
				cause != null;
				cause = cause.getCause()
			) {
				System.err.println(
					"source-cause " + cause.getClass().getSimpleName()
				);
				for (StackTraceElement frame : Arrays.stream(
					cause.getStackTrace()
				)
					.limit(4)
					.toList())
					System.err.println(
						"source-frame " +
							frame.getClassName() +
							"." +
							frame.getMethodName() +
							":" +
							frame.getLineNumber()
					);
			}
			System.exit(1);
		}
	}

	static List<Object> history(Connection source) throws SQLException {
		List<Object> entries = new ArrayList<>();
		try (
			Statement s = source.createStatement();
			ResultSet rs = s.executeQuery(
				"SELECT version, script, checksum, success FROM flyway_schema_history ORDER BY installed_rank"
			)
		) {
			int version = 1;
			while (rs.next()) {
				if (
					!rs.getBoolean("success") ||
					!Integer.toString(version++).equals(rs.getString("version"))
				) throw new IllegalStateException("invalid Flyway history");
				entries.add(
					Map.of(
						"version",
						rs.getString("version"),
						"script",
						rs.getString("script"),
						"checksum",
						rs.getInt("checksum"),
						"success",
						true
					)
				);
			}
			if (version != 26) throw new IllegalStateException(
				"incomplete history"
			);
		}
		return entries;
	}

	static String quoted(String identifier) {
		return "\"" + identifier.replace("\"", "\"\"") + "\"";
	}

	static Object export(Connection source) throws SQLException {
		List<String> names = new ArrayList<>();
		try (
			Statement s = source.createStatement();
			ResultSet rs = s.executeQuery(
				"SELECT table_name FROM information_schema.tables WHERE table_schema = 'public' " +
					"AND table_type = 'BASE TABLE' AND table_name <> 'flyway_schema_history' ORDER BY table_name COLLATE \"C\""
			)
		) {
			while (rs.next()) names.add(rs.getString(1));
		}
		if (
			names.size() != 37 || !new HashSet<>(names).equals(TABLES)
		) throw new IllegalStateException("unexpected source tables");
		List<Object> tables = new ArrayList<>();
		for (String name : names) {
			SortedMap<Short, String> keyOrder = new TreeMap<>();
			try (
				ResultSet rs = source
					.getMetaData()
					.getPrimaryKeys(null, "public", name)
			) {
				while (rs.next())
					keyOrder.put(
						rs.getShort("KEY_SEQ"),
						rs.getString("COLUMN_NAME")
					);
			}
			List<String> keys = new ArrayList<>(keyOrder.values());
			if (keys.isEmpty()) throw new IllegalStateException(
				"missing primary key"
			);
			String order = String.join(
				",",
				keys.stream().map(SourceFixture::quoted).toList()
			);
			List<Object> columns = new ArrayList<>();
			List<Object> rows = new ArrayList<>();
			long count = 0;
			try (
				Statement s = source.createStatement();
				ResultSet rs = s.executeQuery(
					"SELECT * FROM public." +
						quoted(name) +
						" ORDER BY " +
						order
				)
			) {
				ResultSetMetaData meta = rs.getMetaData();
				for (int col = 1; col <= meta.getColumnCount(); col++) {
					columns.add(
						Map.of(
							"name",
							meta.getColumnName(col),
							"type",
							meta.getColumnTypeName(col),
							"nullable",
							meta.isNullable(col) !=
								ResultSetMetaData.columnNoNulls,
							"precision",
							meta.getPrecision(col),
							"scale",
							meta.getScale(col)
						)
					);
				}
				while (rs.next()) {
					count++;
					// Session identities/hashes must not be migrated. Count without serializing them.
					if (name.equals("refresh_sessions")) continue;
					List<Object> row = new ArrayList<>();
					for (
						int col = 1;
						col <= meta.getColumnCount();
						col++
					) row.add(cell(rs, meta, col));
					rows.add(row);
				}
			}
			tables.add(
				Map.of(
					"name",
					name,
					"columns",
					columns,
					"primary_key",
					keys,
					"rows",
					rows,
					"source_count",
					count,
					"action",
					name.equals("refresh_sessions")
						? "INVALIDATE"
						: name.equals("app_metadata")
							? "REFERENCE_ONLY"
							: "EXPORT_ONLY"
				)
			);
		}
		return Map.of(
			"version",
			1,
			"kind",
			"synthetic-flyway-source",
			"tables",
			tables
		);
	}

	static Object cell(ResultSet rs, ResultSetMetaData meta, int col)
		throws SQLException {
		if (rs.getObject(col) == null) return null;
		return switch (meta.getColumnType(col)) {
			case Types.NUMERIC, Types.DECIMAL -> rs.getBigDecimal(
				col
			).toPlainString();
			case Types.SMALLINT, Types.INTEGER, Types.BIGINT -> rs.getLong(col);
			case Types.BOOLEAN, Types.BIT -> rs.getBoolean(col);
			case Types.TIMESTAMP, Types.TIMESTAMP_WITH_TIMEZONE -> {
				if (
					!meta.getColumnTypeName(col).equals("timestamptz")
				) throw new IllegalStateException("unsupported timestamp type");
				yield rs
					.getObject(col, OffsetDateTime.class)
					.toInstant()
					.toString();
			}
			case Types.VARCHAR, Types.CHAR, Types.LONGVARCHAR -> rs.getString(
				col
			);
			case Types.OTHER -> {
				String type = meta.getColumnTypeName(col);
				if (
					!type.equals("uuid") && !type.equals("jsonb")
				) throw new IllegalStateException("unsupported source type");
				yield rs.getString(col);
			}
			default -> throw new IllegalStateException(
				"unsupported source type"
			);
		};
	}

	static void write(Path path, Object value) throws Exception {
		Files.createFile(
			path,
			PosixFilePermissions.asFileAttribute(
				PosixFilePermissions.fromString("rw-------")
			)
		);
		Files.writeString(path, json(value) + "\n", StandardCharsets.UTF_8);
	}

	static String json(Object value) {
		if (value == null) return "null";
		if (value instanceof String text) {
			StringBuilder out = new StringBuilder("\"");
			for (char c : text.toCharArray()) {
				switch (c) {
					case '"' -> out.append("\\\"");
					case '\\' -> out.append("\\\\");
					default -> {
						if (c < 32) out.append(
							String.format(Locale.ROOT, "\\u%04x", (int) c)
						);
						else out.append(c);
					}
				}
			}
			return out.append('"').toString();
		}
		if (
			value instanceof Boolean ||
			value instanceof Long ||
			value instanceof Integer ||
			value instanceof Short
		) return value.toString();
		if (value instanceof Map<?, ?> map) {
			SortedMap<String, Object> sorted = new TreeMap<>();
			map.forEach((key, item) -> sorted.put((String) key, item));
			return (
				"{" +
				String.join(
					",",
					sorted
						.entrySet()
						.stream()
						.map(
							entry ->
								json(entry.getKey()) +
								":" +
								json(entry.getValue())
						)
						.toList()
				) +
				"}"
			);
		}
		if (value instanceof List<?> list) return (
			"[" +
			String.join(",", list.stream().map(SourceFixture::json).toList()) +
			"]"
		);
		throw new IllegalArgumentException(
			"unsupported JSON value; floating point forbidden"
		);
	}

	static void selfTest() {
		if (
			!json(new BigDecimal("900719925474.1234").toPlainString()).equals(
				"\"900719925474.1234\""
			)
		) throw new IllegalStateException("decimal serialization");
		if (
			!json(
				Arrays.asList(
					null,
					"0.000000",
					"00000000-0000-0000-0000-000000000001"
				)
			).equals(
				"[null,\"0.000000\",\"00000000-0000-0000-0000-000000000001\"]"
			)
		) throw new IllegalStateException("NULL/UUID serialization");
	}

	static Object value(Object bundle, String table, int row, String column) {
		List<Map<String, Object>> tables = (List<Map<String, Object>>) (
			(Map<?, ?>) bundle
		).get("tables");
		Map<String, Object> data = tables
			.stream()
			.filter(t -> t.get("name").equals(table))
			.findFirst()
			.orElseThrow();
		List<Map<String, Object>> columns = (List<
			Map<String, Object>
		>) data.get("columns");
		int col = 0;
		while (!columns.get(col).get("name").equals(column)) col++;
		return ((List<List<Object>>) data.get("rows")).get(row).get(col);
	}

	static void verifyFixture(Object bundle) {
		if (
			!Objects.equals(
				value(bundle, "products", 0, "unit_price"),
				"900719925474.1234"
			) ||
			!Objects.equals(
				value(bundle, "products", 0, "id"),
				"00000000-0000-0000-0000-000000000010"
			) ||
			value(bundle, "purchase_order_lines", 0, "unit_cost") != null ||
			!Objects.equals(
				value(bundle, "purchase_orders", 0, "status"),
				"ORDERED"
			) ||
			!Objects.equals(
				value(bundle, "purchase_order_lines", 0, "quantity"),
				"1.000001"
			) ||
			!Objects.equals(
				value(bundle, "organizations", 0, "created_at"),
				"2025-01-01T00:00:00Z"
			) ||
			value(bundle, "branches", 0, "timezone") != null
		) throw new IllegalStateException("exact source fixture regression");
	}
}
