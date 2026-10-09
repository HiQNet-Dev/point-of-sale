# Branch Memberships — modelo de datos

**Implementado en `V6__create_branch_memberships.sql`** (PR: Branch
Memberships). Complementa `docs/02-database/users-model.md` y
`docs/04-security/rbac.md` (scopes de autorización).

## Propósito

Las asignaciones de rol de `user_organization_roles` son **organization-wide**:
un rol vale para toda la organización. Para poder expresar:

```text
Ana
├── Manager @ Branch Centro
└── Cashier @ Branch Zapopan

Gael
└── Organization Administrator   (org-wide)
```

se introduce una **membership explícita** entre User y Branch con sus propios
roles branch-scoped.

## Modelo

```text
branch_memberships
------------------
id              UUID     PK
organization_id UUID
branch_id       UUID
user_id         UUID
status          VARCHAR  ACTIVE | INACTIVE   (default ACTIVE)
version         BIGINT   @Version (optimistic locking)
created_at / updated_at TIMESTAMPTZ
UNIQUE (organization_id, branch_id, user_id)   -- UNA membership por user+branch
UNIQUE (id, organization_id)                  -- superkey para FKs compuestas

branch_membership_roles
-----------------------
organization_id UUID
membership_id   UUID
role_id         UUID
created_at      TIMESTAMPTZ
PK (organization_id, membership_id, role_id)
```

Los roles **reutilizan** el catálogo de `roles` (definiciones scoped por
organización). **No** existe un modelo `BranchRole` separado.

## Restricciones de integridad (PostgreSQL es la autoridad)

| Violación | Cómo la impide PostgreSQL |
| --- | --- |
| Membership Org A + Branch Org B | FK compuesta `(branch_id, organization_id) → branches(id, organization_id)` (superkey `branches_id_organization_unique` añadida en `V6`) |
| Membership Org A + User Org B | FK compuesta `(user_id, organization_id) → users(id, organization_id)` (superkey de `V5`) |
| Membership Org A + Role Org B | FK compuesta `(role_id, organization_id) → roles(id, organization_id)` (superkey de `V5`) |
| Membership duplicada | `UNIQUE (organization_id, branch_id, user_id)` |

El pre-check de Java (`branch.membership.*`) es UX; la carrera la cierra el
constraint traducido por `ConstraintTranslator`
(`branch_memberships_org_branch_user_unique` → `409 BRANCH_MEMBERSHIP_ALREADY_EXISTS`).

## Semántica de `status`

- **ACTIVE** — los roles de la membership pueden conceder permisos **solo para
  esa branch** (mientras Role ACTIVE y Branch ACTIVE).
- **INACTIVE** — no concede ningún branch permission, pero conserva la
  configuración (branch, user y roles asignados).
- Al volver **ACTIVE**, los roles vuelven a aplicar automáticamente.

No hay DELETE: inactivar es la forma de «quitar».

## Authorization branch-scoped

```text
hasBranchPermission(user, org, branch, perm)
  = organización-access (org-wide role, override)
    UNION
    membership branch-specific (membership ACTIVE + role ACTIVE + branch ACTIVE)
```

La parte branch-specific se evalúa en **una sola consulta `EXISTS`**
(`BranchMembershipRoleRepository.existsEffectiveBranchPermission`) que exige:
User ACTIVE, Organization ACTIVE, **Branch ACTIVE** y de la misma org,
Membership ACTIVE de ESA branch, Role ACTIVE de la misma org y asignado a la
membership, y el permiso en el role. Sin N+1, sin caché, sin claims en el JWT.

La parte organización-override no depende de la branch concreta: una branch
ajena bajo el propio org se resuelve en el service con `404 BRANCH_NOT_FOUND`
(contrato de aislamiento cross-org).

## Gestión vs. uso

La **administración** de memberships (crear, listar, patch, roles) se autoriza
siempre con permisos **organization-wide** (`branch.membership.read/.create/
.update/.roles.manage`). Un rol branch-scoped **jamás** administra memberships
o endpoints organization-wide (anti privilege-escalation, probado con tests).

## Endpoints

Véase `docs/03-api/api-guidelines.md` (§ Branch Memberships):
`POST/GET /…/branches/{branchId}/memberships`, `GET/PATCH
/…/memberships/{userId}`, `GET/PUT /…/memberships/{userId}/roles`.

## Sin invalidación de sesiones

Cambiar memberships/roles/status NO revoca refresh sessions: la autorización
es DB-authoritative y el mismo JWT refleja el cambio en el siguiente request
(ADR-0005, `docs/01-architecture/adr/0005-database-authoritative-rbac.md`).