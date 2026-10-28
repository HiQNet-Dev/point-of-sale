# PR — Foundation Rust + Migration Lab DDL-13 (BLOQUEADO)

**Destino:** `dev`  
**Rama:** `feat/backend/rust-foundation-migration-lab`  
**Estado:** listo para revisión técnica; **merge bloqueado por gates normativos**.

## Propósito

Iniciar la implementación autorizada por F4–21/F4–18 sin retirar Java, cambiar
React, usar datos reales ni declarar compatibilidad `/api/v1` inexistente. El PR
incluye foundation, laboratorio nativo PostgreSQL, origen Flyway real, auth
sintético y CI con gate normativo obligatorio.

## Contenido

- Workspace Rust con `Cargo.lock`, toolchain 1.99.1, core exacto sin
  infraestructura, adaptador SQLite y proceso local con cierre seguro.
- Roots SQLx Local/Central/Control independientes. Local tiene
  `organization`/`branch`; Central/Control tienen una tabla cada uno. **No es el
  esquema completo de 82 fichas.**
- Migration Lab con 11 checks SQLite, 19 PostgreSQL 17.11 nativo, 3 source y 8
  AUTH-MIG; 41 checks técnicos en total.
- Origen Java/Flyway V1–V25 sin modificar, 37/37 tablas y exportación determinista.
- CI con detección Rust, PG 17 desde fuente verificada, Gitleaks, RustSec,
  verificación de evidencia y gate `--require-complete` sin `continue-on-error`.
- Rustdoc/comentarios en español, guía, matriz AS-IS, auditoría física y ADR-0020.
- ADR-0021 **propuesto**, no aprobado, para los conflictos del físico.

## Verificación ejecutada

Ver [informe 0b](rust-increment-0b-report.md) para la tabla de los 30 casos.
Evidencia local final:
`backend-rust/target/increment0b-evidence-6/` y
`backend-rust/target/increment0b-gate-5/`.

- `cargo fmt`, `clippy -D warnings`, `test`, `build`: correctos.
- **59 pruebas Rust** correctas, 0 fallidas/ignoradas.
- Lab técnico: **41/41 PASSED**.
- Matriz normativa: **PAR-08 PASSED** (source exporter determinista); 29
  BLOCKED/NOT_RUN. `--require-complete`: **exit 2**.
- PostgreSQL 17.11 nativo: 19/19 checks, limpieza y restore verificados.
- 37/37 tablas source; dos instalaciones con digest funcional idéntico.
- Money: `12.3456` a escala 2 se rechaza; no se redondea. `unit_cost` NULL y
  cero se conservan distintos.
- Auth: 8/8 AUTH-MIG técnicos; política `lab-argon2-v1` **no aprobada**.
- React lint/typecheck/194 tests/build: correcto.
- Java: compilación correcta; `clean verify` **no pasa** por socket Docker
  denegado (`root:docker`). No se declara Java/E2E verde.
- RustSec: 0 vulnerabilidades/sin warnings. Gitleaks: sin leaks en fuentes
  (target generado excluido).
- Flyway V1–V25 y backend/frontend/Compose sin cambios.

## Bloqueos de merge

- [ ] Materializar/revisar las 82 fichas físicas; resolver la cifra editorial 83.
- [ ] Aprobar ADR-0021 y la semántica de `unit_cost`/`line_total`/recepción.
- [ ] Implementar las cinco dependencias sin ficha: `price_book`, `tax_profile`,
      `CostingPolicy`, `ExpenseCategory`, `plan`.
- [ ] Import target idempotente y comparación source↔target.
- [ ] DDL13-01..11, PAR-01..09 y AS-01..10 verdes con evidencia exacta.
- [ ] Regresión Java/Testcontainers/E2E en entorno con Docker accesible.
- [ ] Benchmark Argon2id release en hardware objetivo y aprobación de política.
- [ ] CI remota verde del SHA publicado y revisión humana independiente.

No se solicita merge. Un gate rojo es evidencia real del bloqueo, no un fallo a
ocultar con skips, mocks o excepciones.

## Rollback

La autoridad Java no cambia. Detener el runtime de prueba y descartar únicamente
sus artefactos sintéticos propios; no ejecutar acciones sobre instalaciones de
cliente. No hay cutover ni doble escritura que revertir.
