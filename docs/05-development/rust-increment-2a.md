# Incremento 2A — desbloqueo técnico y preparación de v0.4.0

**Estado: implementado y validado localmente.** Ninguna decisión queda aprobada,
ninguna migración promovida, sin merge ni release. `VERSION` permanece
`0.4.0-SNAPSHOT`.

Base: `feat/fullstack/rust-desktop-vertical-1x`, HEAD previo `05ba4c17`, PR #84
(Draft, base `dev`).

## 1. Resumen de lo entregado

| Frente | Resultado real |
| --- | --- |
| D-AUTH-09 | Instrumento ampliado (percentiles, límites, ráfaga, memoria por fase) y tres ejecuciones registradas; hallazgo de RSS bajo concurrencia |
| Bloque D | Rechazo de enlaces, *junctions* y directorios compartidos; política propuesta completa; `app_data_dir` sigue pendiente por D-INST |
| ADR-0021 | Resolución técnica con una sola pregunta al titular; dominio ya representa «costo desconocido ≠ cero» con prueba nueva |
| 30 casos normativos | Sin cambios de estado; clasificación conservada |
| Migraciones | Ninguna promovida |
| Escritura productiva | Preparada y probada en laboratorio; no activada |
| Purchasing | Inventario de paridad y estrategia de cierre; sin persistencia |
| Pruebas | Rust 293, desktop 22, frontend 212, E2E y Docker PASS |

## 2. D-AUTH-09

Instrumento (`examples/argon2_bench.rs`) ahora mide `p50/p95/p99` de hash y verify
por separado, concurrencia con throughput real, ráfaga de intentos simultáneos,
efecto de límites externos de CPU/RAM y RSS por fase.

Evidencia (`docs/05-development/evidence-2a/`):

| Escenario | verify p50 | verify p95 | 8 concurrentes p99 | Ráfaga 16 |
| --- | --- | --- | --- | --- |
| Windows compartido | 24.8 ms | 27.8 ms | 124 ms | 82 ms p50 |
| Linux libre (12 hilos) | 26.3 ms | 26.9 ms | 621 ms (16 workers) | 177 ms p50 |
| **Linux 2 vCPU / 2 GiB** | **35.5 ms** | **43.5 ms** | **388 ms** | **400 ms p50** |

**Hallazgo nuevo y accionable:** el RSS no es `m × workers`. Con 8 verificaciones
de 19 MiB el proceso alcanzó **1 460 548 KiB**; en el escenario libre, 3.6 GiB. El
asignador retiene memoria por hilo. En un equipo de 4 GiB esto es inaceptable, así
que la política debe incluir un **techo de concurrencia** (recomendado: 4).

Recomendación mantenida: `Argon2id m=19456, t=2, p=1`. D-AUTH-09 **sigue
bloqueada**: falta ejecución en hardware objetivo y aprobación del titular,
además de la política de límite de intentos.

## 3. Bloque D

Implementado sin activar la decisión:

- Se rechazan enlaces simbólicos y, en Windows, cualquier punto de reanálisis
  (`FILE_ATTRIBUTE_REPARSE_POINT`), lo que cubre *junctions* que `is_symlink()`
  no reporta.
- Directorios escribibles por todos se rechazan en Unix; creación con `0700`.
- El archivo de base de datos tampoco puede ser enlace simbólico.
- Se mantiene la prohibición de rutas desde JavaScript y la ausencia de HTTP.

Pendiente de decisión: directorio por plataforma (`app_data_dir`) e identificador
de aplicación estable. Ambos son identidad de producto y no se cambiaron.

## 4. ADR-0021

Resolución técnica en `rust-adr0021-resolution-2a.md`:

- El dominio Rust **ya** modela `unit_cost: Option<UnitPrice>`; se añadió
  `agreed_cost_distinguishes_unknown_from_zero` para fijar que desconocido ≠ cero.
- Se comparan cuatro alternativas físicas y se recomienda `unit_cost TEXT NULL`
  con total de orden `NULL` cuando falte algún costo.
- Se documentan las cinco fichas padre ausentes y la independencia de roots.

Se reduce el conflicto a **una decisión** del titular; nada se aprueba aquí.

## 5. Casos normativos

Sin cambios: **1 PASSED (PAR-08), 28 BLOCKED, 1 NOT_RUN**. `DDL13-05/06/09/11`
siguen requiriendo el esquema completo; `PAR-05`/`AS-09` recibieron evidencia
criptográfica en 1Z/2A pero exigen target persistido y política aprobada. Ver
`rust-increment-1z-normative.md`.

## 6. Migraciones

Ninguna promovida. `migrations/local` sigue conteniendo solo `0001`. La matriz de
dependencias de `0002`–`0015` sigue ejecutándose como prueba (1Z) y pasa.

## 7. Login comercial y escritura productiva

- **Login:** no implementado. Requiere bloque A + D-AUTH-09.
- **`create_product`:** probado de extremo a extremo en laboratorio con auditoría
  transaccional y conflicto tipado. Para activarlo en producto faltan: `0003`
  (identidad), `0006` (auditoría), `0008` (catálogo) aprobadas y composición
  canónica. No se creó una implementación paralela.

## 8. Purchasing

Auditoría en `rust-purchasing-parity-2a.md`: 17 endpoints Java, dominio Rust
completo, sin persistencia, casos de uso ni API. Estrategia en cinco etapas
(P1 decisión `unit_cost` → P5 import y paridad). `v0.4.0` no puede declararse
preparado sin este alcance.

## 9. Validación

| Ámbito | Resultado |
| --- | --- |
| Workspace Rust: fmt, clippy `-D warnings`, test, build | **293 pruebas**, 0 fallos |
| Desktop: fmt, clippy `-D warnings`, test | **22 pruebas**, 0 fallos |
| Frontend: lint, `tsc`, vitest | **212 pruebas**, 3 opcionales omitidas |
| E2E WebView2 laboratorio | PASS (5 comandos, incluye escritura y `CONFLICT`) |
| E2E WebView2 instalación | PASS (primera instalación → reabierta) |
| Docker Linux | PASS (`first_install`/`reopened`, hash `278382e9…`) |
| Benchmark + interoperabilidad | Ejecutados y registrados |

Nota honesta: en ejecuciones paralelas del frontend bajo carga del host se
observaron *timeouts* intermitentes en pruebas de integración de la UI Java (no
relacionadas con 2A). Se añadió un límite propio de 20 s a la prueba más pesada
(`PosSessionPage`, recorrido con login y tres consultas) sin tocar aserciones; la
suite completa pasó después con 212/212.

## 10. CI y PR

Actions **no ejecutado** (restricción vigente). PR #84 actualizado con el estado
1X–2A, sigue Draft y no es merge-ready.

## 11. Requisitos para `v0.4.0`

1. Bloques A, B y C firmados; migraciones promovidas con clean install y upgrade.
2. D-AUTH-09 medida en hardware objetivo y política aprobada, incluido el techo de
   concurrencia.
3. Bloque D cerrado (directorio por plataforma e identificador estable).
4. Purchasing persistente y expuesto, con import sintético y paridad.
5. Casos normativos ejecutados con evidencia real y CI completo sobre el SHA final.
6. Instalador, plan de rollback y Release Audit.
