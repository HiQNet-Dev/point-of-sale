# Auditoría UX programática del frontend

Este documento describe cómo se audita el frontend de HiQNet POS de forma
reproducible **sin inspección visual humana**, qué métricas se registran y qué
evidencia sustenta la modernización del modo caja.

## Objetivo

Verificar, contra la aplicación real en un entorno de desarrollo aislado:

- ausencia de desbordamiento horizontal en las resoluciones objetivo;
- comportamiento de la altura de página y del scroll (único en móvil, interno en
  escritorio);
- que en escritorio el **total y el cobro quedan visibles** sin desplazar la
  página;
- que el catálogo y el carrito se muestran **lado a lado** en escritorio y
  apilados en móvil.

## Método

La auditoría se ejecuta con **Playwright (Chromium)** contra la aplicación real
(Vite en `http://localhost:5173`) y el backend real en un entorno de desarrollo
Docker aislado. **Nunca se ejecuta contra producción.**

El recorrido abre sesión, selecciona sucursal/caja, abre un turno, crea/reanuda
una venta con al menos un producto y luego, en cada resolución, registra:

- `documentElement.scrollWidth` vs `window.innerWidth` → overflow horizontal;
- `documentElement.scrollHeight` / `body.scrollHeight` vs `innerHeight` → altura y
  scroll vertical;
- cajas (`boundingBox`) del buscador de productos, del total (`aria-label`
  «Total de la venta») y del botón **Cobrar** → posición relativa y visibilidad
  dentro del viewport;
- errores de consola del navegador.

Resoluciones medidas: `360×800`, `390×844`, `768×1024`, `1280×800`, `1366×768`,
`1440×900` y `1920×1080`.

> El script de captura es local y temporal; no forma parte del producto ni se
> versiona. El recorrido Playwright permanente y soportado por CI es la suite E2E
> descrita en `pos-e2e.md`.

### Requisitos

- Motor Docker en ejecución con los contenedores `db`, `backend` y `frontend`.
- Frontend servido por el contenedor de desarrollo (bind-mount del código).
- Si el contenedor de frontend se inició antes de un cambio de código, **reiniciar
  el contenedor** para descartar caché de módulos de Vite; de lo contrario el
  servidor puede servir una versión anterior del tema.

## Resultados (estado posterior a la modernización)

Medido con turno activo y **un producto en el carrito** (caso de venta normal).

| Resolución | Overflow horizontal | `scrollHeight` | Catálogo/carrito | Total + Cobrar visibles sin scroll |
| --- | --- | --- | --- | --- |
| 360×800 | No | 2004 | Apilados | No (scroll de página, móvil) |
| 390×844 | No | 1988 | Apilados | No (scroll de página, móvil) |
| 768×1024 | No | 1638 | Apilados | No (scroll de página, tablet) |
| 1280×800 | No | 800 (=viewport) | Lado a lado | **Sí** |
| 1440×900 | No | 900 (=viewport) | Lado a lado | **Sí** |
| 1366×768 | No | 768 (=viewport) | Lado a lado | **Sí** |
| 1920×1080 | No | 1080 (=viewport) | Lado a lado | **Sí** |

En escritorio `scrollHeight == innerHeight`: el *app-shell* evita el scroll de
página y confina el desplazamiento a las columnas. Evidencia puntual a `1280×800`:

- buscador/catálogo en la columna izquierda y total en `x ≈ 1150` (columna
  derecha) → **lado a lado**;
- total (`y ≈ 599`, alto 26) y botón **Cobrar** (`y ≈ 637`, alto 48) dentro del
  viewport de alto 800 → **visibles sin desplazamiento**; a `1366×768` el botón
  queda en `y ≈ 606` (dentro de 768);
- `scrollWidth == innerWidth` → **sin overflow horizontal**;
- alturas táctiles: mosaicos de producto 76px, botones `small` ≥36px, `Cobrar`
  48px;
- a `390×844`: una sola columna, sin overflow horizontal, con scroll de página.
- sin errores de consola del navegador en ninguna resolución.

El diálogo de cobro se abre correctamente desde el pie del carrito, con
`Restante` mostrado por el backend ($35.00 en la fixture local).

## Estado inicial (referencia)

Antes de la modernización el modo caja se apilaba en una sola columna también en
escritorio: el carrito con el total y el botón de cobro quedaba **por debajo del
catálogo**, obligando a desplazar la página para cobrar; la cabecera operacional
(Sucursal/Caja/Turno) no compactaba, y el pie del carrito podía quedar recortado.
El cambio cualitativo (apilar → *app-shell* de altura completa con pie fijo en
escritorio) es el que resuelve el problema de tener el cobro fuera de la vista.

## Limitaciones

- La verificación es **programática** (métricas de DOM y cajas): no sustituye una
  revisión visual humana ni juicios estéticos finos.
- Las capturas de pantalla se guardan como artefacto local, no se versionan.
- Los umbrales de visibilidad dependen de la resolución y del contenido del turno;
  con listas muy largas el catálogo y el carrito hacen scroll interno, que es el
  comportamiento buscado.

## Falso positivo a evitar

`getByText('TURNO ACTIVO')` de Playwright **coincide por subcadena y sin distinguir
mayúsculas**, por lo que también hace match con «Sin turno activo». Para detectar
un turno activo debe usarse una expresión regular sensible a mayúsculas, por
ejemplo `getByText(/TURNO ACTIVO/)`. Ignorar esto hizo que un recorrido anterior
no abriera turno y midiera una pantalla sin modo caja.
