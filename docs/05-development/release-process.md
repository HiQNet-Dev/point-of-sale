# Proceso de release

**Estado: no existe ninguna release ni tag.** Versión en desarrollo:
`0.1.0-SNAPSHOT` (archivo `VERSION` y `pom.xml`).

## Versionado: SemVer

- **`v0.x.x`** — desarrollo. El 0.x comunica inestabilidad: el alcance puede
  cambiar sin romper promesas.
- **`v1.0.0`** — primera release comercial estable. Hoy **no está alcanzada**.
- **Precursoras:** `0.1.0-SNAPSHOT` y derivados `-SNAPSHOT` marcan versiones en
  construcción; no se publican como release.

Regla SemVer una vez en 1.x: `MAJOR` rompe compatibilidad, `MINOR` agrega
funcionalidad compatible, `PATCH` corrige.

## Hitos de versión

- El **tag `v0.1.0` se creará únicamente cuando el milestone Foundation
  esté completo** (ver `docs/00-product/roadmap.md`). Hoy no se ha creado.
- Ninguna otra versión se etiqueta hasta que su milestone correspondiente
  esté completado y validado.

## Pasos de release (futuros / esquema, no implementado)

Cuando exista al menos un milestone completo:

1. CI verde en `main` (backend `./mvnw clean verify`, frontend
   `pnpm lint/typecheck/test/build`).
2. Actualizar `VERSION` y changelog derivado de **Conventional Commits**.
3. Tag `vX.Y.Z` firmado/evidente en Git.
4. Generar artefactos (JAR backend, build estático frontend) — **futuro**;
   hoy no existe pipeline de artefactos ni publicación.
5. Notas de release revisadas por HiQNet.

## Hoy

- **Cero releases publicadas.**
- **Cero tags.**
- Producto **no comercialmente estable**; sin garantías de compatibilidad entre
  snapshots.
- **Licencia pendiente de definición:** ninguna licencia se concede con estas
  versiones.
