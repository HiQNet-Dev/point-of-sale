# Cómo contribuir a HiQNet POS

Gracias por contribuir. Reglas cortas, mantenemos el repo limpio:

## Flujo de trabajo

1. Crea una rama **short-lived** desde el `origin/main` actual, con prefijo por
   enfoque:

   - `feat/backend/*`, `feat/frontend/*`, `feat/fullstack/*` — funcionalidad
   - `fix/backend/*`, `fix/frontend/*` — corrección de errores
   - `chore/dev/*`, `chore/ci/*`, `chore/*` — tooling, CI, mantenimiento
   - `docs/*` — documentación
   - `test/*` — pruebas
   - `refactor/*` — refactor sin cambio de comportamiento

   **No** se crean ramas principales paralelas (`backend`, `frontend`,
   `develop`, `docker`, `integration`): `main` es la única rama canónica y los
   enfoques se separan con prefijos. Detalle en
   `docs/05-development/git-workflow.md`.

2. Trabaja en cambios pequeños y enfocados. Un PR, un propósito.

3. Commits con **Conventional Commits**:

   ```text
   feat(sales): add ticket payment flow
   fix(catalog): validate variant price
   chore: update dependencies
   ```

4. Abre un Pull Request hacia `main`. CI debe pasar antes del merge
   (backend: compile/test/package; frontend: lint/typecheck/test/build;
   docker-compose validado).

5. Nunca hagas push directo a `main` (excepto casos justificados aprobados).

## Antes de un PR

- `git diff --check` sin errores.
- Preferentemente un solo commit por cambio (usa `rebase`/`squash` — escribe con
  `git status` y `git diff` para revisar antes de subir).
- Sin secretos en el diff (`.env`, tokens, credenciales van a `.gitignore`).
- Como máximo **un worktree de feature activo**; tras el merge elimínalo junto
  con su branch local (ver `docs/05-development/git-workflow.md`).
- Pruebas para reglas críticas. Backend: `./mvnw clean verify`. Frontend:
  `pnpm lint && pnpm typecheck && pnpm test && pnpm build`.

## Convenciones de código

Ver `docs/05-development/coding-guidelines.md` (obligatorio para cambios de
dominio financiero/transaccional):

- Dinero siempre con `BigDecimal`.
- Operaciones monetarias transaccionales; sin hard delete de operaciones financieras.
- Schema solo vía Flyway; nunca `ddl-auto=update`.
- Sin lógica de negocio en controllers; sin lógica crítica en React.
- Validación y autorización siempre del lado servidor.

## Estado del repositorio

`main` es la rama estable. El primer commit de fundación fue directo a `main`
porque el repositorio estaba vacío; a partir de ahora el desarrollo normal usa PRs.