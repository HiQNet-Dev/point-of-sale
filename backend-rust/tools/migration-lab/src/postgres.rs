//! PostgreSQL 17 nativo desechable. Los PASSED son técnicos, nunca gates normativos.

use std::{
    fs::{self, File},
    os::unix::fs::{DirBuilderExt, PermissionsExt},
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant},
};

use serde_json::{Value, json};
use sqlx::{
    AssertSqlSafe, ConnectOptions, PgPool, Row,
    migrate::{MigrateError, Migrator},
    postgres::{PgConnectOptions, PgPoolOptions, PgSslMode},
};
use tempfile::TempDir;

use crate::{LabError, Result, provenance::sha256, report::Evidence};

const ADMIN: &str = "lab_admin";
const PORT: u16 = 5432; // Solo namespace UNIX privado; no se reserva ni abre un puerto TCP.
const BASE: &str = "0001_lab.sql";
const UPGRADE: &str = "0002_lab_upgrade.sql";
const FAILURE: &str = "0003_lab_failure.sql";

fn invalid(message: &str) -> LabError {
    LabError::Invalid(message.into())
}

fn require(condition: bool, message: &str) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(invalid(message))
    }
}

fn version_17(text: &str) -> bool {
    text.split_once("(PostgreSQL) ")
        .and_then(|(_, version)| version.split_whitespace().next())
        .is_some_and(|version| {
            version.starts_with("17.") && version[3..].starts_with(|c: char| c.is_ascii_digit())
        })
}

fn validate_bin(bin: &Path) -> Result<PathBuf> {
    let bin = bin
        .canonicalize()
        .map_err(|_| invalid("bin debe contener binarios nativos PostgreSQL 17"))?;
    for name in ["initdb", "postgres", "pg_ctl", "pg_dump", "pg_restore"] {
        let metadata = fs::metadata(bin.join(name)).map_err(|_| {
            invalid("faltan binarios PostgreSQL 17: initdb/postgres/pg_ctl/pg_dump/pg_restore")
        })?;
        require(
            metadata.is_file() && metadata.permissions().mode() & 0o111 != 0,
            "los binarios PostgreSQL deben ser archivos ejecutables",
        )?;
    }
    Ok(bin)
}

// Solo se publican categorías y SQLSTATE, nunca mensajes del driver/DSN/entorno.
fn safe_error(error: &LabError) -> &'static str {
    match error {
        LabError::Sqlx(_) => "sqlx_error",
        LabError::Migrate(_) => "sqlx_migration_error",
        LabError::Io(_) => "local_io_error",
        LabError::Json(_) => "json_error",
        _ => "lab_precondition_or_assertion",
    }
}

fn error_detail(error: &LabError) -> Value {
    let sqlstate = match error {
        LabError::Sqlx(sqlx::Error::Database(db)) => db.code().map(|code| code.into_owned()),
        LabError::Migrate(MigrateError::ExecuteMigration(sqlx::Error::Database(db), _)) => {
            db.code().map(|code| code.into_owned())
        }
        _ => None,
    };
    let driver_kind = match error {
        LabError::Sqlx(sqlx::Error::PoolTimedOut) => Some("pool_timeout"),
        LabError::Sqlx(sqlx::Error::PoolClosed) => Some("pool_closed"),
        LabError::Sqlx(sqlx::Error::ColumnDecode { .. }) => Some("column_decode"),
        LabError::Sqlx(sqlx::Error::ColumnNotFound(_)) => Some("column_not_found"),
        LabError::Sqlx(sqlx::Error::Io(_)) => Some("io"),
        LabError::Sqlx(sqlx::Error::Protocol(_)) => Some("protocol"),
        _ => None,
    };
    json!({"error_kind": safe_error(error), "sqlstate": sqlstate, "driver_kind": driver_kind})
}

fn observe(checks: &mut Vec<Value>, id: &str, result: Result<Value>) -> Result<Value> {
    match result {
        Ok(detail) => {
            checks.push(json!({"id": id, "status": "PASSED", "coverage": "synthetic_technical", "detail": detail}));
            Ok(detail)
        }
        Err(error) => {
            checks.push(json!({"id": id, "status": "FAILED", "coverage": "synthetic_technical", "detail": error_detail(&error)}));
            Err(error)
        }
    }
}

/// El hijo se mata y recolecta incluso si falla el spawn/wait siguiente o hay unwind.
struct OwnedChild(Child);

impl Drop for OwnedChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn wait_bounded(child: &mut Child, duration: Duration) -> Result<ExitStatus> {
    let deadline = Instant::now() + duration;
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(status);
        }
        if Instant::now() >= deadline {
            return Err(invalid("proceso PostgreSQL excedió su límite de tiempo"));
        }
        thread::sleep(Duration::from_millis(20));
    }
}

struct Cluster {
    // Drop se ejecuta antes de borrar temp; todos los recursos pertenecen al lab.
    bin: PathBuf,
    temp: TempDir,
    socket: PathBuf,
    child: Option<OwnedChild>,
    pools: Vec<PgPool>,
    binary_versions: Value,
}

impl Cluster {
    fn command(&self, program: &str) -> Command {
        let mut command = Command::new(self.bin.join(program));
        command
            .env_clear()
            .env("LC_ALL", "C")
            .env("TZ", "UTC")
            .env("HOME", self.temp.path())
            .current_dir(self.temp.path())
            .stdin(Stdio::null());
        command
    }

    fn execute(&self, program: &str, args: &[&str], seconds: u64) -> Result<Vec<u8>> {
        let stdout = self.temp.path().join("command.stdout");
        let stderr = self.temp.path().join("command.stderr");
        let mut child = OwnedChild(
            self.command(program)
                .args(args)
                .stdout(File::create(&stdout)?)
                .stderr(File::create(stderr)?)
                .spawn()?,
        );
        let status = wait_bounded(&mut child.0, Duration::from_secs(seconds))?;
        require(
            status.success(),
            "falló un proceso PostgreSQL; consultar postgres-process.log",
        )?;
        Ok(fs::read(stdout)?)
    }

    fn start(bin: &Path) -> Result<Self> {
        // SQLx 0.9 hereda PGOPTIONS incluso con new_without_pgpass; no dejar que
        // configuración ajena cambie search_path, role ni opciones de seguridad.
        require(
            std::env::var_os("PGOPTIONS").is_none(),
            "PGOPTIONS no está permitido en este laboratorio",
        )?;
        let bin = validate_bin(bin)?;
        // /tmp corto evita el límite sockaddr_un, independientemente de TMPDIR.
        let temp = tempfile::Builder::new()
            .prefix("hiqnet-pg-")
            .tempdir_in("/tmp")?;
        fs::set_permissions(temp.path(), fs::Permissions::from_mode(0o700))?;
        let socket = temp.path().join("s");
        fs::DirBuilder::new().mode(0o700).create(&socket)?;
        let mut cluster = Self {
            bin,
            temp,
            socket,
            child: None,
            pools: Vec::new(),
            binary_versions: json!({}),
        };
        for program in ["initdb", "postgres", "pg_ctl", "pg_dump", "pg_restore"] {
            let version = String::from_utf8_lossy(&cluster.execute(program, &["--version"], 5)?)
                .trim()
                .to_owned();
            require(
                version_17(&version),
                "se exige PostgreSQL 17 para todos los binarios",
            )?;
            cluster.binary_versions[program] = json!(version);
        }
        cluster.execute(
            "initdb",
            &[
                "-D",
                "data",
                "--username=lab_admin",
                "--no-locale",
                "--encoding=UTF8",
                "--auth-local=trust",
                "--auth-host=reject",
                "--no-instructions",
            ],
            45,
        )?;
        // No entradas TCP en pg_hba, tampoco replication. Trust queda confinado a
        // un directorio 0700 recién creado y solo a este usuario del sistema.
        fs::write(
            cluster.temp.path().join("data/pg_hba.conf"),
            "local all all trust\n",
        )?;
        let log = File::create(cluster.temp.path().join("postgres.log"))?;
        let child = cluster
            .command("postgres")
            .args(["-D", "data", "-k"])
            .arg(&cluster.socket)
            .args([
                "-p",
                "5432",
                "-c",
                "listen_addresses=",
                "-c",
                "unix_socket_permissions=0700",
                "-c",
                "unix_socket_group=",
                "-c",
                "max_connections=20",
                "-c",
                "logging_collector=off",
                "-c",
                "log_statement=none",
                "-c",
                "log_min_error_statement=panic",
                "-c",
                "log_connections=off",
                "-c",
                "log_disconnections=off",
                "-c",
                "statement_timeout=10000",
                "-c",
                "lock_timeout=3000",
                "-c",
                "idle_in_transaction_session_timeout=10000",
            ])
            .stdout(log.try_clone()?)
            .stderr(log)
            .spawn()?;
        cluster.child = Some(OwnedChild(child));
        Ok(cluster)
    }

    fn options(&self, database: &str, user: &str, schema: &str) -> PgConnectOptions {
        PgConnectOptions::new_without_pgpass()
            .host("/unused-private-socket-only")
            .socket(&self.socket)
            .port(PORT)
            .username(user)
            .password("")
            .database(database)
            .ssl_mode(PgSslMode::Disable)
            .application_name("hiqnet-synthetic-postgres-lab")
            .options([
                ("search_path", schema),
                ("timezone", "UTC"),
                ("DateStyle", "ISO, YMD"),
                ("statement_timeout", "10000"),
                ("lock_timeout", "3000"),
            ])
            .disable_statement_logging()
    }

    async fn pool(&mut self, database: &str, user: &str, schema: &str) -> Result<PgPool> {
        // Registrar antes del primer await: Drop también cubre conexión fallida/cancelada.
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .acquire_timeout(Duration::from_secs(3))
            .connect_lazy_with(self.options(database, user, schema));
        self.pools.push(pool.clone());
        sqlx::query("SELECT 1").execute(&pool).await?;
        Ok(pool)
    }

    async fn ready(&mut self) -> Result<PgPool> {
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            require(
                self.child
                    .as_mut()
                    .is_some_and(|child| matches!(child.0.try_wait(), Ok(None))),
                "postgres terminó antes de estar listo",
            )?;
            match self.pool("postgres", ADMIN, "public").await {
                Ok(pool) => return Ok(pool),
                Err(_) if Instant::now() < deadline => {
                    if let Some(pool) = self.pools.pop() {
                        pool.close().await;
                    }
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                Err(error) => return Err(error),
            }
        }
    }

    async fn close_pools(&mut self) {
        for pool in &self.pools {
            pool.close().await;
        }
        self.pools.clear();
    }

    fn stop(&mut self) -> Result<()> {
        if self.child.is_none() {
            return Ok(());
        }
        let stop = self.execute(
            "pg_ctl",
            &["-D", "data", "stop", "-m", "immediate", "-w", "-t", "5"],
            8,
        );
        // Incluso si pg_ctl falla, OwnedChild mata y recolecta al postmaster.
        if let Some(mut child) = self.child.take() {
            if stop.is_ok() {
                wait_bounded(&mut child.0, Duration::from_secs(3))?;
            }
            drop(child);
        }
        stop.map(|_| ())
    }
}

impl Drop for Cluster {
    fn drop(&mut self) {
        // Drop no puede await ni crear un runtime anidado en el hilo Tokio. Un
        // hilo acotado drena los pools en cancelación/unwind antes de parar PG.
        if !self.pools.is_empty() {
            let pools = std::mem::take(&mut self.pools);
            let _ = thread::spawn(move || {
                if let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    runtime.block_on(async {
                        for pool in pools {
                            let _ =
                                tokio::time::timeout(Duration::from_secs(3), pool.close()).await;
                        }
                    });
                }
            })
            .join();
        }
        let _ = self.stop();
    }
}

fn fixtures() -> PathBuf {
    // Resuelto relativo a este archivo al compilar, sin depender de cwd de CLI.
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/postgres")
}

fn root(context: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../migrations")
        .join(context)
}

fn copy_fixtures(path: &Path, upgrade: bool, failure: bool) -> Result<()> {
    fs::copy(fixtures().join("migrations").join(BASE), path.join(BASE))?;
    if upgrade {
        fs::copy(
            fixtures().join("migrations").join(UPGRADE),
            path.join(UPGRADE),
        )?;
    }
    if failure {
        fs::copy(
            fixtures().join("transaction-failure").join(FAILURE),
            path.join(FAILURE),
        )?;
    }
    Ok(())
}

async fn history(pool: &PgPool, schema: &str) -> Result<Value> {
    // schema solo procede de constantes internas, nunca argumentos del usuario.
    require(
        matches!(schema, "public" | "lab"),
        "schema de tracking inválido",
    )?;
    let rows = sqlx::query(AssertSqlSafe(format!("SELECT version, description, installed_on::text AS installed_on, success, encode(checksum, 'hex') AS checksum, execution_time FROM {schema}._sqlx_migrations ORDER BY version")))
        .fetch_all(pool).await?;
    let entries: Vec<Value> = rows.iter().map(|row| -> Result<Value> {
        Ok(json!({"version": row.try_get::<i64, _>("version")?, "description": row.try_get::<String, _>("description")?,
            "installed_on": row.try_get::<String, _>("installed_on")?, "success": row.try_get::<bool, _>("success")?,
            "checksum": row.try_get::<String, _>("checksum")?, "execution_time": row.try_get::<i64, _>("execution_time")?}))
    }).collect::<Result<_>>()?;
    Ok(json!(entries))
}

async fn tables(pool: &PgPool) -> Result<Vec<String>> {
    Ok(sqlx::query_scalar("SELECT schemaname || '.' || tablename FROM pg_tables WHERE schemaname IN ('public','lab') ORDER BY 1")
        .fetch_all(pool).await?)
}

async fn schema(pool: &PgPool) -> Result<Value> {
    // Catálogo SQL estable, no dump binario ni bytes pg_dump aleatorios. Incluye
    // tipos/defaults/ownership, constraints, índices, RLS, policies y privilegios.
    let sql = "SELECT COALESCE(jsonb_agg(to_jsonb(t) ORDER BY t.schema_name, t.table_name), '[]'::jsonb)::text FROM (
        SELECT n.nspname AS schema_name, c.relname AS table_name, pg_get_userbyid(c.relowner) AS owner,
          c.relrowsecurity AS rls, c.relforcerowsecurity AS force_rls, c.relacl::text AS acl,
          (SELECT jsonb_agg(jsonb_build_array(a.attname, format_type(a.atttypid,a.atttypmod),a.attnotnull,pg_get_expr(d.adbin,d.adrelid)) ORDER BY a.attnum)
           FROM pg_attribute a LEFT JOIN pg_attrdef d ON d.adrelid=a.attrelid AND d.adnum=a.attnum
           WHERE a.attrelid=c.oid AND a.attnum>0 AND NOT a.attisdropped) AS columns,
          (SELECT jsonb_agg(jsonb_build_array(k.conname,pg_get_constraintdef(k.oid)) ORDER BY k.conname) FROM pg_constraint k WHERE k.conrelid=c.oid) AS constraints,
          (SELECT jsonb_agg(pg_get_indexdef(i.indexrelid) ORDER BY ic.relname) FROM pg_index i JOIN pg_class ic ON ic.oid=i.indexrelid WHERE i.indrelid=c.oid) AS indexes,
          (SELECT jsonb_agg(jsonb_build_array(p.polname,p.polcmd,pg_get_expr(p.polqual,p.polrelid),pg_get_expr(p.polwithcheck,p.polrelid)) ORDER BY p.polname) FROM pg_policy p WHERE p.polrelid=c.oid) AS policies
        FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace
        WHERE n.nspname IN ('public','lab') AND c.relkind='r'
    ) t";
    let text: String = sqlx::query_scalar(sql).fetch_one(pool).await?;
    Ok(serde_json::from_str(&text)?)
}

fn quoted(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

async fn data(pool: &PgPool) -> Result<Value> {
    let names = sqlx::query("SELECT schemaname, tablename FROM pg_tables WHERE schemaname IN ('public','lab') ORDER BY schemaname, tablename")
        .fetch_all(pool).await?;
    let mut result = serde_json::Map::new();
    for row in names {
        let schema: String = row.try_get("schemaname")?;
        let table: String = row.try_get("tablename")?;
        let sql = format!(
            "SELECT COALESCE(jsonb_agg(to_jsonb(t) ORDER BY to_jsonb(t)::text COLLATE \"C\"), '[]'::jsonb)::text FROM {}.{} t",
            quoted(&schema),
            quoted(&table)
        );
        // Ambos identificadores vienen del catálogo privado y se citan con escape.
        let text: String = sqlx::query_scalar(AssertSqlSafe(sql))
            .fetch_one(pool)
            .await?;
        // Conservar el texto canónico SQL: parsear NUMERIC a Value/f64 antes
        // del digest podría ocultar diferencias de precisión o escala.
        result.insert(format!("{schema}.{table}"), Value::String(text));
    }
    Ok(Value::Object(result))
}

async fn snapshot(pool: &PgPool) -> Result<Value> {
    let schema = schema(pool).await?;
    let data = data(pool).await?;
    Ok(
        json!({"schema_sha256": sha256(serde_json::to_vec(&schema)?), "data_sha256": sha256(serde_json::to_vec(&data)?),
        "schema": schema, "data": data}),
    )
}

async fn original_data(pool: &PgPool) -> Result<String> {
    Ok(sqlx::query_scalar("SELECT jsonb_agg(jsonb_build_array(id::text,tenant,amount::text,quantity::text,payload) ORDER BY id)::text FROM lab.lab_record")
        .fetch_one(pool).await?)
}

async fn setup(cluster: &mut Cluster, admin: &PgPool) -> Result<Value> {
    for context in ["central", "control"] {
        // Identificadores enteramente estáticos; no URLs, passwords ni input SQL.
        sqlx::raw_sql(AssertSqlSafe(format!("CREATE ROLE {context}_migrator LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS;
            CREATE ROLE {context}_app LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS NOINHERIT;")))
            .execute(admin).await?;
        for suffix in ["", "_restore"] {
            let database = format!("hiqnet_lab_{context}{suffix}");
            sqlx::query(AssertSqlSafe(format!("CREATE DATABASE {database} OWNER {context}_migrator TEMPLATE template0 ENCODING 'UTF8'")))
                .execute(admin).await?;
            sqlx::query(AssertSqlSafe(format!(
                "REVOKE ALL ON DATABASE {database} FROM PUBLIC"
            )))
            .execute(admin)
            .await?;
            sqlx::query(AssertSqlSafe(format!(
                "GRANT CONNECT ON DATABASE {database} TO {context}_migrator, {context}_app"
            )))
            .execute(admin)
            .await?;
            let pool = cluster.pool(&database, ADMIN, "public").await?;
            sqlx::query("REVOKE CREATE ON SCHEMA public FROM PUBLIC")
                .execute(&pool)
                .await?;
            pool.close().await;
        }
    }
    sqlx::query(
        "CREATE DATABASE hiqnet_lab_source OWNER lab_admin TEMPLATE template0 ENCODING 'UTF8'",
    )
    .execute(admin)
    .await?;
    for database in ["postgres", "template1", "hiqnet_lab_source"] {
        sqlx::query(AssertSqlSafe(format!(
            "REVOKE ALL ON DATABASE {database} FROM PUBLIC"
        )))
        .execute(admin)
        .await?;
    }
    let source = cluster.pool("hiqnet_lab_source", ADMIN, "public").await?;
    require(
        tables(&source).await?.is_empty(),
        "source debe permanecer vacía; Flyway lo integra otro módulo",
    )?;
    source.close().await;
    Ok(
        json!({"databases": ["hiqnet_lab_central", "hiqnet_lab_control", "hiqnet_lab_source"],
        "source_empty": true, "source_flyway_executed": false, "external_connections": false}),
    )
}

async fn isolation(cluster: &Cluster, admin: &PgPool) -> Result<Value> {
    let version: String = sqlx::query_scalar("SHOW server_version")
        .fetch_one(admin)
        .await?;
    let number: i32 = sqlx::query_scalar("SELECT current_setting('server_version_num')::integer")
        .fetch_one(admin)
        .await?;
    let listen: String = sqlx::query_scalar("SHOW listen_addresses")
        .fetch_one(admin)
        .await?;
    let socket: String = sqlx::query_scalar("SHOW unix_socket_directories")
        .fetch_one(admin)
        .await?;
    let tcp: bool = sqlx::query_scalar("SELECT inet_server_addr() IS NOT NULL")
        .fetch_one(admin)
        .await?;
    let auth: Vec<String> = sqlx::query_scalar(
        "SELECT type || ':' || auth_method FROM pg_hba_file_rules ORDER BY rule_number",
    )
    .fetch_all(admin)
    .await?;
    require(
        (170000..180000).contains(&number) && listen.is_empty() && !tcp,
        "versión o transporte PostgreSQL no permitidos",
    )?;
    require(
        socket == cluster.socket.to_string_lossy() && auth == ["local:trust"],
        "socket/HBA fuera del namespace propio",
    )?;
    require(
        fs::metadata(&cluster.socket)?.permissions().mode() & 0o777 == 0o700
            && fs::metadata(cluster.temp.path())?.permissions().mode() & 0o777 == 0o700,
        "namespace no privado",
    )?;
    Ok(
        json!({"server_version": version, "server_version_num": number, "binary_versions": cluster.binary_versions,
        "listen_addresses": listen, "transport": "private_unix_socket", "namespace_mode": "0700", "hba": auth}),
    )
}

async fn clean_root(pool: &PgPool, context: &str, evidence: &Evidence) -> Result<Value> {
    require(tables(pool).await?.is_empty(), "root no parte de BD limpia")?;
    let migrator = Migrator::new(root(context)).await?;
    migrator.run(pool).await?;
    let expected = if context == "central" {
        "public.operational_tenant"
    } else {
        "public.customer"
    };
    require(
        tables(pool).await? == ["public._sqlx_migrations", expected],
        "tablas root inesperadas o contaminación cross-root",
    )?;
    let before = snapshot(pool).await?;
    let tracking = history(pool, "public").await?;
    require(
        tracking.as_array().is_some_and(|rows| {
            rows.len() == 1 && rows[0]["version"] == 1 && rows[0]["success"] == true
        }),
        "tracking root inválido",
    )?;
    migrator.run(pool).await?;
    require(
        before == snapshot(pool).await?,
        "rerun root modificó esquema, datos o tracking",
    )?;
    evidence.json(
        &format!("schema-snapshots/postgres-{context}-clean.json"),
        &before,
    )?;
    Ok(
        json!({"root": context, "scope": "partial_existing_root", "tables": tables(pool).await?, "history": tracking,
        "snapshot": before, "normative_status": "BLOCKED", "full_schema": false}),
    )
}

async fn seed_v1(pool: &PgPool) -> Result<()> {
    let first = tempfile::tempdir()?;
    copy_fixtures(first.path(), false, false)?;
    Migrator::new(first.path()).await?.run(pool).await?;
    sqlx::query(
        "INSERT INTO lab.lab_record(id,tenant,amount,quantity,payload) VALUES
        ('00000000-0000-0000-0000-000000000001','alpha',19.9950,1.000001,'synthetic-alpha'),
        ('00000000-0000-0000-0000-000000000002','beta',0.0001,-2.123456,'synthetic-beta')",
    )
    .execute(pool)
    .await?;
    Ok(())
}

async fn upgrade(pool: &PgPool, context: &str, evidence: &Evidence) -> Result<Value> {
    let before_data = original_data(pool).await?;
    let before = snapshot(pool).await?;
    Migrator::new(fixtures().join("migrations"))
        .await?
        .run(pool)
        .await?;
    let after_data = original_data(pool).await?;
    require(
        before_data == after_data,
        "upgrade modificó datos/UUID/decimales N-1",
    )?;
    let notes: Vec<String> = sqlx::query_scalar("SELECT note FROM lab.lab_record ORDER BY id")
        .fetch_all(pool)
        .await?;
    let index: bool =
        sqlx::query_scalar("SELECT to_regclass('lab.lab_record_tenant_idx') IS NOT NULL")
            .fetch_one(pool)
            .await?;
    require(
        notes == ["upgraded", "upgraded"] && index,
        "upgrade N no utilizable",
    )?;
    let after = snapshot(pool).await?;
    evidence.json(
        &format!("schema-snapshots/postgres-{context}-v1.json"),
        &before,
    )?;
    evidence.json(
        &format!("schema-snapshots/postgres-{context}-v2.json"),
        &after,
    )?;
    Ok(
        json!({"before_data_sha256": sha256(before_data), "after_data_sha256": sha256(after_data),
        "before": before, "after": after, "new_column_verified": true, "new_index_verified": true}),
    )
}

async fn migrate_session(
    pool: &PgPool,
    migrator: &Migrator,
) -> std::result::Result<(), MigrateError> {
    // SQLx 0.9 sale de VersionMismatch/ExecuteMigration antes de unlock().
    // Nunca devolver esa sesión con advisory locks al pool (ni en cancelación).
    let mut connection = pool.acquire().await?;
    connection.close_on_drop();
    let outcome = migrator.run(&mut *connection).await;
    let closed = connection.close().await;
    outcome?;
    closed?;
    Ok(())
}

fn is_checksum_mismatch(error: &MigrateError) -> bool {
    matches!(error, MigrateError::VersionMismatch(1))
}

fn is_ddl_failure(error: &MigrateError) -> bool {
    matches!(error, MigrateError::ExecuteMigration(sqlx::Error::Database(db), 3) if db.code().as_deref() == Some("42P01"))
}

async fn checksum(pool: &PgPool, context: &str, evidence: &Evidence) -> Result<Value> {
    let changed = tempfile::tempdir()?;
    copy_fixtures(changed.path(), true, false)?;
    let mut sql = fs::read(changed.path().join(BASE))?;
    sql.extend_from_slice(b"\n-- deliberately changed technical checksum\n");
    fs::write(changed.path().join(BASE), sql)?;
    let before = snapshot(pool).await?;
    let outcome = migrate_session(pool, &Migrator::new(changed.path()).await?).await;
    require(
        matches!(&outcome, Err(error) if is_checksum_mismatch(error)),
        "se esperaba exactamente VersionMismatch(1)",
    )?;
    require(
        before == snapshot(pool).await?,
        "checksum mismatch cambió esquema/datos/historia",
    )?;
    let detail = json!({"expected_error": "VersionMismatch", "version": 1, "exact_variant_verified": true, "snapshot_unchanged": true});
    evidence.json(
        &format!("failure-logs/postgres-{context}-expected-checksum.json"),
        &detail,
    )?;
    Ok(detail)
}

async fn rerun(pool: &PgPool) -> Result<Value> {
    let before = snapshot(pool).await?;
    let before_history = history(pool, "lab").await?;
    let migrator = Migrator::new(fixtures().join("migrations")).await?;
    migrator.run(pool).await?;
    migrator.run(pool).await?;
    let after_history = history(pool, "lab").await?;
    require(
        before_history == after_history && before == snapshot(pool).await?,
        "rerun mutó esquema/datos/historial exacto",
    )?;
    require(
        after_history.as_array().is_some_and(|entries| {
            entries.len() == 2 && entries.iter().all(|entry| entry["success"] == true)
        }),
        "historial fixture inesperado",
    )?;
    Ok(
        json!({"reruns": 2, "before_history": before_history, "after_history": after_history,
        "before_history_sha256": sha256(serde_json::to_vec(&before_history)?),
        "after_history_sha256": sha256(serde_json::to_vec(&after_history)?), "snapshot_unchanged": true}),
    )
}

async fn ddl_failure(pool: &PgPool, context: &str, evidence: &Evidence) -> Result<Value> {
    let failing = tempfile::tempdir()?;
    copy_fixtures(failing.path(), true, true)?;
    let before = snapshot(pool).await?;
    let before_history = history(pool, "lab").await?;
    let outcome = migrate_session(pool, &Migrator::new(failing.path()).await?).await;
    require(
        matches!(&outcome, Err(error) if is_ddl_failure(error)),
        "se esperaba ExecuteMigration(42P01,3)",
    )?;
    let partial: bool = sqlx::query_scalar("SELECT to_regclass('lab.lab_partial') IS NOT NULL")
        .fetch_one(pool)
        .await?;
    let successes: i64 =
        sqlx::query_scalar("SELECT count(*) FROM lab._sqlx_migrations WHERE version=3 AND success")
            .fetch_one(pool)
            .await?;
    require(
        !partial
            && successes == 0
            && before == snapshot(pool).await?
            && before_history == history(pool, "lab").await?,
        "rollback dejó DDL/datos/tracking parcial",
    )?;
    let detail = json!({"expected_sqlstate": "42P01", "migration_version": 3, "partial_table_absent": true,
        "snapshot_unchanged": true, "history_unchanged": true, "success_rows_version_3": successes});
    evidence.json(
        &format!("failure-logs/postgres-{context}-expected-ddl-failure.json"),
        &detail,
    )?;
    Ok(detail)
}

async fn role_flags(pool: &PgPool, expected: &str) -> Result<()> {
    let row = sqlx::query("SELECT current_user::text AS name, rolsuper, rolbypassrls, rolcreatedb, rolcreaterole, rolreplication FROM pg_roles WHERE rolname=current_user")
        .fetch_one(pool).await?;
    require(
        row.try_get::<String, _>("name")? == expected,
        "rol de sesión inesperado",
    )?;
    for flag in [
        "rolsuper",
        "rolbypassrls",
        "rolcreatedb",
        "rolcreaterole",
        "rolreplication",
    ] {
        require(
            !row.try_get::<bool, _>(flag)?,
            "rol de aplicación/migración con privilegios elevados",
        )?;
    }
    Ok(())
}

async fn denied(pool: &PgPool, statement: &'static str) -> Result<()> {
    match sqlx::query(statement).execute(pool).await {
        Err(sqlx::Error::Database(error)) if error.code().as_deref() == Some("42501") => Ok(()),
        _ => Err(invalid("se esperaba rechazo SQLSTATE 42501")),
    }
}

async fn privileges(cluster: &mut Cluster, context: &str, migrator: &PgPool) -> Result<Value> {
    role_flags(migrator, &format!("{context}_migrator")).await?;
    require(
        matches!(context, "central" | "control"),
        "contexto de roles inválido",
    )?;
    // Allowlist cerrada: no se interpola ningún identificador del usuario.
    sqlx::raw_sql(AssertSqlSafe(format!(
        "GRANT USAGE ON SCHEMA lab TO {context}_app;
        GRANT SELECT, INSERT, UPDATE, DELETE ON lab.lab_record TO {context}_app;"
    )))
    .execute(migrator)
    .await?;
    let app = cluster
        .pool(
            &format!("hiqnet_lab_{context}"),
            &format!("{context}_app"),
            "lab",
        )
        .await?;
    role_flags(&app, &format!("{context}_app")).await?;
    // Una sola conexión fija asegura que el GUC del tenant no depende del pool.
    let mut connection = app.acquire().await?;
    sqlx::query("SET lab.tenant='alpha'")
        .execute(&mut *connection)
        .await?;
    let visible: Vec<String> = sqlx::query_scalar("SELECT tenant FROM lab.lab_record ORDER BY id")
        .fetch_all(&mut *connection)
        .await?;
    require(visible == ["alpha"], "RLS expuso filas ajenas")?;
    let changed = sqlx::query("UPDATE lab.lab_record SET payload='forbidden' WHERE tenant='beta'")
        .execute(&mut *connection)
        .await?;
    require(
        changed.rows_affected() == 0,
        "RLS permitió UPDATE de otro tenant",
    )?;
    let rejected = sqlx::query("INSERT INTO lab.lab_record(id,tenant,amount,quantity,payload) VALUES ('00000000-0000-0000-0000-000000000003','beta',0,0,'forbidden')")
        .execute(&mut *connection).await;
    require(
        matches!(rejected, Err(sqlx::Error::Database(ref error)) if error.code().as_deref() == Some("42501")),
        "RLS permitió INSERT de otro tenant",
    )?;
    // Un write propio sí debe funcionar: diferencia RLS de ausencia total de permisos.
    let own = sqlx::query("UPDATE lab.lab_record SET payload=payload WHERE tenant='alpha'")
        .execute(&mut *connection)
        .await?;
    require(
        own.rows_affected() == 1,
        "app no puede usar schema N con privilegios legítimos",
    )?;
    drop(connection);
    for statement in [
        "CREATE TABLE lab.app_forbidden(id int)",
        "CREATE TABLE public.app_forbidden(id int)",
        "ALTER TABLE lab.lab_record ADD COLUMN forbidden int",
        "DELETE FROM lab._sqlx_migrations",
        "DELETE FROM public._sqlx_migrations",
        "SET ROLE lab_admin",
    ] {
        denied(&app, statement).await?;
    }
    Ok(
        json!({"migrator": format!("{context}_migrator"), "app": format!("{context}_app"),
        "superuser": false, "bypassrls": false, "app_ddl_denied": true, "app_history_write_denied": true,
        "app_set_admin_denied": true, "rls_select_update_insert_verified": true, "app_schema_n_usable": true}),
    )
}

async fn boundary(cluster: &mut Cluster, admin: &PgPool) -> Result<Value> {
    for context in ["central", "control"] {
        let other = if context == "central" {
            "control"
        } else {
            "central"
        };
        for role in [format!("{context}_app"), format!("{context}_migrator")] {
            for database in [format!("hiqnet_lab_{other}"), "hiqnet_lab_source".into()] {
                let pool = PgPoolOptions::new()
                    .max_connections(1)
                    .acquire_timeout(Duration::from_secs(3))
                    .connect_lazy_with(cluster.options(&database, &role, "public"));
                cluster.pools.push(pool.clone());
                let result = sqlx::query("SELECT 1").execute(&pool).await;
                require(
                    matches!(result, Err(sqlx::Error::Database(ref error)) if error.code().as_deref() == Some("42501")),
                    "rol accedió a otro contexto o fallo distinto de CONNECT denegado",
                )?;
                pool.close().await;
            }
        }
    }
    let source = cluster.pool("hiqnet_lab_source", ADMIN, "public").await?;
    require(
        tables(&source).await?.is_empty(),
        "source contaminada por roots SQLx",
    )?;
    let databases: i64 = sqlx::query_scalar("SELECT count(*) FROM pg_database WHERE datname IN ('hiqnet_lab_central','hiqnet_lab_control','hiqnet_lab_source')")
        .fetch_one(admin).await?;
    require(databases == 3, "faltan bases independientes")?;
    Ok(
        json!({"cross_database_connect_denials": 8, "sqlstate": "42501", "source_empty": true, "independent_databases": 3}),
    )
}

async fn restore(
    cluster: &mut Cluster,
    pool: &PgPool,
    context: &str,
    evidence: &Evidence,
    checks: &mut Vec<Value>,
) -> Result<Value> {
    let database = format!("hiqnet_lab_{context}");
    let target = format!("{database}_restore");
    let dump = format!("{context}.dump");
    let before = snapshot(pool).await?;
    let socket = cluster.socket.to_string_lossy().into_owned();
    cluster.execute(
        "pg_dump",
        &[
            "--host",
            &socket,
            "--port",
            "5432",
            "--username",
            ADMIN,
            "--no-password",
            "--format=custom",
            "--file",
            &dump,
            &database,
        ],
        30,
    )?;
    let bytes = fs::read(cluster.temp.path().join(&dump))?;
    evidence.write(&format!("backup-fixtures/postgres-{context}.dump"), &bytes)?;
    observe(
        checks,
        &format!("postgres_{context}_ddl_rollback"),
        ddl_failure(pool, context, evidence).await,
    )?;
    // El backup ya existe antes del upgrade fallido; restore sobre otra BD vacía.
    cluster.execute(
        "pg_restore",
        &[
            "--host",
            &socket,
            "--port",
            "5432",
            "--username",
            ADMIN,
            "--no-password",
            "--exit-on-error",
            "--single-transaction",
            "--dbname",
            &target,
            &dump,
        ],
        30,
    )?;
    let restored = cluster
        .pool(&target, &format!("{context}_migrator"), "lab")
        .await?;
    let after = snapshot(&restored).await?;
    require(
        before == after,
        "pg_dump/pg_restore no recuperó exactamente schema/datos/historial",
    )?;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM lab.lab_record")
        .fetch_one(&restored)
        .await?;
    require(count == 2, "restore smoke test falló")?;
    restored.close().await;
    // Reabrir de verdad; no reutilizar la conexión anterior.
    let reopened = cluster
        .pool(&target, &format!("{context}_migrator"), "lab")
        .await?;
    require(
        after == snapshot(&reopened).await?,
        "restore cambió al reabrir",
    )?;
    let detail = json!({"before": before, "after": after, "dump_sha256": sha256(&bytes),
        "dump_bytes": bytes.len(), "restored_to_separate_database": true, "reopened": true, "smoke_count": count,
        "digest_method": "ordered SQL jsonb data and SQL catalog; not dump byte equality"});
    evidence.json(
        &format!("backup-fixtures/postgres-{context}-restore.json"),
        &detail,
    )?;
    Ok(detail)
}

async fn context_checks(
    cluster: &mut Cluster,
    context: &str,
    evidence: &Evidence,
    checks: &mut Vec<Value>,
) -> Result<Value> {
    let database = format!("hiqnet_lab_{context}");
    let user = format!("{context}_migrator");
    let root_pool = cluster.pool(&database, &user, "public").await?;
    let clean = observe(
        checks,
        &format!("postgres_{context}_clean_partial_root"),
        clean_root(&root_pool, context, evidence).await,
    )?;
    sqlx::query("CREATE SCHEMA lab").execute(&root_pool).await?;
    let pool = cluster.pool(&database, &user, "lab").await?;
    seed_v1(&pool).await?;
    let upgrade = observe(
        checks,
        &format!("postgres_{context}_fixture_upgrade_preserves_data"),
        upgrade(&pool, context, evidence).await,
    )?;
    observe(
        checks,
        &format!("postgres_{context}_checksum_version_mismatch"),
        checksum(&pool, context, evidence).await,
    )?;
    let rerun = observe(
        checks,
        &format!("postgres_{context}_rerun_exact_history"),
        rerun(&pool).await,
    )?;
    let privileges = observe(
        checks,
        &format!("postgres_{context}_migration_vs_app_privileges"),
        privileges(cluster, context, &pool).await,
    )?;
    let result = restore(cluster, &pool, context, evidence, checks).await;
    let restore = observe(
        checks,
        &format!("postgres_{context}_dump_restore_exact_sql_digests"),
        result,
    )?;
    require(
        history(&pool, "public").await? == clean["history"],
        "fixtures modificaron tracking del root parcial",
    )?;
    Ok(
        json!({"clean_partial_root": clean, "upgrade": upgrade, "rerun": rerun, "privileges": privileges, "restore": restore}),
    )
}

async fn execute_checks(
    cluster: &mut Cluster,
    evidence: &Evidence,
    checks: &mut Vec<Value>,
) -> Result<Value> {
    let admin = cluster.ready().await?;
    let runtime = observe(
        checks,
        "postgres_native_17_private_socket",
        isolation(cluster, &admin).await,
    )?;
    observe(
        checks,
        "postgres_separate_database_setup",
        setup(cluster, &admin).await,
    )?;
    let mut contexts = json!({});
    // Un contexto fallido no impide observar el otro; nunca se convierte en PASS.
    for context in ["central", "control"] {
        match context_checks(cluster, context, evidence, checks).await {
            Ok(value) => contexts[context] = value,
            Err(error) => {
                contexts[context] = json!({"status": "FAILED", "error_kind": safe_error(&error)});
                observe(
                    checks,
                    &format!("postgres_{context}_context_complete"),
                    Err(error),
                )
                .ok();
            }
        }
    }
    let central = &contexts["central"]["clean_partial_root"]["history"];
    let control = &contexts["control"]["clean_partial_root"]["history"];
    let _ = observe(
        checks,
        "postgres_independent_root_tracking",
        (|| {
            require(
                !central.is_null()
                    && !control.is_null()
                    && central[0]["version"] == 1
                    && control[0]["version"] == 1
                    && central[0]["checksum"] != control[0]["checksum"],
                "roots sin tracking independiente",
            )?;
            Ok(
                json!({"central": central, "control": control, "same_version_different_checksums": true}),
            )
        })(),
    );
    let _ = observe(
        checks,
        "postgres_database_boundaries",
        boundary(cluster, &admin).await,
    );
    Ok(json!({"runtime": runtime, "contexts": contexts}))
}

/// No acepta DATABASE_URL, ni expone handles, procesos u opciones de conexión.
/// Cierra pools, para/recolecta el hijo y elimina el namespace antes de retornar.
pub(crate) async fn run(
    bin: &Path,
    evidence: &Evidence,
) -> std::result::Result<Value, crate::LabError> {
    let mut checks = Vec::new();
    let mut cluster = match Cluster::start(bin) {
        Ok(cluster) => cluster,
        Err(error) => {
            evidence.json("failure-logs/postgres-startup.json", &json!({"status": "FAILED", "error_kind": safe_error(&error),
                "reason": "Se requieren binarios nativos PG17 válidos, usuario no root y PGOPTIONS ausente; no se usa DATABASE_URL"}))?;
            return Err(error);
        }
    };
    let result = match tokio::time::timeout(
        Duration::from_secs(180),
        execute_checks(&mut cluster, evidence, &mut checks),
    )
    .await
    {
        Ok(result) => result,
        Err(_) => Err(invalid("PostgreSQL lab excedió 180 segundos")),
    };
    // Este cierre explícito también cubre Result::Err. Drop es la segunda barrera
    // para cancelación/unwind; no se delega la vida del servidor al llamador.
    cluster.close_pools().await;
    let stop = cluster.stop();
    let log = fs::read(cluster.temp.path().join("postgres.log"));
    let path = cluster.temp.path().to_path_buf();
    drop(cluster);
    let cleanup = observe(
        &mut checks,
        "postgres_cleanup",
        (|| {
            stop?;
            require(!path.exists(), "no se eliminó namespace temporal")?;
            Ok(
                json!({"pools_closed": true, "child_reaped": true, "temporary_cluster_removed": true}),
            )
        })(),
    );
    if let Ok(log) = log {
        evidence.write("failure-logs/postgres-process.log", log)?;
    }
    let observations = match result {
        Ok(value) => value,
        Err(error) => {
            observe(&mut checks, "postgres_execution_complete", Err(error)).ok();
            json!({})
        }
    };
    // Conservar la versión observada aun si una precondición posterior falla.
    let runtime = observations
        .get("runtime")
        .cloned()
        .or_else(|| {
            checks
                .iter()
                .find(|check| {
                    check["id"] == "postgres_native_17_private_socket"
                        && check["status"] == "PASSED"
                })
                .map(|check| check["detail"].clone())
        })
        .unwrap_or(Value::Null);
    let technical_status = if checks.iter().any(|check| check["status"] == "FAILED") {
        "FAILED"
    } else {
        "PASSED"
    };
    let report = json!({"status": technical_status, "technical_status": technical_status,
        "scope": "synthetic_technical_and_partial_roots", "normative_status": "BLOCKED", "normative_complete": false,
        "normative_blockers": ["DDL13-02/03 requieren esquemas completos; roots actuales parciales", "Especificación completa 82/83 tablas ausente; no certificada", "Sin ejecución Flyway/PAR-01..09/AS-01..10 en este módulo"],
        "technical_checks": checks, "runtime": runtime, "contexts": observations["contexts"],
        "source": {"database": "hiqnet_lab_source", "flyway_executed": false, "lifetime": "private cluster destroyed before return", "connection_options_exported": false},
        "cleanup": cleanup.ok()});
    evidence.json("postgres-technical-report.json", &report)?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_postgresql_17_is_accepted() {
        assert!(version_17("postgres (PostgreSQL) 17.11\n"));
        assert!(version_17("pg_dump (PostgreSQL) 17.11"));
        for version in [
            "postgres (PostgreSQL) 16.11",
            "postgres (PostgreSQL) 18.1",
            "17",
            "postgres (PostgreSQL) 170.1",
        ] {
            assert!(!version_17(version));
        }
    }

    #[test]
    fn missing_native_binaries_are_an_error_not_a_pass() -> crate::Result<()> {
        let temp = tempfile::tempdir()?;
        assert!(validate_bin(temp.path()).is_err());
        Ok(())
    }

    #[tokio::test]
    async fn requesting_run_without_binaries_returns_error_and_failure_evidence()
    -> crate::Result<()> {
        let temp = tempfile::tempdir()?;
        let evidence = Evidence::create(&temp.path().join("evidence"))?;
        assert!(run(temp.path(), &evidence).await.is_err());
        let startup: Value = serde_json::from_slice(&fs::read(
            evidence.root.join("failure-logs/postgres-startup.json"),
        )?)?;
        assert_eq!(startup["status"], "FAILED");
        assert!(
            !evidence
                .root
                .join("postgres-technical-report.json")
                .exists()
        );
        Ok(())
    }

    #[test]
    fn checksum_rejection_requires_exact_variant_and_version() {
        assert!(is_checksum_mismatch(
            &sqlx::migrate::MigrateError::VersionMismatch(1)
        ));
        assert!(!is_checksum_mismatch(
            &sqlx::migrate::MigrateError::VersionMismatch(2)
        ));
        assert!(!is_checksum_mismatch(
            &sqlx::migrate::MigrateError::VersionMissing(1)
        ));
    }

    #[test]
    fn expected_ddl_failure_requires_sqlstate_and_migration_version() {
        assert!(!is_ddl_failure(
            &sqlx::migrate::MigrateError::VersionMismatch(3)
        ));
    }

    #[test]
    fn details_do_not_publish_driver_messages() {
        let error = LabError::Invalid("postgres://secret:password@example/private".into());
        assert_eq!(safe_error(&error), "lab_precondition_or_assertion");
    }
}

// La integración PostgreSQL real se ejecuta mediante el CLI y
// scripts/postgres_integration_test.sh; así `cargo test --all-features` no
// requiere un clúster ni convierte una ausencia de binarios en un test omitido.
