# Recuperación y reconstrucción controlada — Incrementos 0d-B / 1A / 1B

Fecha de inicio: 2026-10-29. Rama de trabajo: `feat/backend/rust-organization-branch-1b`.

## INC-RECOVERY-01

**Incremento 1A no recuperable en el entorno actual.**

Se verificaron sin cambiar referencias, limpiar objetos ni ejecutar GC:

- `origin/dev@329c0984722e8339f3e3b29497960d426ac2c43c`;
- ramas locales/remotas, reflogs y worktrees registrados;
- el objeto esperado `c8052d4d7edfe860cfb7b71770622c749385c536` mediante `git cat-file`;
- objetos inalcanzables con `git fsck --full --no-reflogs --unreachable`;
- bundles, parches y worktrees accesibles del directorio de proyectos;
- `git ls-remote` de la rama histórica.

El objeto, la rama `feat/backend/rust-0d-readiness`, `organization.rs` y una copia
Rust recuperable no estaban disponibles. La única historia alcanzable de la
foundation Rust era `4236fabb`. Por ello, el código posterior es una
**reconstrucción**, no una recuperación ni una atribución al commit perdido.

## Alcance reconstruido

`backend-rust/crates/pos-core/src/organization_branch.rs` contiene dominio puro y
puertos de aplicación para:

- identidad UUID canónica estable, código de negocio normalizado e inmutable;
- `Organization` con nombre, ZoneId compatible estructuralmente, ISO 4217 según el
  catálogo exportado del JDK de referencia, locale BCP-47, estado, política legacy
  de stock negativo y CAS por revisión;
- `Branch` con owner inmutable, código scoped, estado, herencia/override triestado
  de timezone y CAS;
- patches atómicos: cualquier error deja el agregado sin mutación;
- puertos pequeños de repositorio, unidad de trabajo, auditoría append-only,
  generador de identidad y autorización contextual deny-by-default;
- casos de uso de bootstrap de Organization y CRUD scoped de Branch.

El alta ordinaria de organización sigue cerrada: Java reserva el primer
provisioning para `POST /bootstrap`; el caso `create` exige explícitamente el
permiso interno `BootstrapOrganization`. No hay adaptador de producción allow-all.

## Persistencia experimental

`pos-storage-sqlite` incorpora `SqliteOrganizationBranchStore` sobre
`migrations/local/0001_organization_branch.sql`, sin editar el DDL:

- SQL parametrizado, rehidratación validada y errores tipados;
- UNIQUE, FK y SQLite STRICT reales;
- lecturas/listados de Branch siempre con `organization_id`;
- CAS con `WHERE organization_id/id/version` (o `id/version` para Organization),
  y resultado diferenciado `Updated` / `Missing` / `Conflict`;
- reapertura durable cubierta por una prueba de laboratorio;
- rollback de negocio/auditoría demostrado con una tabla fixture creada por la
  prueba dentro de SQLite, no como migración ni audit trail productivo.

El esquema Local no tiene una tabla de auditoría aprobada. Por eso la escritura
productiva de `AuditAppender` permanece sin adaptador SQLite y no se declara
operativa: una futura composición debe ejecutar repositorios y audit append-only en
la misma transacción antes de habilitar mutaciones auditables fuera del laboratorio.

## Paridad y diferencias conocidas

| Área | Estado | Límite |
| --- | --- | --- |
| `DOMINIO_RUST_VERIFICADO` | Reconstruido con tests fuente | La ejecución puede estar bloqueada por App Control del entorno. |
| `APLICACION_RUST_VERIFICADA` | Casos de uso y rollback en unidad de trabajo in-memory cubiertos por tests fuente | No hay composición productiva de auth, clock ni audit. |
| `SQLITE_LAB_VERIFICADO` | Adaptador y pruebas SQLx reales añadidos | Esquema parcial; no equivale a DDL13 ni a audit productivo. |
| `PARIDAD_INTEGRAL_PENDIENTE` | Pendiente | HTTP, JWT/RBAC DB-authoritative, OpenAPI, React, auditoría productiva y E2E. |

### Validadores cuyo límite se mantiene explícito

- Java `ZoneId` consulta proveedores de reglas TZDB instalados. El núcleo Rust
  reconstruido acepta aliases Java, offsets válidos hasta ±18:00 y una forma TZDB;
  aún no incorpora una base TZDB versionada, por lo que no declara paridad para la
  existencia histórica de cada ID TZDB.
- El catálogo ISO 4217 se exportó del JDK disponible durante la reconstrucción.
  Debe regenerarse o versionarse cuando cambie el JDK de referencia.
- `String.trim()` Java y la longitud UTF-16 se implementan explícitamente. Rust
  `str::trim()` no se usa para esos campos; los espacios Unicode como NBSP se
  preservan como Java.
- `Quantity` no se reconstruyó en este incremento porque Organization/Branch no la
  consume. Sigue como trabajo separado de 0d-B y no debe confundirse con dinero o
  valoración.

## Gates normativos y operación

La matriz normativa conserva exactamente el estado histórico documentado en
`rust-foundation.md`: **1/30 aprobado (PAR-08), 29 pendientes/bloqueados**;
`--require-complete` no se modificó ni se ejecutó como éxito. La reconstrucción de
Organization/Branch no aprueba DDL13, PAR ni AS adicionales. ADR-0021 y CC-0021
permanecen `PROPOSED`; no se añaden migraciones físicas controvertidas.

Los comandos de Rust deben repetirse en un host que permita ejecutar `cargo-fmt`,
build scripts y binarios de prueba. En este entorno Windows, App Control devolvió
`os error 4551` al lanzar `cargo-fmt`, build scripts y ejecutables de test; esos
bloqueos no se cuentan como resultados verdes ni como fallos del código.
