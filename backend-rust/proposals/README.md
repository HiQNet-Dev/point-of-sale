# Propuestas de migración — cerrado en v0.4.0

**Estado: PROMOVIDAS.** El conjunto físico `0002`–`0017` dejó de ser candidato y
vive en `backend-rust/migrations/local/`, que es el único root que descubre el
migrador productivo (`sqlx::migrate!("../../migrations/local")`).

La aceptación de las decisiones físicas asociadas (D-AUTH, D-INST, D-CAT,
D-CASH, D-SALES, D-INV, CC21-03) se registra en
`docs/01-architecture/adr/0026-local-schema-promotion-v040.md`, con la evidencia
de pruebas que la sustenta.

## Reglas

- Las migraciones promovidas son **inmutables**: toda corrección posterior exige
  una migración nueva con versión superior.
- `0001_organization_branch.sql` no se modificó.
- Este directorio ya no contiene DDL ejecutable. Se conserva para explicar por
  qué dejó de existir la separación `proposals/` ↔ `migrations/local/` y para
  evitar que alguien reintroduzca una copia divergente.

## Dónde mirar

| Necesidad | Documento |
| --- | --- |
| Orden y contenido físico | `backend-rust/migrations/local/` |
| Decisión y evidencia | `docs/01-architecture/adr/0026-local-schema-promotion-v040.md` |
| Estado por módulo | `docs/05-development/rust-migration-board.md` |
| Modelo de datos | `docs/02-database/` |
