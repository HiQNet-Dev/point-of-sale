# AGENTS.md — Reglas del repositorio HiQNet POS para agentes

Reglas repo-local para agentes automatizados que trabajan en este repositorio.
Complementan el flujo humano en `docs/05-development/git-workflow.md` y
`CONTRIBUTING.md`.

## Branching

- `main` es la rama estable/release: no se desarrolla directamente en `main`.
- `dev` es la rama permanente de integración/preproducción de la próxima release.
  Las features normales nacen de `dev` y se integran en `dev`.
- Ramas short-lived con prefijo de enfoque (`feat/backend/*`, `feat/frontend/*`,
  `feat/fullstack/*`, `fix/*`, `test/*`, `chore/*`, `docs/*`, `refactor/*`) creadas
  desde `origin/dev`; PR hacia `dev`.
- Release: `dev` → `main` solo tras la Release Audit y con CI verde.
- Hotfix: `hotfix/*` nace de `main`, vuelve a `main` y luego se sincroniza a `dev`.
- No crear ramas principales paralelas (`backend`, `frontend`, `develop`, `docker`,
  `integration`).

## Worktrees

- Trabaja directamente en el checkout principal con `git switch` y ramas
  short-lived.
- No uses worktrees por defecto. Solo crea uno cuando necesites mantener dos
  checkouts simultáneos y documenta la razón en el PR.
- Nunca commitees `.worktrees/` (es local y está en `.gitignore`).

## Pull requests

- Un PR, un propósito coherente (no mezclar Docker, features de UI, migraciones de
  schema y limpieza aleatoria).
- Los PR apuntan a `dev`; los `hotfix/*` apuntan a `main`.
- No commitear ni hacer push directo a `main` o `dev` (salvo tareas explícitas de
  release o sincronización controlada), ni force-push, ni reescribir historia
  publicada.
- Nunca commitear secretos ni `.env` reales.

## Contrato de API y documentación

- La especificación OpenAPI (`/v3/api-docs`; Swagger UI solo en `dev` con
  `app.security.permit-opendoc=true`) es la **fuente de verdad** del contrato
  HTTP.
- Todo cambio de contrato de API (endpoint, request, response, código de estado,
  permiso o código de error estable) debe actualizar OpenAPI **en el mismo
  cambio**: anotaciones (`@Operation`, `@Schema`, `@ApiResponse`) y los
  documentos de `docs/03-api` que apliquen.
- La forma del error (`ApiError`) se genera una sola vez en `OpenApiConfig`
  (`apiErrorSchemaCustomizer`); no dupliques el schema a mano.
- Un PR que cambia el contrato sin actualizar la especificación y su
  documentación es un cambio incompleto.

## Onboarding incremental para agentes

Estas reglas aplican a agentes de Zed y OpenCode. Reducen lecturas repetidas sin
sustituir las fuentes de verdad.

1. **Git primero.** Antes de editar, revisa `git status` y el diff de los archivos
   que vas a tocar. Conserva cambios y configuraciones existentes.
2. **Mapa antes que exploración.** Empieza por
   `docs/agent-context/PROJECT_MAP.md`; es el índice de rutas y fuentes de
   verdad. No recorras el repositorio completo en cada sesión.
3. **Búsqueda dirigida.** Localiza símbolos con LSP/Serena y lee sólo la
   implementación, las interfaces y las pruebas relacionadas. Amplía la búsqueda
   únicamente si falta evidencia.
4. **No re-descubras.** Reutiliza el mapa y los documentos canónicos; no releas
   todo el repo ni los documentos completos si el mapa ya apunta a la sección
   necesaria.
5. **Mapa mantenible.** Actualiza `docs/agent-context/PROJECT_MAP.md` sólo cuando
   cambien de forma estable la arquitectura, los comandos o las fuentes de
   verdad. No lo conviertas en bitácora ni en copia del repositorio.
6. **Salida concisa.** Entrega primero un plan breve y, al terminar, un resumen
   de archivos, validaciones y riesgos. Sin razonamiento interno extenso.
7. **Configuración de agentes.** Las reglas compartidas viven aquí; las
   instrucciones reutilizables de tarea viven en Skills
   (`.agents/skills/` o la Skill global). Los perfiles de Zed sólo controlan
   herramientas/modelo.

Planes por defecto: OpenCode usa los agentes de `.opencode/agents/`; Zed usa los
perfiles de `.zed/settings.json`. Ambos comparten estos roles: `planning`,
`implementation`, `testing`, `review`, `efficient`.
