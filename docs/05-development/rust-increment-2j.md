# Incremento 2J — consolidación Organization/Branch y CI de dos niveles

**Estado: PARCIAL Y VERIFICADO EN LOCAL. Sin publicación ni CI remoto.**
Rama: `feat/fullstack/rust-integration-recovery`, creada desde
`fix/gitleaks-synthetic-vectors-2i` (`4e24acaa`) y esta desde `e2de3f55`.

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
| Interoperabilidad Java↔Rust | `node tools/credential-interop/run.mjs` | PASS, 10/10 checks |
| Gitleaks | `gitleaks dir <export>/backend-rust --redact` | **0 leaks** |
| Contratos de CI | `python3 .github/scripts/test_ci_changes.py` (contenedor Linux) | **25/25** |
| Verificador de evidencia | `python3 -B test_verify_evidence.py` (contenedor) | **19/19** |

No se ejecutó CI remoto, no hay run sobre estos SHAs y no se atribuyen checks
antiguos al árbol nuevo.

## 4. Tablero

| Frente | Estado | Evidencia | Commit | Bloqueador |
| --- | --- | --- | --- | --- |
| Organization/Branch único | DONE | 133 tests + guardas | `e7435198` | — |
| Gitleaks | DONE | 0 leaks en export limpio | `5e2ec804` | — |
| CI técnico/normativo | DONE (local) | 25/25 contratos | `a9c4b6c8` | CI remoto no autorizado |
| Provisioning opción B | PENDING | — | — | Sin implementar |
| Migración `0017` | PENDING | — | — | Sin implementar |
| Bootstrap con derecho de provisioning | PENDING | — | — | Depende de lo anterior |
| Login/sesiones/RBAC | PENDING | — | — | Sin implementar |
| Windows ACL / Tauri | PENDING | — | — | Sin implementar |
| Frontend de identidad | PENDING | — | — | Sin implementar |
| CI normativo | **BLOCKED** | 1 PASSED / 28 BLOCKED / 1 NOT_RUN | — | Evidencia normativa incompleta |
| PR #84 | DRAFT | GitHub | `e2de3f55` publicado | Cambios 2J sin publicar |

## 5. Lo que falta antes de pedir el merge a `dev`

1. Publicar la rama consolidada (autorización vigente, condicionada a recuperar
   2E–2G).
2. Ejecutar CI remoto sobre el SHA final y confirmar que los checks corresponden a
   ese SHA.
3. Decidir sobre 2E–2G: recuperarlos con el bundle o declarar formalmente que se
   reconstruyen desde cero.
4. Implementar los frentes B–F si se mantiene el alcance del megaprompt.
5. Configurar las reglas de GitHub (`dev`/`main`), que siguen sin protección.

`TECHNICALLY_READY`: parcial. `READY_FOR_REVIEW`: no. `MERGE_READY`: no.

El estado normativo real sigue siendo **1 PASSED / 28 BLOCKED / 1 NOT_RUN**. Este
incremento no lo modifica ni lo presenta como aprobado.
