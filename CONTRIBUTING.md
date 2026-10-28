# Cómo contribuir a HiQNet POS

¿Es tu primera vez? Empieza por la guía visual de onboarding:
[`docs/05-development/collaborator-onboarding.md`](docs/05-development/collaborator-onboarding.md).
Este documento sigue siendo la referencia de las reglas formales de contribución.

Gracias por contribuir. Reglas cortas, mantenemos el repo limpio:

## Flujo de trabajo

1. Crea una rama **short-lived** desde el `origin/dev` actual, con prefijo por
   enfoque:

   - `feat/backend/*`, `feat/frontend/*`, `feat/fullstack/*` — funcionalidad
   - `fix/backend/*`, `fix/frontend/*` — corrección de errores
   - `chore/dev/*`, `chore/ci/*`, `chore/*` — tooling, CI, mantenimiento
   - `docs/*` — documentación
   - `test/*` — pruebas
   - `refactor/*` — refactor sin cambio de comportamiento
   - `hotfix/*` — corrección urgente de la versión estable (nace de `main`)

   **No** se crean ramas principales paralelas (`backend`, `frontend`,
   `develop`, `docker`, `integration`). `dev` es la rama de integración y `main`
   la estable/release. El detalle está en `docs/05-development/git-workflow.md`.

2. Trabaja en cambios pequeños y enfocados. Un PR, un propósito.

3. Commits con **Conventional Commits**:

   ```text
   feat(sales): add ticket payment flow
   fix(catalog): validate variant price
   chore: update dependencies
   ```

4. Abre un Pull Request hacia `dev` (los `hotfix/*` hacia `main`). CI debe pasar
   antes del merge (backend: compile/test/package; frontend:
   lint/typecheck/test/build; docker-compose validado). El CI general valida
   código limpio e integración solo en las áreas tocadas y **no** corre el E2E;
   `main` corre el CI más robusto con E2E. Detalle en
   `docs/05-development/ci.md`.

5. Nunca hagas push directo a `main` ni a `dev` (excepto tareas explícitas de
   release o sincronización controlada).

## Antes de un PR

- `git diff --check` sin errores.
- Preferentemente un solo commit por cambio (usa `rebase`/`squash` — escribe con
  `git status` y `git diff` para revisar antes de subir).
- Sin secretos en el diff (`.env`, tokens, credenciales van a `.gitignore`).
- Trabaja en el checkout principal con ramas short-lived; **no uses worktrees por
  defecto** (solo con justificación documentada). Tras el merge elimina la rama
  local (y el worktree, si lo hubo).
- Pruebas para reglas críticas. Backend: `./mvnw clean verify`. Frontend:
  `pnpm lint && pnpm typecheck && pnpm test && pnpm build`.

## Convenciones de código

Ver `docs/05-development/coding-guidelines.md` (obligatorio para cambios de
dominio financiero/transaccional):

- Dinero Java con `BigDecimal`; en Rust, `MoneyMinor` exacto y operandos decimales sin float, según F4–21. No redondear históricos silenciosamente.
- Operaciones monetarias transaccionales; sin hard delete de operaciones financieras.
- Java/PostgreSQL legado: schema solo vía Flyway; nunca `ddl-auto=update`. Nuevos contextos Rust: SQLx con roots Local/Central/Control independientes (F4–18). No editar V1–V25 ni convertir historiales.
- Sin lógica de negocio en controllers; sin lógica crítica en React.
- Validación y autorización siempre del lado servidor.
- Contrato de API: si cambias un endpoint, request, response, código de estado o
  código de error, actualiza la especificación OpenAPI (`/v3/api-docs`) en el
  **mismo** cambio — anotaciones (`@Operation`/`@Schema`/`@ApiResponse`) y
  `docs/03-api`. Un contrato sin su doc es un cambio incompleto.

## Foundation Rust (parcial, sin datos productivos)

La guía reproducible es [`backend-rust/README.md`](backend-rust/README.md).
Desde `backend-rust/`: `cargo fmt --all -- --check`,
`cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`,
`cargo test --locked --workspace --all-features` y
`cargo build --locked --workspace --all-features`. El primer esquema Local
requiere IMP-01/DDL13-01; el gate `--require-complete` permanece bloqueado mientras
los casos obligatorios no estén implementados y ejecutados. No omitirlos para
obtener CI verde. Rust no sustituye Java, React ni su OpenAPI.

## Estado del repositorio

`main` es la rama estable/release. `dev` es la rama de integración y
preproducción de la próxima release. El primer commit de fundación fue directo a
`main` porque el repositorio estaba vacío; a partir de ahora el desarrollo normal
nace de `dev` y usa PRs.
