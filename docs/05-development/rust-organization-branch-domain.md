# Dominio Rust de Organization y Branch — autoridad única

**Estado: CONSOLIDADO en `pos-core::organization_branch`.**
Implementa reglas puras en `pos-core`; no habilita persistencia nueva, RBAC, HTTP,
React ni sustitución del backend Java.

## Autoridad única

La rama publicada mantenía dos modelos en paralelo:

- `pos-core/src/organization.rs` y `pos-core/src/branch.rs`, reexportados desde la
  raíz del crate pero **sin ningún consumidor** en el workspace.
- `pos-core/src/organization_branch/`, usado por `pos-application`,
  `pos-storage-sqlite`, `apps/pos-desktop` y `tools/pos-lab`.

Se conservó `organization_branch` como única autoridad y se eliminaron los módulos
paralelos. Las validaciones útiles que solo tenía el modelo duplicado se portaron:

| Regla                    | Antes en el módulo paralelo            | Ahora en `organization_branch`                          |
| ------------------------ | -------------------------------------- | ------------------------------------------------------- |
| Offsets `ZoneId` Java    | `+H`, `+HHMMSS`, `+HH:MM:SS`           | `valid_offset` acepta las seis formas de `ZoneId.of`     |
| Zona nombrada            | `chrono_tz::Tz`                        | `valid_tzdb_zone` valida contra TZDB, no solo la forma   |
| Rango de offset          | `≤ ±18:00` con minutos/segundos válidos | mismo límite, corregido también para minutos y segundos  |
| Locale BCP-47            | roundtrip canónico con ICU             | `LocaleTag::new` exige la forma canónica de ICU         |
| Revisión                 | `i64::MAX`                             | `MAX_REVISION` y rechazo atómico al desbordar            |

Se corrigió un defecto real del validador de offsets: aceptaba minutos fuera de
rango en formas con hora menor que 18 (`+00:99`, `+01:60`). Los tests cubren ahora
`+18:01`, `+19`, `+18:00:01`, `+01:60`, `+00:99` y `Z`/`UTC`/`GMT`/`UT`.

## Alcance materializado

| Componente   | Implementado en Rust                                                                                    | Fuera del dominio                        |
| ------------ | ------------------------------------------------------------------------------------------------------- | ---------------------------------------- |
| Organization | `EntityId`, código inmutable, nombre, `Timezone`, ISO 4217, BCP-47, estado, política legada y revisión    | Generación UUID, reloj, unicidad y CAS SQL |
| Branch       | `EntityId`/owner inmutables, código/nombre, estado, timezone override y herencia, `OwnershipMismatch`    | Lookup scopeado, unicidad `(org, code)`  |
| Quantity     | se conserva en `money_scale`                                                                            | Codec/persistencia SQLite                |

El dominio no depende de SQLx, SQLite, PostgreSQL, Axum, Tauri, Spring ni JPA.

## Paridad Java → Rust comprobada

| Regla AS-IS                          | Fuente Java                            | Rust                                                     | Prueba Rust                                        |
| ------------------------------------ | -------------------------------------- | -------------------------------------------------------- | -------------------------------------------------- |
| Nombre trim Java, no vacío, 120 UTF-16 | `Organization.java`, `Branch.java`     | `DisplayName` con `java_trim` y `utf16_len`               | creación y patches                                 |
| Slug minúsculo máximo 64 e inmutable | `OrganizationService`, `BranchService` | `BusinessCode::new` normaliza y valida; sin mutador       | vectores inválidos y acceso inmutable              |
| `ZoneId.of` válido                   | `ZoneId.of`                            | `Timezone::new`: TZDB + offsets fijos + prefijos          | formas aceptadas y rechazadas                      |
| Moneda ISO 4217 válida y mayúsculas  | `Currency.getInstance`                 | `IsoCurrency` con el catálogo JDK embebido (233 códigos)  | `MXN` válido, `XYZ` rechazado, catálogo ordenado   |
| Locale BCP-47 con extensiones        | `Locale.forLanguageTag`                | `LocaleTag` con perfil y roundtrip ICU                    | `es-MX-u-ca-gregory` aceptado; `es_MX` rechazado    |
| Defaults Organization                | `Organization.create`                  | `ACTIVE` / `STRICT`                                       | creación                                           |
| Estado legacy stock negativo         | `InventoryNegativeStockPolicy`         | `NegativeStockPolicy` `STRICT`/`ALLOW_NEGATIVE`           | patch, sin valores nuevos                          |
| Patch no-op y revisión               | servicios Java + `@Version`            | `patch` solo incrementa ante cambio                       | no-op, modificación y revisión stale               |
| Límite de revisión                   | `BIGINT` Java/PostgreSQL               | `MAX_REVISION = i64::MAX`; overflow sin mutar             | incremento en el máximo rechazado                  |
| Owner y código Branch inmutables     | `Branch`                               | campos sin mutador y `OwnershipMismatch` explícito        | patch con owner ajeno                              |
| Timezone Branch ternaria             | `PatchField` Java                      | `TimezoneOverridePatch::Keep`/`Set`/`Clear`               | override y retorno a herencia                      |

Los módulos son
[`organization_branch/mod.rs`](../../backend-rust/crates/pos-core/src/organization_branch/mod.rs),
[`organization.rs`](../../backend-rust/crates/pos-core/src/organization_branch/organization.rs),
[`branch.rs`](../../backend-rust/crates/pos-core/src/organization_branch/branch.rs) y
[`values.rs`](../../backend-rust/crates/pos-core/src/organization_branch/values.rs).

`DisplayName` y `BusinessCode` usan ya la semántica Java (`String.trim()` y longitud
en unidades UTF-16), por lo que la diferencia de paridad que documentaba el
incremento 1A queda cerrada.

## SQLite y Quantity

`Quantity::MAX_MICROS` para `NUMERIC(19,6)` es `9_999_999_999_999_999_999`, mayor
que `i64::MAX`. Un `INTEGER` SQLite STRICT no cubre todo el rango sin pérdida. No
se autoriza `REAL` ni casts silenciosos: el adaptador debe usar una codificación
decimal canónica TEXT con validación de escala/rango, o un contrato aprobado. Este
incremento no cambia DDL ni define ese codec.

## Validación

```sh
cd backend-rust
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace --all-features
```

Las pruebas cubren construcción, defaults, valores inválidos, formas de offset Java,
validación TZDB, patch no-op/material, revisión stale, límite `i64::MAX`, ownership y
herencia de timezone. No acreditan paridad de persistencia, RBAC, aislamiento por
query, auditoría ni contratos JSON.

## Dependencias justificadas

| Dependencia       | Uso aislado                                                                              |
| ----------------- | ---------------------------------------------------------------------------------------- |
| `chrono-tz`       | Validación TZDB de `ZoneId` sin I/O; los offsets fijos se validan con gramática explícita |
| `icu_locale_core` | Perfil y forma canónica BCP-47, incluidas extensiones de locale                          |
| `serde`/`serde_json` | Metadatos de auditoría e identidad, sin persistencia                                  |

`uuid` e `iso_currency` se retiraron de `pos-core` al perder su único consumidor. El
catálogo ISO se conserva embebido porque replica el de `Currency.getInstance()` del
JDK de referencia, que incluye códigos históricos que el crate `iso_currency` no
acepta.

ADR-0021 y CC-0021 no se modifican: la política de stock negativa conserva
únicamente los valores Java y no adopta pending, retornos ni costeo propuesto.
