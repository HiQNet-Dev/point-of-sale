# ADR-0023 — Vertical Tauri/IPC de lectura en laboratorio

## Estado y autoridad

**Implementación técnica aislada solicitada en 1X; activación comercial propuesta,
NO aprobada.** No modifica el Design Freeze ni aprueba D-AUTH/D-INST/D-CAT,
D-CASH/D-SALES/D-INV o ADR-0022. Materializa en pruebas la preferencia Tauri/IPC
de ADR-0020 y F4–21, sin sustituir Java ni introducir HTTP.

## Decisión de implementación

- `backend-rust/tools/desktop-lab` es una herramienta independiente, no una nueva
  ruta comercial de `pos-local`. Mantiene workspace/lockfile propios para que
  GTK/WebKit/Tauri no se conviertan en dependencias del workspace headless.
- Tauri **2.12.5**, Tauri Specta **=2.0.0-rc.26**, Specta **=2.0.0-rc.26**,
  Specta TypeScript **=0.0.13**, API JS **2.12.4**. Versiones comprobadas en el
  registro y en sus fuentes oficiales, compiladas con Rust 1.99.1. Specta sigue
  siendo RC; no se considera una garantía comercial de estabilidad.
- Un registro exporta bindings y configura los handlers `lab_list_branches`,
  `lab_list_categories` y `lab_list_products`. Las capabilities solo permiten
  esas lecturas a la ventana local `main`; no conceden shell, filesystem ni
  bootstrap. Orígenes remotos carecen de autorización.
- El composition root de laboratorio conserva un directorio SQLite temporal;
  no acepta rutas de base de datos ni sesiones del frontend. Reutiliza la
  fixture explícita de `pos-lab`, ahora biblioteca además de CLI, sin duplicar
  el escenario ni los casos comerciales.
- Cada invocación vuelve a resolver principal/sesión/permisos desde SQLite.
  La sesión y el reloj son **fixtures sintéticas fijas**, no login comercial.
  Nunca se ofrecen como identidad del producto.
- `pos-application::organization::list_authorized_branches` introduce un puerto
  de lectura real; el adaptador existente implementa ese puerto. No cambia el
  bootstrap SYSTEM ni crea otro camino de mutación Organization/Branch.
- React tiene entrada y build separados (`rust-lab.html`), usa el tema MUI y
  TanStack Query existentes y consume un servicio sobre bindings generados.
  La aplicación Java conserva su router, login, cliente HTTP y OpenAPI.
- Los importes son strings escala 4 y las versiones strings enteros exactos.
  No se promete compatibilidad automática con DTOs Java ni se ofrecen writes.

## Validación y límites

El dispatcher con MockRuntime, el cliente de subprocess y el E2E gráfico son
pruebas distintas. El E2E Windows utiliza WebView2 **real**, React y los handlers
reales; Playwright se conecta a un puerto DevTools efímero de loopback habilitado
únicamente en el proceso de prueba. No es un adaptador HTTP del POS y el arranque
manual no habilita ese puerto. Docker ejecuta la suite headless, no el E2E GUI.

## Aceptación pendiente para el producto

El titular debe confirmar la activación del transporte desktop y la política de
sesión de la distribución, junto con DDL/Auth/RBAC/auditoría autorizados. Antes
de habilitar usuarios reales: composition root canónico en `pos-local`, credenciales
y sesiones comerciales aprobadas, eliminación de fixtures del camino comercial,
contratos/paginación definitivos, instalador/actualización y E2E de seguridad.
D-AUTH-09 sigue bloqueada. Nada en este ADR constituye esa aceptación.
