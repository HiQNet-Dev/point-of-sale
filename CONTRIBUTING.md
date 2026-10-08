# Cómo contribuir a HiQNet POS

Gracias por contribuir. Reglas cortas, mantenemos el repo limpio:

## Flujo de trabajo

1. Crea una rama desde `main` con prefijo descriptivo:

   - `feat/*` — nueva funcionalidad
   - `fix/*` — corrección de errores
   - `refactor/*` — refactor sin cambio de comportamiento
   - `docs/*` — documentación

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