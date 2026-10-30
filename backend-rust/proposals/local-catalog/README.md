# Propuestas de migración Local — Catálogo

**Estado: PROPUESTA. No ejecutar. No está cargada por `pos-local` ni por SQLx.**

`0008_catalog.proposed.sql` vive fuera de `backend-rust/migrations/` porque SQLx
solo descubre `migrations/local`. Añadirla al migrador es un cambio separado que
requiere autorización del titular.

## Dependencias

| Orden | Migración candidata | Depende de | Estado |
| --- | --- | --- | --- |
| 0001 | `0001_organization_branch.sql` (existente) | — | Publicada, intocable |
| 0002–0007 | `proposals/local-auth/*` | `0001` | Propuesta (D-AUTH/D-INST) |
| 0008 | `0008_catalog.proposed.sql` | `0001` | Propuesta (independiente de D-AUTH) |

El catálogo Local **no** referencia `pos_user`, `local_session` ni `audit_event`.
Puede promoverse sin aprobar el paquete Auth/RBAC; solo requiere aceptar la
representación física del precio y los nombres de tabla.

## Diferencias con Flyway V9

| Aspecto | Java/PostgreSQL | Propuesta Local | Motivo |
| --- | --- | --- | --- |
| Nombres | `categories`, `products` | `category`, `product` | Convención Local (`organization`, `branch`) |
| Precio | `NUMERIC(19,4)` | `TEXT` canónico `0.0000` | Rango `NUMERIC(19,4)` > `i64`; SQLite no tiene decimal exacto ni admite `REAL` para dinero |
| Tenant | FK compuesta same-org | Igual, con `UNIQUE (organization_id, id)` | Paridad exacta |
| Barcode | Índice único parcial | Igual | Paridad exacta |
| Timestamps | `TIMESTAMPTZ` | `TEXT` ISO UTC | Igual que `0001` |
| Borrado | `status = INACTIVE` | Igual | Sin hard delete |

`unit_price` se normaliza en el adaptador con `Decimal4`/`UnitPrice`, que ya
rechazan coma flotante, más de cuatro decimales y valores negativos. El `CHECK`
físico añade la barrera sobre escrituras SQL directas.

## Puntos que requieren decisión del titular

| ID | Pregunta | Recomendación |
| --- | --- | --- |
| D-CAT-01 | ¿Se acepta `unit_price TEXT` canónico escala 4 en Local? | Sí: conserva exactitud y rango completo; `INTEGER` perdería valores `NUMERIC(19,4)` |
| D-CAT-02 | ¿`category`/`product` en singular? | Sí, por coherencia con `0001` |
| D-CAT-03 | ¿El catálogo puede promoverse antes que 0002–0007? | Sí: no tiene FKs hacia Auth/RBAC |

## Validación

`pos-storage-sqlite/src/catalog_store_tests.rs` aplica `0001` y esta propuesta
sobre SQLite real y prueba persistencia, unicidad, tenant, CAS, rollback por
auditoría fallida, reapertura y aislamiento entre organizaciones. La aplicación
se ejercita mediante `SqliteCatalogApplication`, que usa `VerifiedPrincipal`,
autorización DB-authoritative y `SqliteAuditWriter` en la misma transacción.
