# Incremento 1A — dominio Rust de Organization y Branch

**Estado: PARCIAL.** Implementa únicamente reglas puras en `pos-core`; no habilita
persistencia, casos de uso, RBAC, HTTP, React ni sustitución del backend Java.

## Alcance materializado

| Componente   | Implementado en Rust                                                                                         | Fuera de este incremento                                                            |
| ------------ | ------------------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------- |
| Organization | UUID recibido, código inmutable, nombre, `ZoneId` Java, ISO 4217, BCP-47, estado, política legada y revisión | Generación UUID, reloj/timestamps, unicidad, CAS SQL, auditoría, autorización y API |
| Branch       | UUID/owner inmutables, código/nombre, estado, timezone override e herencia, revisión                         | Lookup scopeado, unicidad `(organization_id, code)`, auditoría, autorización y API  |
| Quantity     | Se conserva `NUMERIC(19,6)` exacto en `i128` dentro del dominio                                              | Codec/persistencia SQLite y reglas MWAC-DEC1                                        |

Los módulos son [`organization.rs`](../../backend-rust/crates/pos-core/src/organization.rs)
y [`branch.rs`](../../backend-rust/crates/pos-core/src/branch.rs). No dependen de
SQLx, SQLite, PostgreSQL, Axum, Tauri, Spring ni JPA.

## Paridad Java → Rust comprobada

| Regla AS-IS                          | Fuente Java                            | Rust 1A                                                  | Prueba Rust                                                 |
| ------------------------------------ | -------------------------------------- | -------------------------------------------------------- | ----------------------------------------------------------- |
| Nombre trim, no vacío, máximo 120    | `Organization.java`, `Branch.java`     | `DisplayName`                                            | creación y patches                                          |
| Slug minúsculo máximo 64 e inmutable | `OrganizationService`, `BranchService` | `BusinessCode`; sin mutador                              | vectores inválidos y acceso inmutable                       |
| `ZoneId` válida                      | `ZoneId.of`                            | TZDB `chrono-tz` + offsets fijos propios, `IanaTimezone` | inválida rechazada; TZDB, `Z`, offsets y prefijos aceptados |
| Moneda ISO 4217 válida y mayúsculas  | `Currency.getInstance`                 | `iso_currency`, `OrganizationCurrency`                   | MXN válido, XYZ rechazado                                   |
| Locale BCP-47 con extensiones        | `Locale.forLanguageTag`                | `icu_locale_core::Locale`, `LocaleTag`                   | inválido rechazado; `en-u-ca-gregory` aceptado              |
| Defaults Organization                | `Organization.create`                  | `ACTIVE` / `STRICT`                                      | creación                                                    |
| Estado legacy stock negativo         | `InventoryNegativeStockPolicy`         | `STRICT` / `ALLOW_NEGATIVE`                              | patch y no se agregan valores nuevos                        |
| Patch no-op y revisión               | servicios Java + `@Version`            | `apply_patch` solo incrementa ante cambio                | no-op, modificación y versión stale                         |
| Owner y código Branch inmutables     | `Branch`                               | campos sin mutador                                       | construcción/patch                                          |
| Timezone Branch ternaria             | `PatchField` Java                      | `Keep` / `Set` / `ClearToInherited`                      | override y retorno a herencia                               |

La diferencia deliberada es que Java asigna UUID/timestamps mediante `UUID.randomUUID`
y callbacks JPA; Rust recibe UUID de la aplicación y no recibe un reloj todavía. La
unicidad y el compare-and-swap persistente deben implementarse posteriormente con
constraints SQLx y casos de uso, no con colecciones en memoria.

`IanaTimezone` conserva su nombre por compatibilidad interna, pero valida un `ZoneId`
Java: zonas TZDB y `Z`/offsets fijos con signo, incluso con los prefijos `UTC`, `GMT`
y `UT`. `LocaleTag` admite extensiones BCP-47, como `en-u-ca-gregory`. Permanece una
diferencia acotada que debe evaluarse antes de paridad HTTP: Java limita `String.length()`
en unidades UTF-16 y usa `String.trim()`, mientras `DisplayName` cuenta escalares Unicode
y usa `str::trim()` de Rust.

## SQLite y Quantity

`Quantity::MAX_MICROS` para `NUMERIC(19,6)` es `9_999_999_999_999_999_999`, mayor
que `i64::MAX` (`9_223_372_036_854_775_807`). Por tanto un entero `INTEGER` SQLite
STRICT no puede almacenar todo el rango sin pérdida. No se autoriza `REAL` ni casts
silenciosos. El futuro adaptador SQLite debe usar una codificación decimal canónica
TEXT con validación de escala/rango, o limitar el rango mediante un contrato aprobado
y probado contra PostgreSQL; este incremento no cambia DDL ni define ese codec.

## Validación

```sh
cd backend-rust
cargo clippy --locked -p pos-core --all-targets --all-features -- -D warnings
cargo test --locked -p pos-core
```

Las pruebas actuales cubren construcción, defaults, UUID preservado, valores inválidos,
patch no-op/material, revisión stale, ownership y herencia timezone. No acreditan
paridad de persistencia, RBAC, aislamiento por query, auditoría ni contratos JSON.

## Dependencias justificadas

| Dependencia       | Uso aislado                                                                           |
| ----------------- | ------------------------------------------------------------------------------------- |
| `uuid`            | Identidades estables proporcionadas por la aplicación                                 |
| `chrono-tz`       | Validación TZDB de `ZoneId` sin I/O; offsets fijos se validan con gramática explícita |
| `iso_currency`    | Validación ISO 4217 equivalente al catálogo Java                                      |
| `icu_locale_core` | Validación BCP-47, incluidas extensiones de locale                                    |

ADR-0021 y CC-0021 no se modifican: la política de stock negativa conserva únicamente
los valores Java y no adopta pending, retornos ni costeo propuesto.
