# Incremento 1C — UoW SQLite experimental y portabilidad del Migration Lab

Estado: **implementación experimental verificada en Windows; no habilita producción ni reemplaza Java**.

## Portabilidad

`migration-lab` conserva el módulo PostgreSQL nativo completo en Unix. En Windows
compila mediante `postgres_unsupported.rs`: si se solicita `--postgres-bin`, el
lab devuelve un fallo técnico explícito porque no emula sockets Unix ni permisos
POSIX `0700` mediante TCP o ACLs. No marca esos checks como `PASSED`.

Ejecutar PostgreSQL 17 real y el gate completo en Linux/WSL2:

```sh
cd backend-rust
sh tools/migration-lab/scripts/build_postgres.sh
cargo run --locked -p migration-lab -- --output target/migration-evidence-linux \
  --postgres-bin target/pg17-build/install/bin --java-source
cargo run --locked -p migration-lab -- --output target/migration-gate-linux \
  --postgres-bin target/pg17-build/install/bin --java-source --require-complete
```

El segundo comando debe terminar con exit `2` mientras los 30 casos normativos
no estén completos. En Windows se ejecutan las pruebas SQLite y el workspace,
pero no se declara que PostgreSQL nativo esté validado.

## Composición asíncrona

`pos-storage-sqlite` incorpora:

- `SqliteUnitOfWork`: adquiere una única conexión y ejecuta `BEGIN IMMEDIATE`;
- `SqliteTransaction`: toda lectura, inserción, CAS y auditoría usa esa misma
  `PoolConnection`, sin `block_on`, conexiones clonadas ni mutex global;
- `SqliteBranchApplication`: composición experimental async para `create` y
  `update` de Branch, con autorización antes de abrir la transacción;
- `TransactionalAuditWriter`: puerto que recibe la transacción activa.

El orden de mutación es autorización → validación/lectura → CAS → auditoría →
commit. Cualquier error antes de commit ejecuta rollback. El no-op no llama al
writer de auditoría.

## Auditoría y autorización

No se añadió una migración Local de auditoría. La tabla `audit_fixture` existe
solo dentro de pruebas SQLx y demuestra atomicidad; **no es auditoría productiva**
ni sustituye `audit_events` Java V7. Falta una ficha física aprobada, el DDL
append-only y la resolución de actor USER/SYSTEM/JTI antes de habilitar un writer
productivo.

La UoW acepta el puerto de autorización existente, que sigue siendo una frontera,
no RBAC DB-authoritative. No hay adaptador allow-all de producción, sesiones,
JWT, memberships ni endpoints HTTP. Las pruebas usan un autorizador fixture
explícito y cubren el rechazo deny-by-default.

## Evidencia ejecutada en Windows

- `cargo fmt --all -- --check`: correcto.
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`: correcto.
- `cargo test --locked --workspace --all-features`: correcto tras resolver la
  portabilidad; la prueba UoW SQLite usa SQLite STRICT real.
- `cargo build --locked --workspace --all-features`: correcto.

Los gates siguen intactos: PAR-08 es el único normativo aprobado históricamente;
los demás DDL13/PAR/AS no se promueven por estos fixtures.
