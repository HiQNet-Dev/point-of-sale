# Proceso de release

**Estado: release técnica privada publicada — `v0.3.0` (Inventory,
2026-10-14).** La release se valida sobre el corte de `dev` y añade Inventory
sobre `v0.2.0` (POS Core). La secuencia sigue el proceso verificado previamente:
commit dedicado, CI verde, tag anotado y GitHub Release con notas revisadas.

## Versionado: SemVer

- **`0.x.y`** — desarrollo. El 0.x comunica inestabilidad: el alcance puede
  cambiar sin romper promesas. `0.1.0-SNAPSHOT` y derivados `-SNAPSHOT` marcan
  versiones en construcción; no se publican como release.
- **`v1.0.0`** — primera release comercial estable. Hoy **no está alcanzada**.
- Regla SemVer en 1.x: `MAJOR` rompe compatibilidad, `MINOR` agrega
  funcionalidad compatible, `PATCH` corrige (rollback/corrección).

## Fuente canónica de versión

1. `VERSION` (raíz) = **fuente de verdad de release**.
2. `backend/pom.xml` = debe coincidir con `VERSION`.
3. `frontend/package.json` = paquete privado; se sincroniza con `VERSION` para
   releases y snapshots; no define la versión de release.

En un release, el bump se hace en el ORDEN: `VERSION` → `pom.xml` →
`package.json` (mismo valor), en un commit dedicado (`chore(release): vX.Y.Z`).

## Release gate v0.1.0 (checklist de autorización)

Debe cumplirse ANTES de publicar:

- [ ] CI verde en `main` (backend `./mvnw clean verify` — 361 tests con
      Testcontainers reales; frontend `pnpm lint/typecheck/test/build`; Compose).
- [ ] `docs/05-development/foundation-readiness.md` sin **release blockers**
      (solo DEFERRED con milestone explícito).
- [ ] Code review de los módulos Foundation sin BLOCKER/HIGH sin resolver.
- [ ] Migraciones `V1`–`V8` inmutables; clean install y upgrade `V7→V8`
      validados por tests.
- [ ] Smoke del perfil `prod` (`FoundationProdProfileTest`) verde.
- [x] **Modelo de distribución propietario aprobado** (HiQNet = propietario /
      All Rights Reserved; sin licencia open-source).
- [x] **`LICENSE`** (aviso propietario) presente.
- [x] **Repositorio privado**.

**`v0.1.0` = release técnica privada de la Foundation** — un hito técnico, NO
una release comercial, NO un producto "production-ready" para clientes. La
primera **release comercial estable** sigue siendo `v1.0.0`. La publicación de
artefactos externos para clientes continuará requiriendo los **contratos
comerciales** correspondientes (EULA/acuerdos de servicio, futuros).

## Pasos de release (cuando HiQNet lo autorice)

1. **Marco legal ya resuelto** para el repositorio: distribución propietaria /
   All Rights Reserved, `LICENSE` presente, repo privado
   (`docs/05-development/legal-status.md`). Los términos comerciales hacia el
   cliente (EULA/servicio/suscripción) se definen aparte antes del primer
   piloto/cliente externo.
2. **Bump**: `VERSION` → versión final del release (+ `pom.xml`, `package.json` sincronizados).
3. **CI completo** sobre el commit de bump (backend+frontend+compose verdes).
4. **Changelog / release notes**: derivar de Conventional Commits desde el
   último punto de referencia; revisión humana de HiQNet.
5. **Tag anotado `vX.Y.Z`** en Git, apuntando al commit de bump validado.
6. **GitHub Release** con las notas aprobadas.
7. **Artefactos**: política mínima para esta fundación — fuente = tag +
   `backend/target/hiqnet-pos-backend-0.1.0.jar` del build validado si el
   workflow de release lo justifica. No existe pipeline de imágenes OCI, npm
   package ni instaladores todavía (nada que inventar).
8. **Post-release**: el ciclo posterior a `v0.3.0` ya está abierto como
   `0.4.0-SNAPSHOT` en `VERSION`, `pom.xml` y `package.json`. La sincronización
   `dev = main` posterior no cambia la versión.

## Rollback / corrección

- Bug en una release publicada: rama `fix/*` desde el **tag**, PR a `main` con
  CI verde, `PATCH` bump (`v0.1.1`) siguiendo los mismos pasos. Si el artefacto
  es inmutable (tag/JAR): publicar el parche como nueva release.
- Nunca reescribir tags/`Release`s publicados; nunca editar migraciones
  publicadas `V1`–`V8` (los fixes de schema son `V9+`).

## Hoy

- **Última release: `v0.3.0`** (Inventory, release técnica privada). El ciclo
  de desarrollo actual es **`0.4.0-SNAPSHOT`** para Purchasing.
- Producto **no comercialmente estable**; sin garantías entre snapshots.
- **Distribución legal del repositorio resuelta**: propietario / All Rights
  Reserved, `LICENSE` presente, repo privado (no open-source).
- Sin pipeline externo de publicación (imágenes, paquetes, instaladores); los
  artefactos binarios para clientes requerirán contratos comerciales y política
  de distribución propia.