# deploy/

Estrategia de despliegue — **documentación por ahora**, sin implementación.

## Estado actual

- `docker-compose.yml` levanta el stack local de desarrollo completo
  (PostgreSQL + backend Spring Boot + frontend Vite con HMR). Ver
  `../docs/05-development/local-development.md`.
- Los **fixtures de desarrollo** se cargan con `pnpm dev:seed` y se reinician de
  forma determinista con `pnpm dev:reset` (solo desarrollo; nunca contra datos
  reales).
- `deploy/` describe la **estrategia de despliegue a producción**, todavía
  documentación sin implementación.
- **No hay despliegue a producción configurado** y CI no despliega nada.

## Intención futura

- Validar candidatos a release con `./mvnw clean verify` y `pnpm build` (CI).
- Empaquetar el backend como imagen OCI (JRE 21 + `hiqnet-pos-backend-*.jar`) y el
  frontend como artefacto estático servido tras un reverse proxy.
- PostgreSQL gestionado por el operador del entorno; **Flyway es el único mecanismo
  de evolución de esquema** — nunca `ddl-auto=update` en producción.
- Preferimos **instalaciones aisladas por cliente** antes de complejidad multi-tenant
  (ver `../docs/01-architecture/architecture.md`, sección HiQNet Control Plane).
- La aplicación de caja (Electron) se documentará en `../docs/01-architecture/architecture.md`
  cuando se implemente hardware POS.

Nada de esto está construido todavía: son directrices para no contradecir la arquitectura.