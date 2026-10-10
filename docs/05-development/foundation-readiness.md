# Foundation readiness — v0.1.0

**Propósito:** resumen de gates reales para cortar `v0.1.0-Foundation`. Cada
gate es `PASS`/`DEFERRED` (con motivo explícito). Documento vivo: se actualiza
en cada slice de hardening.

Release candidate / release: `0.1.0` (Foundation, V1–V8).

Gates técnicos = **PASS**; gate legal/distribución del repositorio = **PASS**
(software propietario, All Rights Reserved, `LICENSE` presente, repo privado).

## Gates

| Área | Estado | Evidencia |
| --- | --- | --- |
| Seguridad — deny-by-default | PASS | `SecurityConfig.anyRequest().denyAll()`; matchers explícitos por ruta; sin `/api/v1/**` blanket |
| Seguridad — superficie pública | PASS | health/error + auth público + docs bajo flag + `POST /bootstrap` solo con secret (filtro) |
| Seguridad — `POST /organizations` cerrado | PASS | sin matcher ni permiso; tests (SecurityDeny, BootstrapApi, UnsupportedMethod) |
| Seguridad — bootstrap disabled | PASS | sin `BOOTSTRAP_SECRET` la ruta cae en denyAll; `BootstrapDisabledTest`, `FoundationProdProfileTest` |
| Seguridad — CORS sin wildcard | PASS | `CorsConfig.parseOrigins` rechaza `*`/patrones en startup + tests |
| Seguridad — secretos fail-fast | PASS | `AUTH_JWT_SECRET`/`BOOTSTRAP_SECRET`: Base64 ≥32B, sin default, fallo de arranque; tests |
| Seguridad — sin secretos en logs/errores | PASS | scans §20/§21; `ApiError` sin internals; ConstraintTranslator estructurado |
| Seguridad — authorization por permisos | PASS | todo endpoint protegido con `@PreAuthorize` validado contra catálogo (19 códigos, todos usados) |
| Seguridad — scopes org/branch | PASS | `hasPermission` (org) vs `hasBranchPermission` (org ∪ branch ACTIVE); anti-escalación probado |
| Base de datos — Flyway V1–V8 | PASS | clean install + upgrade `V7→V8` en PostgreSQL real (`FlywayMigrationPathTest`); V1–V8 inmutables |
| Base de datos — `ddl-auto` | PASS | `validate` en todos los profiles; Flyway única autoridad; sin `update/create` |
| Base de datos — constraints | PASS | FKs compuestas same-org, uniques tenant, CHECKs (status/slug/singleton/append-only) |
| Auth — JWT | PASS | identity-only (sin claims RBAC); validators: firma/exp/issuer/audience/`token_type`/UUIDs |
| Auth — refresh lifecycle | PASS | rotación, familia 30d, logout/logout-all, reuse detection, carrera concurrente, lock `user→session` |
| Auth — deactivación | PASS | revocación de sesiones + audit `user.deactivated` en la misma transacción |
| RBAC — database-authoritative | PASS | immediacy con el mismo JWT (permisos/roles/estados); sin cache, sin securityVersion |
| RBAC — rol `administrator` no mágico | PASS | rol normal; quitar permiso/desactivar → 403; sin bypass por code |
| Audit — append-only | PASS | trigger DB (UPDATE/DELETE rechazados por JDBC), repo sin delete, `@Immutable`, sin API de mutación |
| Audit — metadata | PASS | keys sensibles recursivo, inmutabilidad profunda, ≤16 KiB, sin POJOs |
| Audit — transaccional | PASS | `MANDATORY`, misma transacción; fallo de audit → rollback (test) |
| Bootstrap — one-shot/race | PASS | singleton + `FOR UPDATE`; 1 ganador concurrente; completed → 409; audit SYSTEM |
| Config — prod | PASS | DB/JWT requeridos; OpenAPI cerrado; CORS vacío; `FoundationProdProfileTest` |
| Config — CORS dev | PASS | allowlist `http://localhost:5173`; wildcard imposible |
| CI | PASS | permisos mínimos (`contents: read`); backend+frontend+compose verdes en main |
| Docs | PASS | roadmap/módulos/seguridad/API alineados; README con Foundation y `BOOTSTRAP_SECRET` |
| Legal/distribución | PASS | Propietario / All Rights Reserved; `LICENSE` presente; repo privado; acuerdos comerciales separados para clientes |
| Versiones | PASS | `VERSION`=fuente canónica; backend `0.1.0`; frontend sincronizado (`0.1.0`) |
| Tests | PASS | `./mvnw clean verify` (361 tests) + frontend `lint/typecheck/test/build` + Compose |
| Rendimiento | PASS | sin N+1 (dominio sin asociaciones; checks `EXISTS` únicos), paginación acotada, índices críticos presentes |

## Deferred (no bloquean v0.1.0 — fuera del alcance Foundation)

| Pendiente | Motivo / milestone |
| --- | --- |
| Ventas, catálogo, inventario, cajas, compras, reportes, hardware, frontend admin UI | dominios futuros (v0.2+), no prometidos en Foundation |
| Control Plane HiQNet (customers/licenses/billing/deployments) | arquitectura separada |
| Telemetría de autenticación unificada en `audit_events` | solo si aparece el requerimiento |
| Audit retention/archival | política pendiente por diseño (sin volúmenes reales) |
| Device/terminal identity, client IP | requieren dominio hardware y topología de proxies |
| Rate limiting (edge/gateway) | defensa futura por topología |
| Password recovery / root recovery | procedimiento de soporte futuro (sin backdoor) |
| Backup/restore automatizado | gate operativo del primer piloto (v0.8 Pilot) |
| Listado filtrado "mis sucursales" | RESUELTO en v0.2.0: `GET /api/v1/auth/me/branches` (self-scoped, paginado) |
| EULA / términos de suscripción / servicio / SLA para clientes | precomercial; requerido antes del primer despliegue o piloto con cliente externo |

## Release blockers v0.1.0

No remaining technical or repository-licensing blockers.

Sin blockers técnicos ni de licenciamiento del repositorio: CI verde; sin
BLOCKER/HIGH de seguridad; migraciones e upgrade validados; sin wildcard CORS;
sin credenciales por defecto; sin fugas cross-org; bootstrap one-shot; modelo
propietario aprobado (`LICENSE` presente, repo privado).

Requisito FUTURO antes del primer cliente externo (NO bloquea la release
técnica privada `v0.1.0`): **términos comerciales hacia el cliente**
(EULA/contrato de servicio/suscripción), a definir por HiQNet.