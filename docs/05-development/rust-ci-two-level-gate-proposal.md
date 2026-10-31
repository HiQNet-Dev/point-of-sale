# CI de dos niveles para Rust — diseño e implementación

**Estado: IMPLEMENTADO y fusionado en `dev`** (PR #84 y posteriores). Los
workflows distinguen el nivel técnico (`rust`) del normativo (`rust-normative`)
y el gate de `dev` ya tolera `BLOCKED` normativo declarado.

> **Nota 2026-10-31.** Las referencias a `backend-rust/proposals/0002`–`0016` y
al hecho de que «el migrador solo embebe `0001`» describen el estado previo a la
> promoción física registrada en ADR-0026: hoy `migrations/local` contiene
> `0001`–`0017` y `proposals/` conserva solo su README.

Fecha de redacción: 2026-10-31.
SHA base evaluado: `e2de3f554e4c6958adbf8eaf65431334ee0cb1f4`.

## 1. Motivo

El job actual `Rust (technical checks + mandatory DDL13 gate)` mezcla dos
responsabilidades con finalidades distintas:

1. Verificar que el código Rust integra correctamente (compila, pasa pruebas,
   no introduce secretos ni dependencias vulnerables) y puede entrar en `dev`
   aunque sus funcionalidades sigan inactivas comercialmente.
2. Verificar que el conjunto normativo (DDL13, PAR, AS, evidencia) está completo
   para autorizar un release.

Esa mezcla obliga a que un PR hacia `dev` quede rojo por cualquier obligatorio
normativo pendiente, lo que impide distinguir «integración técnica fallida» de
«release normativo bloqueado».

La propuesta no relaja ninguno de los dos controles: los separa y hace que cada
gate exija lo que corresponde a su destino.

## 2. Evidencia del estado real (preflight)

| Hecho | Evidencia observada |
| --- | --- |
| HEAD local y remoto de la rama | `e2de3f55` en ambos (`git ls-remote`) |
| `origin/dev` | `c8052d4d`; `HEAD...origin/dev` = `123 0` |
| `origin/main` | `2f0d6983` |
| PR #84 | Abierto, **Draft**, base `dev`, `mergeable=MERGEABLE`, `mergeStateStatus=UNSTABLE`, 100 commits, 321 archivos |
| CI remoto sobre `e2de3f55` | Rust **FAILURE**; `CI Gate` **FAILURE**; Backend, Frontend, Compose, Detect Changes y Rust Local Contracts en verde; E2E **SKIPPED** por ser `pull_request` |
| Causa del fallo Rust | `Scan Rust sources for secrets` detectó 2 coincidencias (`leaks found: 2`); los pasos posteriores se omitieron y el gate normativo corrió sin PostgreSQL → `exit=1` en lugar de `exit=2` |
| Coincidencias de gitleaks | Contraseñas sintéticas `synthetic-interop-vector-1/2` en `backend-rust/tools/credential-interop/run.mjs` (líneas 21–22). No son credenciales reales |
| Rulesets del repositorio | `[]` (ninguno) |
| Branch protection `dev` y `main` | `404 Branch not protected` en ambas |
| Tag protection | `404 Not Found` (no configurada o no disponible por API) |
| Rulesets organizacionales | No auditables con los scopes actuales (`admin:org` requerido) |
| Matriz de simulación 2G | `21/21` tests en `.github/scripts/test_ci_changes.py` y `19/19` en `test_verify_evidence.py`, ejecutados en Linux (contenedor) |
| Checks técnicos locales | `cargo fmt --check`, Clippy `-D warnings`, tests y build del workspace: PASS |
| Comandos comerciales | El registry Tauri registra 5 comandos de laboratorio/instalación; los 17 comandos de purchasing **no** están registrados |
| Migraciones candidatas | `backend-rust/proposals/` contiene `0002`–`0016`; **no existe `0017`**; el migrador de producto solo embebe `migrations/local/0001` |

Los tests de simulación no sustituyen un CI real: el único run remoto
disponible está rojo por la falsa positiva de gitleaks.

## 3. Diseño propuesto

### 3.1 Nivel 1 — `rust-technical` (integración técnica)

Job independiente, obligatorio cuando `rust_run=true`, en todo PR, push a `dev`
y push a `main`:

- Escaneo de secretos con Gitleaks 8.24.3 fijado por SHA-256 (mismo pin actual).
- `cargo fmt --all -- --check`.
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`.
- `cargo test --locked --workspace --all-features` (incluye las guardas de
  arquitectura de `pos-application` y las pruebas SQLite/SQLx del workspace).
- `cargo build --locked --workspace --all-features`.
- `cargo audit` 0.22.2 sobre `Cargo.lock` (bloqueante, sin `--ignore`).
- Verificación técnica de evidencia con `--expect-exit 0`.
- Guardas negativas de producto: que los comandos comerciales no estén
  registrados y que las migraciones candidatas no formen parte del migrador.
- Contratos Rust/TypeScript (hoy en `rust-contracts.yml`; se mantiene como gate
  requerido del Nivel 1).
- Seguridad Tauri: job propuesto sobre `apps/pos-desktop` (workspace propio) con
  fmt, Clippy, test y build. Requiere runner con dependencias de Tauri; si el
  coste no se acepta, debe declararse explícitamente como cobertura pendiente y
  no simularse.

Este job **no** ejecuta el laboratorio normativo y **no** puede quedar `skipped`
cuando es requerido.

### 3.2 Nivel 2 — `rust-normative` (activación y release)

Job independiente que ejecuta el gate normativo real:

1. Compila PostgreSQL 17.11 fijado (`build_postgres.sh`) en el propio job. No se
   comparte el binario entre jobs; si se usa caché, debe estar versionada por
   digest del script.
2. Ejecuta `migration-lab --require-complete` **sin modificar su semántica** y
   captura el código de salida explícitamente.
3. Ejecuta `verify_evidence.py --evidence target/migration-gate-evidence` con el
   exit esperado correspondiente (`2` si BLOCKED, `0` si PASSED).
4. Publica la evidencia como artefacto con `if: always()`.

Contrato de códigos de salida (sin `continue-on-error`, sin `|| true`):

| Exit del laboratorio | Estado | Step | Gate `dev` | Gate `main` |
| --- | --- | --- | --- | --- |
| `0` | PASSED | éxito | permitido | requerido |
| `2` | BLOCKED | éxito declarado | permitido (incompletitud declarada) | **rechazado** |
| `1` | FAILED | fallo | fallo | fallo |

Implementación sugerida del step (el exit se publica como output, no se oculta).
La versión aplicada vive en `.github/scripts/rust-normative-gate.sh` y coincide
con este contrato:

```bash
set +e
cargo run --locked -p migration-lab -- \
  --output target/migration-gate-evidence \
  --postgres-bin target/pg17-build/install/bin \
  --java-source --require-complete
code=$?
set -e
case "$code" in
  0) status=PASSED ;;
  2) status=BLOCKED ;;
  *) status=FAILED ;;
esac
echo "normative_status=$status" >> "$GITHUB_OUTPUT"
echo "normative_exit=$code" >> "$GITHUB_OUTPUT"
test "$status" != "FAILED"
```

El step siguiente verifica la evidencia con el `--expect-exit` derivado del
output. Un `FAILED` nunca se convierte en verde.

### 3.3 `ci-gate`

`ci-gate` sigue siendo el único required status check y evalúa según destino:

- **PR → `dev` y push a `dev`**: `changes=success`, `rust-technical=success`,
  `rust-normative` con estado `BLOCKED` o `PASSED` (nunca `FAILED`, `skipped` ni
  ausente), resto de jobs requeridos `success` o `skipped` según detección.
- **PR → `main`, push a `main` y `workflow_dispatch`**: todos los jobs técnicos en
  `success` y `rust-normative` en `PASSED` (exit 0). E2E obligatorio en `main` y
  en dispatch.
- Jobs requeridos ausentes por detección inválida: fallo, igual que hoy.
- `skipped` solo se acepta para jobs no requeridos por la detección.

### 3.4 Detección de cambios

`ci-changes.sh` se conserva. Se añaden salidas derivadas:

- `rust_technical_run` = `rust_run`.
- `rust_normative_required` = `true` para `main`/dispatch; `false` para PR y
  push a `dev` (se ejecuta igualmente para declarar BLOCKED, pero no bloquea por
  incompletitud).

## 4. Matriz de escenarios 2G

| # | Escenario | Resultado esperado del gate |
| --- | --- | --- |
| 1 | Rust válido e inactivo | Nivel 1 verde; Nivel 2 BLOCKED; PR a `dev` permitido |
| 2 | Error de compilación | Nivel 1 rojo; gate rojo |
| 3 | Test fallido | Nivel 1 rojo; gate rojo |
| 4 | Migración candidata promovida indebidamente | Guarda negativa roja; gate rojo |
| 5 | Comando Tauri comercial no autorizado | Guarda negativa roja; gate rojo |
| 6 | Activación criptográfica no aprobada | Nivel 2 BLOCKED; `main` rojo |
| 7 | Modificación de workflows | Detect Changes activa todos los jobs; contrato de workflow validado |
| 8 | Documentación solamente | Sin jobs ejecutables; gate verde |
| 9 | Normativa incompleta | Nivel 2 BLOCKED; `dev` permitido; `main` rojo |
| 10 | Normativa completa | Nivel 2 PASSED; `main` permitido si el resto pasa |
| 11 | PR a `main` con pendientes | Gate rojo aunque Nivel 1 esté verde |
| 12 | Fallo de generación/verificación de evidencia | Nivel 2 FAILED; gate rojo |

Los escenarios 1–12 se cubren en simulación por los tests existentes y por los
nuevos casos que deben añadirse al separar jobs. La validación real requiere CI
remoto autorizado sobre un SHA final.

## 5. Reglas y protecciones propuestas

No aplicar sin autorización. No habilitar bypass administrativo general.

Para `dev`:

- Pull request obligatorio.
- Required check: `CI Gate`.
- Review obligatorio.
- Historial protegido y force-push prohibido.
- Merge solo desde el proceso aprobado.

Para `main`:

- Pull request obligatorio desde `dev` (o `hotfix/*` aprobado).
- Required checks: `CI Gate`, `Rust Local Contracts`, E2E y Release Audit.
- Reviews obligatorias.
- Protección de tags de release.

El repositorio permite administración con el token actual, pero las reglas no
están configuradas. La API de rulesets organizacionales no es auditable con los
scopes disponibles; debe confirmarse con `admin:org`.

## 6. Resolución pendiente de la falsa positiva de gitleaks

`docs/05-development/ci.md` prohíbe explícitamente allowlists y excepciones.
Por tanto, cualquiera de estas opciones requiere aprobación del titular:

- **A. Anotación inline `// gitleaks:allow`** en las dos líneas sintéticas.
  Es precisa, pero es una excepción y contradice la política escrita.
- **B. Reemplazar los literales sintéticos por una expresión construida en
  tiempo de ejecución**, sin allowlist y sin debilitar el escáner. No cambia la
  semántica de la prueba.
- **C. Mantener el CI rojo** hasta decidir política y valores.

Recomendación: **B**, por conservar la política de «sin allowlists» y no ocultar
ninguna coincidencia real. En cualquier caso, la decisión debe quedar registrada.

Hallazgo adicional fuera del alcance bloqueante actual: gitleaks `generic-api-key`
reporta 9 coincidencias en `backend/src` (`application-dev-local.yml` y tests de
Java). El escaneo de CI solo cubre `backend-rust`; deben confirmarse como valores
de desarrollo/prueba antes de ampliar el alcance del escáner.

## 7. Autorizaciones

- [x] Aprobar la separación de controles técnicos y normativos.
- [x] Aprobar la modificación de los workflows y de sus tests de contrato.
- [x] Aprobar la resolución de la falsa positiva de gitleaks (opción B).
- [x] Preparar y publicar la rama consolidada (condicionado a recuperar 2E–2G).
- [ ] Aprobar la ejecución de CI remoto sobre el SHA final.
- [ ] Aprobar (por separado) el merge a `dev`.

La autorización de publicación o de CI no autoriza el merge.
