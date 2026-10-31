# Incremento 2J — consolidación Organization/Branch y CI de dos niveles

**Estado: PUBLICADO Y VERIFICADO EN REMOTO.**
Rama: `feat/fullstack/rust-desktop-vertical-1x` (PR #84). Los incrementos 2E–2G
no se recuperaron ni se reconstruyeron: no existen en este checkout ni en el
remoto, y el titular decidió no bloquear la integración técnica por ellos.

## 1. Preflight: no había cambios nuevos en el remoto

El encargo original era traer los cambios publicados desde otro equipo. La
verificación no encontró ninguno:

| Referencia | Valor observado |
| --- | --- |
| `origin/feat/fullstack/rust-desktop-vertical-1x` | `e2de3f554e4c6958adbf8eaf65431334ee0cb1f4` (sin cambios) |
| `origin/dev` | `c8052d4d7edfe860cfb7b71770622c749385c536` (sin cambios) |
| `origin/main` | `2f0d6983325523a17a8a22ba8aa82e49b4994948` |
| PR #84 | OPEN, **Draft**, `MERGEABLE`, `UNSTABLE`, head `e2de3f55` |
| Ramas remotas | 28; ninguna nueva respecto de 2I |
| `hiqnet-rust-2g.bundle` | no existe en el disco |

`git ls-remote origin` confirmó el mismo conjunto de refs tras `git fetch --prune`.
Los commits 2E–2G (`c794814`, `eb32f06`, `3f8c94f`) siguen sin existir en este
checkout ni en el remoto. Su recuperación continúa dependiendo de un bundle o una
rama de respaldo del equipo original; ver
[`rust-2g-bundle-import-procedure.md`](rust-2g-bundle-import-procedure.md).

## 2. Lo implementado en este incremento

### Frente A — una sola autoridad Organization/Branch (commit `e7435198`)

- Se eliminaron `pos-core/src/organization.rs` y `pos-core/src/branch.rs`; no
  tenían ningún consumidor y duplicaban el modelo usado por 100 archivos.
- `organization_branch` conserva la autoridad y absorbe las validaciones del
  modelo paralelo: formas de offset de `ZoneId.of`, TZDB real y BCP-47 canónico.
- Se corrigió el validador de offsets, que aceptaba `+00:99` y `+01:60`.
- `MAX_REVISION = i64::MAX` con rechazo atómico al desbordar.
- Se retiraron `uuid` e `iso_currency` de `pos-core` al quedar sin uso.
- Los tests del dominio viven ahora en
  `pos-core/src/organization_branch/tests.rs`.

### Frente H — CI técnico y normativo separados (commit `a9c4b6c8`)

- `rust` pasa a ser **Rust Technical Integration Gate**: gitleaks, `fmt`, Clippy,
  tests, build, evidencia técnica, `cargo audit`. No ejecuta `--require-complete`.
- `rust-normative` es un job independiente (**Rust Normative Release Gate**) que
  compila PostgreSQL 17.11 y ejecuta
  [`.github/scripts/rust-normative-gate.sh`](../../.github/scripts/rust-normative-gate.sh).
  El wrapper clasifica `0 → PASSED`, `2 → BLOCKED`, cualquier otro → `FAILED`, y
  publica el exit real como output, resumen del step y evidencia. Un `FAILED` rompe
  el paso; no hay `continue-on-error` ni `|| true`.
- `CI Gate` aplica la política del destino: `dev` acepta `BLOCKED` declarado,
  `main` y `workflow_dispatch` exigen `PASSED`.
- Los tests de contrato se reescribieron para la estructura de dos jobs.

### Gitleaks (sin cambios de política)

Los dos vectores sintéticos ya se construían en tiempo de ejecución desde
`5e2ec804`. Se revalidó el árbol consolidado: **0 hallazgos** con Gitleaks 8.24.3
sobre una exportación limpia de `HEAD`. Sin allowlists, exclusiones ni
`gitleaks:allow`.

## 3. Validación ejecutada

| Comprobación | Comando | Resultado |
| --- | --- | --- |
| Formato | `cargo fmt --all -- --check` | PASS |
| Clippy | `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings` | PASS |
| Tests Rust | `cargo test --locked --workspace --all-features` | PASS (133 en `pos-core`, 136 en `pos-storage-sqlite`, resto en verde) |
| Guardas de arquitectura | `cargo test -p pos-application architecture_tests` | 4/4 PASS |
| Desktop Windows | `cargo fmt/clippy/test/build --locked` en `apps/pos-desktop` | 35/35 PASS |
| Interoperabilidad Java↔Rust | `node tools/credential-interop/run.mjs` | PASS, 10/10 checks |
| Gitleaks | `gitleaks dir <export>/backend-rust --redact` | **0 leaks** |
| Contratos de CI | `python3 .github/scripts/test_ci_changes.py` (contenedor Linux) | **28/28** |
| Verificador de evidencia | `python3 -B test_verify_evidence.py` (contenedor) | **19/19** |

### CI remoto

Run `41208791381` sobre `8ea80c00`: todos los jobs en `success`, E2E `skipped`
por ser `pull_request`.

| Job | Resultado |
| --- | --- |
| Detect Changes | success |
| Backend (Java) | success |
| Rust Technical Integration Gate | success (incluye Gitleaks y 41/41 técnicos) |
| Frontend | success |
| Validate docker-compose | success |
| Rust Local Contracts | success |
| Rust Normative Release Gate | success con estado **BLOCKED** |
| CI Gate | success (`destination=dev`) |

El wrapper publicó `normative_status=BLOCKED normative_exit=2` y el verificador
confirmó `technical=41, normative_passed=1, normative_blocked=29, exit=2`. El
estado normativo se observó y verificó; no se presentó como aprobación.

## 4. Tablero

| Frente | Estado | Evidencia | Commit | Bloqueador |
| --- | --- | --- | --- | --- |
| Organization/Branch único | DONE | 133 tests + guardas | `e7435198` | — |
| Gitleaks | DONE | 0 leaks en export limpio | `5e2ec804` | — |
| CI técnico/normativo | DONE | 28/28 contratos + run `41208791381` | `a9c4b6c8` | — |
| Desktop Windows reproducible | DONE | 35/35 con `--locked` | pendiente de publicar | — |
| CI Windows Tauri | DONE (local) | contratos + job nuevo | pendiente de publicar | CI remoto del SHA nuevo |
| Provisioning opción B | PENDING | — | — | Sin implementar |
| Migración `0017` | PENDING | — | — | Sin implementar |
| Bootstrap con derecho de provisioning | PENDING | — | — | Depende de lo anterior |
| Login/sesiones/RBAC | PENDING | — | — | Sin implementar |
| Windows ACL / Tauri | PENDING | — | — | Sin implementar |
| Frontend de identidad | PENDING | — | — | Sin implementar |
| CI normativo | **BLOCKED** | 1 PASSED / 28 BLOCKED / 1 NOT_RUN | — | Evidencia normativa incompleta |
| PR #84 | DRAFT | GitHub | `e2de3f55` publicado | Cambios 2J sin publicar |

## 5. Lo que falta antes de pedir el merge a `dev`

1. Publicar los commits correctivos (lockfile del desktop, job Windows, docs).
2. Ejecutar CI remoto sobre el SHA final y confirmar que los checks corresponden a
   ese SHA.
3. Configurar las reglas de GitHub (`dev`/`main`), que siguen sin protección.
4. Recibir al menos una revisión de código.

`TECHNICALLY_READY`: sí. `READY_FOR_REVIEW`: sí. `MERGE_READY`: pendiente de
revisión y de autorización expresa del titular.

El estado normativo real sigue siendo **1 PASSED / 28 BLOCKED / 1 NOT_RUN**. Este
incremento no lo modifica ni lo presenta como aprobado.
