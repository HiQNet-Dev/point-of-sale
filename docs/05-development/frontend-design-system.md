# Sistema de diseño del frontend

Documenta el sistema visual de HiQNet POS: dónde viven los tokens, cómo se
consumen, qué reglas respetar y cómo se comporta el **modo caja**. El objetivo es
que toda la interfaz comparta una identidad coherente y predecible sin recurrir a
valores sueltos por pantalla.

## Ubicación e integración

- Tokens y tema: `frontend/src/app/theme/appTheme.ts`.
- Consumo: `frontend/src/app/providers/AppProviders.tsx` envuelve la aplicación en
  `ThemeProvider` con `appTheme` y aplica `CssBaseline`.
- Los componentes consumen el tema vía `sx`, `theme.palette`, `theme.spacing`,
  `theme.typography` o los overrides de MUI. No se fijan colores, radios ni
  tamaños arbitrarios en cada pantalla.

`appTheme.ts` exporta además `designTokens`, el objeto con la fuente única de
verdad de la identidad. Los overrides del tema derivan de esos tokens, por lo que
cambiar la identidad se hace en un solo lugar.

## Tokens

| Grupo | Token | Uso |
| --- | --- | --- |
| `brand` | `main`, `light`, `dark` | Azul institucional: acción principal y foco. |
| `neutral` | `background`, `surface`, `border`, `textPrimary`, `textSecondary` | Superficies y textos de alto contraste. |
| `status` | `success`, `warning`, `error` | Estados semánticos alineados con los `Chip` de la aplicación. |
| `shape` | `radius`, `radiusLarge` | Radios comunes de controles y diálogos. |

El espaciado usa la escala de `8px` de MUI (`spacing: 8`) y la tipografía está
definida por variante con una escala compacta pensada para densidad de datos de
punto de venta.

## Reglas

1. **Sin valores sueltos.** Colores, radios y tamaños se toman del tema o de
   `designTokens`; nunca se codifican hex o `px` arbitrarios por componente.
2. **Sin estética genérica.** No se agregan degradados, `glow`, blobs, sombras
   ornamentales ni animación decorativa. La jerarquía se logra con tipografía,
   contraste, espaciado y bordes.
3. **Identidad discreta.** El color de marca se reserva para acciones y foco; las
   superficies y textos son neutros y legibles en uso prolongado.
4. **Foco visible.** `MuiCssBaseline` define `:focus-visible` con contorno del
   color de marca para toda la aplicación; no se elimina el contorno sin
   reemplazo equivalente.
5. **Controles táctiles.** Botones y filas mantienen alturas mínimas razonables
   en pantallas pequeñas (`Button` con `size="small"` en barras densas y
   `size="large"` en la acción principal de cobro).
6. **No ocultar controles necesarios.** Los selectores de contexto, la
   paginación y los botones requeridos por accesibilidad y pruebas permanecen
   visibles y alcanzables por teclado; la densidad se logra compactando, no
   escondiendo.

## Modo caja

El modo caja es la pantalla de venta con turno activo. Su layout prioriza catálogo
y carrito simultáneos en escritorio y un único scroll en móvil.

- **Escritorio (`md`, ≥900px):** *app-shell* de altura completa. Con turno activo
  el elemento `main` fija `height: 100dvh`, `overflow: hidden` y `display: flex`
  en columna; la cadena de contenedores (`Container`→`Stack`→`Paper`→`Stack`→
  `SalesWorkspace`) usa `flexGrow: 1` y `minHeight: 0` para repartir la altura. El
  catálogo y el carrito viven en una rejilla de dos columnas
  (`minmax(0, 1fr) minmax(340px, 400px)`) con una fila `minmax(0, 1fr)`; cada
  columna hace **scroll interno** y el pie del carrito (total + **Cobrar**) queda
  fijo, siempre visible sin desplazar la página.
- **Cabecera compacta:** en `md` con turno activo se ocultan por CSS los rótulos
  redundantes (título de sección, filas «Sucursal actual»/«Caja actual», subtítulo
  «Turno») y el resumen del turno pasa a una fila horizontal. Los textos siguen en
  el DOM (solo se ocultan visualmente), preservando accesibilidad y pruebas.
- **Modo altura completa (`fillHeight`):** prop que `PosSessionPage` activa al
  montar `SalesWorkspace`. En móvil/tablet sus estilos (`minHeight: 0`,
  `overflowY: auto`) son inertes porque la página no impone altura; el shell solo
  se activa en `md`.
- **Móvil y tablet:** una sola columna; el carrito aparece tras el catálogo y la
  página tiene un único scroll vertical (sin scroll anidado).
- **Ancho completo:** con turno activo el contenedor deja de limitar el ancho a
  `md`, permitiendo que la rejilla use el ancho disponible.
- **Sin desbordamiento horizontal:** todas las columnas usan `minWidth: 0`, los
  textos largos se envuelven (`overflowWrap: 'anywhere'`) y las barras de chips
  hacen scroll horizontal propio.

### Atajo de teclado

Con catálogo disponible, `F2` enfoca el campo **Buscar productos** (que expone
`aria-keyshortcuts="F2"`). El atajo se ignora cuando se repite, cuando hay
modificadores (`Ctrl`/`Meta`/`Alt`), cuando el foco está en un campo editable
(`INPUT`/`TEXTAREA`/`SELECT`/`contentEditable`) o cuando hay un diálogo, popover o
`listbox` abierto, para no interferir con la captura normal.

### Controles táctiles

Las alturas mínimas de `MuiButton` derivan del tamaño (`small` 36px, `medium`
40px, `large` 48px) desde el tema, de modo que las acciones del carrito, cantidad,
búsqueda y turno sean alcanzables en tablet y móvil sin fijar `px` por pantalla.

## Cómo extender

- Agregar o ajustar identidad: editar `designTokens` en `appTheme.ts`; los
  overrides y el resto de la app heredan el cambio.
- Nuevo override de componente: declararlo bajo `components` en `appTheme.ts`,
  derivando de los tokens.
- Nuevo layout: reutilizar las convenciones de rejilla y `minWidth: 0` descritas
  en **Modo caja** para conservar el comportamiento responsivo y sin overflow.

## Validación

Antes de dar por terminado un cambio de UI:

```text
cd frontend && pnpm lint && pnpm typecheck && pnpm typecheck:e2e && pnpm test && pnpm build
```

La evidencia visual programática (resoluciones, overflow y jerarquía del modo
caja) se describe en `docs/05-testing/frontend-ux-audit.md`.
