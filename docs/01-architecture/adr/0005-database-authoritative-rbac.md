# ADR-0005: Autorización database-authoritative — JWT identity-only

## Status

Accepted (validado por tests end-to-end con JWTs reales en el slice RBAC;
véase `docs/04-security/rbac.md`).

## Context

Hasta el slice RBAC, las business APIs (`/api/v1/organizations/**`, branches,
users admin) estaban **deny-by-default** incluso con un access JWT válido. Al
introducir Roles/Permissions hay que decidir cómo se modela la autorización:
dónde viven los roles/permisos, si viajan o no en el JWT, y cómo se refleja una
revocación/desactivación en peticiones que llevan un JWT stateless.

Estado previo que enmarca la decisión:

- Access JWT **stateless, 10m**, claims de identidad únicamente
  (`sub`, `org_id`, `token_type`, `iss`, `aud`, `iat`, `exp`, `jti`). Invalidado
  criptográficamente solo por `exp`; sin blacklist.
- PostgreSQL ya es la autoridad de sesiones (`refresh_sessions`) y de identidad
  operativa (`/me` resuelve User + Organization desde DB).
- Despliegue por cliente (una instalación, típicamente una instancia); escala
  modesta; operaciones transaccionales fuertemente consistentes.

## Decision

1. **JWT = identidad, PostgreSQL = autoridad de autorización.** Los permisos
   efectivos se consultan desde la base en cada petición
   (`AuthorizationService.hasPermission` → query `EXISTS` con user ACTIVE +
   org ACTIVE + rol ACTIVE + rol asignado + rol contiene el permiso).
2. **Roles y permissions NO viajan en el JWT** (sin claims `roles`,
   `permissions`, ni flags de admin). El mismo JWT refleja inmediatamente
   cualquier cambio de DB (remover permiso, quitar rol, desactivar rol,
   INACTIVE de user/org) — verificado por tests con el MISMO access token.
3. **Sin `securityVersion`/security stamp/blacklist por ahora.** Con
   resolución-DB por petición no aportan valor a esta escala; si en el futuro
   hay cache/Redis/verificación offline, se reabre este ADR con datos medidos.
4. **Method security** activada explícitamente (`@EnableMethodSecurity` +
   `@PreAuthorize("@authorizationService.hasPermission(...)")` con **permission
   codes**, no roles hardcodeados ni `hasRole("ADMIN")`).
5. **Sin `SUPER_ADMIN`/`ROOT`/`HIQNET_ADMIN`** dentro del POS de clientes: la
   administración de HiQNet (bootstrap, Control Plane) es un sistema separado.
6. **`POST /api/v1/organizations` permanece cerrado** (no existe el permiso
   `organization.create`): crear organizaciones pertenece al secure bootstrap /
   futuro Control Plane.
7. **Sin cache de permisos ni claims RBAC** en esta fundación.

## Consequences

- Revocación **inmediata**: quitar un permiso/rol o desactivar rol/user/org
  deja de autorizar en la siguiente petición con el mismo JWT (tests e2e).
- Permisos efectivos = **UNION** de los permisos de los roles activos asignados
  al usuario en la organización; sin deny rules, jerarquías ni wildcards.
- Roles `INACTIVE` no conceden nada mientras sus assignments permanecen en DB;
  al volver `ACTIVE` vuelven a conceder.
- Cada petición protegida hace 1 query `EXISTS` (sin N+1, sin cache).
- `SecurityConfig` matchea rutas administrativas concretas a `authenticated()`
  y deja `anyRequest().denyAll()`; la autorización fina vive en method security.
- 401 vs 403: sin token → 401; token sin el permiso (o identidad/org no
  operativa) → 403 `FORBIDDEN`; violación de scope de autorización (otra
  organización) → 403; aislamiento de recurso (id ajeno) → 404.
- Si algún día se necesitan verificaciones offline o caché de autorización,
  este ADR se reabre con el driver medido que lo justifique.