# ADR-0030 — Administración local de identidad: usuarios, roles y membresías

**ACEPTADA para la release técnica v0.4.0.** Define cómo se administran usuarios,
roles y accesos por sucursal dentro de Free Local, con autorización
DB-authoritative, control de concurrencia optimista y auditoría transaccional.

## Contexto

ADR-0026 promovió el esquema `0002`–`0017`; ADR-0027 activó el login comercial y
ADR-0028 la primera instalación. El bootstrap crea un administrador con un rol
que reúne los 59 permisos, y hasta este incremento esa era la **única** identidad
posible.

Dos consecuencias bloqueaban el resto del producto:

1. No existía forma de crear un segundo usuario, asignarle un rol ni retirarle
   permisos desde el producto.
2. `LocalPrincipalService` resuelve la pierna de sucursal **solo** desde
   `branch_membership` + `branch_membership_role` + rol ACTIVO. Sin administración
   de membresías, cualquier principal con sucursal seleccionada obtenía cero
   permisos y todos los módulos operativos —caja, ventas, inventario— quedaban
   denegados por diseño. El bloqueo no era de interfaz: era de modelo de datos.

La alternativa de conceder permisos de sucursal desde el rol de organización se
descartó: elimina la separación de alcance que el backend Java mantiene y
convertiría un permiso de sucursal en un permiso global.

## Decisión

### 1. Dos agregados nuevos en el dominio

- `pos_core::identity::Role`: identidad, organización, código inmutable único por
  organización, nombre visible, estado y revisión. Los permisos no viven en el
  agregado, sino en la tabla de unión `role_permission`; el agregado conserva la
  revisión que protege la operación completa (`Role::touch`).
- `pos_core::identity::BranchMembership`: identidad, organización, sucursal,
  usuario, estado y revisión. Organización, sucursal y usuario son inmutables: una
  membresía se crea y se activa o desactiva, nunca se muda.

Ambos reutilizan `Status`, `BusinessCode`, `DisplayName` y `EntityId` del dominio
Organization/Branch. No se añade ningún value object redundante.

### 2. Composición asíncrona en `pos-application::administration`

Igual que la configuración del negocio (ADR-0026 e incremento posterior), la
composición es asíncrona sobre `SqliteUnitOfWork`: `UnitOfWork::transaction(fn)`
síncrono no puede alojar `SqliteUnitOfWork::begin` sin bloquear el runtime, y la
forma previa de auditoría no admite la atribución canónica USER/SYSTEM.

Los casos de uso reutilizan los agregados para decidir qué cambia y qué revisión
resulta. La aplicación solo coordina autorización, CAS, auditoría y transacción.

### 3. Contrato de permisos

| Operación | Permiso |
| --- | --- |
| Listar y consultar usuarios | `user.read` |
| Crear usuarios | `user.create` |
| Editar nombre, email y estado | `user.update` |
| Reemplazar roles de organización | `role.assign` |
| Listar y consultar roles | `role.read` |
| Crear roles | `role.create` |
| Editar nombre y estado de roles | `role.update` |
| Reemplazar permisos de un rol | `role.permissions.manage` |
| Consultar el catálogo de permisos | `permission.read` |
| Listar membresías | `branch.membership.read` |
| Crear membresías | `branch.membership.create` |
| Editar el estado de una membresía | `branch.membership.update` |
| Reemplazar roles de una membresía | `branch.membership.roles.manage` |

Crear un rol con permisos exige además `role.permissions.manage`: crear un rol
vacío y rellenarlo después no puede ser una vía para eludir la concesión.
Asignar roles en el alta de una membresía exige
`branch.membership.roles.manage`.

Estos trece códigos ya existían en el catálogo de 59 permisos y en
`PERMISSION_CATALOG`; no se inventa ninguno nuevo. Solo se añaden las variantes
correspondientes al enum `Permission` de aplicación.

### 4. Continuidad de la administración

Tres reglas protegen que la instalación no quede sin administración:

1. Un administrador **no puede modificar sus propios roles** (`role.assign` sobre
   sí mismo responde `Forbidden`). Evita la auto-escalada y el auto-bloqueo.
2. Un administrador **no puede desactivar su propia cuenta**.
3. Ninguna operación puede dejar la organización sin ningún **administrador
   efectivo**, definido como usuario ACTIVO de organización ACTIVA con rol ACTIVO
   que concede `user.update`. Se comprueba al desactivar un usuario, al reemplazar
   sus roles y al retirar `user.update` de un rol.

La definición es SQL explícito y compartido por los tres controles; no depende de
nombres de rol ni de convenciones de la interfaz.

### 5. Credenciales de usuarios nuevos

`create_user` recibe una contraseña inicial en claro, la deriva con Argon2id
**antes** de abrir la transacción (coste de memoria fuera de la reserva de
escritura) y persiste solo el hash. El puerto `PasswordHasher` vive en la capa de
aplicación; el adaptador `LocalPasswordHasher` se ejecuta en `spawn_blocking` con
la política `LAB_V1`, la misma del asistente de primera instalación, de modo que
ambos caminos producen material verificable por el mismo gate.

La política comercial sigue pendiente de D-AUTH-09. Este ADR **no** la ratifica.

### 6. Revocación de sesiones

Desactivar una cuenta revoca sus sesiones activas en la **misma** UoW. Además, la
autorización ya era DB-authoritative: cada operación protegida revalida usuario,
estado, organización, roles y permisos en SQLite, de modo que retirar un rol o un
permiso surte efecto en la siguiente llamada sin reiniciar la aplicación ni
invalidar la sesión.

### 7. Frontera IPC

Catorce comandos, todos con el principal resuelto desde el handle nativo:

`user_list`, `user_get`, `user_create`, `user_update`, `user_set_roles`,
`role_list`, `role_get`, `role_create`, `role_update`, `role_set_permissions`,
`permission_list`, `membership_list`, `membership_create`, `membership_update`.

Los DTO rechazan campos desconocidos. Ninguno transporta organización, actor,
permisos, hashes ni digests. Las revisiones viajan como **texto decimal exacto**,
nunca como `number` de JavaScript.

## Consecuencias

- El bloqueo de permisos de sucursal desaparece: una membresía activa con rol
  activo habilita `auth_select_branch` y las operaciones de alcance sucursal.
- La revocación de permisos es inmediata en el siguiente comando protegido.
- Los módulos de caja, ventas, inventario y compras quedan desbloqueados en su
  capa de autorización; su IPC y pantallas siguen pendientes y son incrementos
  posteriores.
- No se implementa restablecimiento administrativo de contraseña: exige contrato
  propio y pruebas de seguridad.
- No se implementa rotación automática de hashes existentes: depende de
  `VerificationOutcome::Verified { rehash_required }` y de D-AUTH-09.
- El E2E probado es de integración Rust sobre SQLite productiva e IPC con
  `MockRuntime`; **no** es un E2E gráfico de WebView2.

## Referencias

- [ADR-0026](0026-local-schema-promotion-v040.md) — promoción del esquema local.
- [ADR-0027](0027-local-authentication.md) — autenticación y custodia de sesión.
- [ADR-0028](0028-guided-first-installation.md) — primera instalación guiada.
- [ADR-0029](0029-windows-native-file-custody.md) — custodia nativa en Windows.
- `docs/05-development/rust-v040-cut-matrix.md` — estado de corte.
