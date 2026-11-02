# ADR-0031 — Custodia nativa de la sucursal seleccionada

**ACEPTADA para la release técnica v0.4.0.** Define dónde vive la sucursal
operativa de la sesión y por qué no viaja en cada comando.

## Contexto

ADR-0027 dejó el handle de sesión en memoria nativa y ADR-0030 añadió la
administración de membresías, que es la única vía por la que un principal obtiene
permisos de alcance sucursal.

Al activar cajas y turnos apareció un defecto real: `auth_select_branch` devolvía
la identidad del scope de sucursal **solo en esa respuesta**, pero
`authenticated_principal()` revalidaba la sesión con `branch_id = None`. Cualquier
comando con `require_branch_scope` respondía `Forbidden` después de seleccionar
una sucursal, aunque la selección hubiera sido válida y auditada.

Las dos alternativas evidentes eran incorrectas:

1. **Aceptar `branchId` en cada petición.** Convierte un dato de autorización en
   entrada del renderer y rompe la regla de que el scope lo decide SQLite.
2. **Persistir la selección en SQLite o en el renderer.** Introduce una segunda
   fuente de verdad para el contexto operativo y expone datos de sesión en el
   almacenamiento del navegador.

## Decisión

La sucursal seleccionada se custodia **en memoria nativa**, junto al handle de
sesión, dentro de `ProductInstallation`:

- `auth_select_branch` valida y audita la sucursal con el caso de uso existente y
  **solo después** la guarda en el proceso Rust.
- `authenticated_principal()` revalida la sesión pasando esa sucursal a
  `IdentityApplication::principal_in`, de modo que el principal resultante ya
  incluye la pierna de sucursal resuelta por SQLite.
- `auth_me` usa `current_in` para devolver la identidad del mismo scope que
  gobernarán los comandos siguientes.
- `auth_login`, `auth_logout` y `auth_logout_all` limpian la selección; el cierre
  de la aplicación la descarta con el resto del estado nativo.

La selección **no** concede permisos por sí misma: `important` el scope y los
permisos se recalculan en cada operación. Retirar una membresía o desactivar una
cuenta surte efecto en la siguiente llamada sin reiniciar la aplicación.

## Consecuencias

- Los comandos operativos (cajas, turnos y, después, ventas e inventario) no
  aceptan `branchId`: lo leen del estado verificado.
- La interfaz no necesita reenviar la sucursal y no puede ampliar su scope.
- Una reapertura del proceso exige volver a iniciar sesión y volver a seleccionar
  sucursal; es coherente con la custodia efímera del handle (ADR-0027).
- Si la sucursal se desactiva mientras la sesión está abierta, el scope sigue
  apuntando a ella, pero la autorización de cada operación vuelve a consultar
  estado y permisos en SQLite, por lo que las operaciones normales quedan
  denegadas y solo el camino de recuperación del contrato de turnos puede cerrar
  un turno histórico.

## Referencias

- [ADR-0027](0027-local-authentication.md) — autenticación y custodia de sesión.
- [ADR-0030](0030-local-identity-administration.md) — membresías de sucursal.
- `docs/03-api/rust-desktop-ipc.md` — contrato de los comandos de caja.
