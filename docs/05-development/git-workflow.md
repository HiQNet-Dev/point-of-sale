# Flujo de trabajo con Git

## Rama `main`

- `main` es **estable**: siempre compila, con CI verde.
- **No se hace push directo a `main`** salvo casos justificados y comunicados.

**Excepción ya registrada:** el primer commit se hizo directo en `main` porque
el repositorio estaba vacío (no existía rama ni historial que proteger).
Mensaje: `chore: initialize HiQNet POS foundation`.

## Ramas

| Prefijo | Uso |
| --- | --- |
| `feat/*` | Funcionalidad nueva |
| `fix/*` | Corrección de defectos |
| `refactor/*` | Reestructuración sin cambio de comportamiento |
| `docs/*` | Solo documentación |

Ejemplos: `feat/catalog-products`, `fix/sale-total-rounding`, `docs/security-model`.

## Commits: Conventional Commits

```text
<tipo>: <descripción corta en imperativo>

feat: add branch stock endpoint
fix: correct payment rounding
chore: initialize HiQNet POS foundation
docs: document security model
```

Tipos usados: `feat`, `fix`, `refactor`, `docs`, `test`, `chore`, `ci`, `build`.
Sin force-push sobre ramas compartidas; sin reescribir historia publicada.

## Pull Requests

- **PR es el camino normal** de desarrollo, incluso para el autor principal.
- **CI obligatoria antes de merge** (backend + frontend + docker-compose):
  exactamente `./mvnw clean verify` y `pnpm lint/typecheck/test/build`.
- Merge solo con CI verde y revisión del diff.

## Higiene de Git

- `git diff --check` antes de commitear (sin whitespace basura).
- **No commitear archivos generados** (`target/`, `node_modules/`, `dist/`,
  lockfiles ajenos al gestor previsto — el `pnpm-lock.yaml` sí se versiona).
- **Nunca secretos**, credenciales ni `.env` reales en el historial.
- Diffs pequeños y enfocados; nada de cambios no relacionados en un PR.
