# Evidencia criptográfica D-AUTH-09 — Incremento 2A

**Estado: evidencia ampliada. D-AUTH-09 continúa BLOQUEADA.** Este documento
sustituye en precisión al de 1Z (`rust-auth-benchmark-1z.md`, conservado como
histórico) y añade percentiles, límites externos, ráfaga de intentos y perfil de
memoria.

## 1. Instrumento

```sh
cargo run -p pos-credentials --release --example argon2_bench -- \
  --rounds 10 --workers 1,4,8 --burst 16 --json <ruta>
```

Novedades de 2A:

- Latencia individual por operación: `p50`, `p95`, `p99`, mínimo, máximo y media,
  con **hash y verify separados**.
- Escenarios de concurrencia con distribución de latencia y throughput real.
- **Ráfaga** de intentos simultáneos que modela una avalancha de credenciales.
- `POS_BENCH_LIMITS` registra los límites externos (CPU/RAM) junto al entorno.
- Pico de RSS por fase cuando existe `/proc`.

Los límites de CPU y memoria se aplican **desde fuera** del binario (contenedor,
`systemd`, `taskset`). Simularlos dentro del proceso daría una cifra que no
corresponde a ninguna máquina real.

## 2. Entornos

| Clave | CPU | CPU efectivas | Memoria | Clase |
| --- | --- | --- | --- | --- |
| Windows | AMD64 Family 25 Model 80 | 12 | 32 GiB | `DEV_WORKSTATION`, máquina compartida |
| Linux libre | AMD Ryzen 5 5600G | 12 | 16 GiB | `CONTAINER_LINUX`, sin límites |
| Linux 2 vCPU | AMD Ryzen 5 5600G | **2** | **2 GiB** | `CONTAINER_LINUX_2CPU`, límites reales |

El escenario 2 vCPU / 2 GiB es el proxy más cercano a un equipo económico de
comercio, pero **no es hardware objetivo**: comparte host, hipervisor y
almacenamiento. La aceptación de D-AUTH-09 sigue exigiendo una ejecución en el
equipo que se venderá.

## 3. Política candidata

`Argon2id v=19, m=19 456 KiB, t=2, p=1`, sal 16 B, salida 32 B.

| Entorno | verify p50 | verify p95 | hash p50 |
| --- | --- | --- | --- |
| Windows (compartido) | 24.8 ms | 27.8 ms | 25.2 ms |
| Linux libre | 26.3 ms | 26.9 ms | 26.3 ms |
| **Linux 2 vCPU / 2 GiB** | **35.5 ms** | **43.5 ms** | 24.5 ms |

Cumple con holgura el umbral propuesto de 250 ms para una verificación
individual, incluso con dos núcleos.

## 4. Concurrencia y ráfaga

| Entorno | Workers | p50 | p99 | Throughput | Fallos |
| --- | --- | --- | --- | --- | --- |
| 2 vCPU | 1 | 35.6 ms | 39.8 ms | 31 /s | 0 |
| 2 vCPU | 4 | 82.3 ms | 112.3 ms | 53 /s | 0 |
| 2 vCPU | 8 | 200.4 ms | 388.2 ms | 38 /s | 0 |
| 2 vCPU | ráfaga 16 | 400.2 ms | 429.6 ms | 35 /s | 0 |
| Linux libre | 16 | 98.4 ms | 621.3 ms | 82 /s | 0 |

Lectura: con dos núcleos, **cuatro verificaciones simultáneas** ya saturan la
CPU (el throughput deja de crecer) y ocho degradan la latencia p99 a ~390 ms. En
la ráfaga de 16 todos los intentos terminan, pero con ~400 ms de latencia media.
Ninguno falla: el sistema no pierde solicitudes, solo se encola.

**El throttling no es criptográfico.** El límite de intentos por usuario/IP es una
política de aplicación; el instrumento solo mide el coste de atenderlos.

## 5. Memoria: hallazgo relevante

| Fase (2 vCPU / 2 GiB) | RSS pico acumulado |
| --- | --- |
| Tras políticas (`LAB_V0`/`LAB_V1`) | 215 240 KiB |
| Tras candidatos (hasta 64 MiB) | 280 632 KiB |
| **Tras concurrencia de 8 workers** | **1 460 548 KiB** |
| Linux libre, tras 16 workers y ráfaga 32 | 3 595 016 KiB |

El pico **no** es `m × workers`: el asignador retiene bloques por hilo. Con
8 verificaciones simultáneas de 19 MiB se observó ≈1.4 GiB de RSS, un valor
inaceptable en un equipo de 4 GiB que además ejecuta el POS.

Consecuencia técnica: la política de parámetros es segura, pero **debe acotarse
la concurrencia de verificación**. La base ya dispone de
`credential_lab::VerificationGate` (semáforo de permisos); la política comercial
de cuántos intentos simultáneos admitir y cómo encolar/limitar por usuario queda
pendiente de decisión, junto con D-AUTH-09.

> **Corrección 2B.** El aislamiento por proceso
> ([`rust-auth-benchmark-2b.md`](rust-auth-benchmark-2b.md)) muestra que el coste
> criptográfico real es ≈19.5 MiB por verificación y que la cifra de 1.46 GiB
> acumulaba candidatos de 64 MiB y rondas en el mismo proceso. Con procesos
> aislados, 8 verificaciones pican ≈158 MiB. La necesidad de acotar la
> concurrencia se mantiene, pero el techo razonable es 4 verificaciones
> simultáneas. 2B implementa además cola acotada, espera máxima y respuesta de
> saturación segura.

## 6. Interoperabilidad

Ejecutada en 1Z y conservada: BCrypt (coste 10/12) de Java → Rust
`VERIFIED_REHASH`; Argon2id de Rust → `Argon2PasswordEncoder` de Java `true` con
la contraseña correcta y `false` con otra. Reproducible con
`node backend-rust/tools/credential-interop/run.mjs`.

## 7. Recomendación (propuesta, no aprobada)

1. Política comercial: `Argon2id m=19456, t=2, p=1`, sal 16 B, salida 32 B.
2. **Techo de concurrencia de verificación en 4** para equipos de 2 núcleos; el
   resto se encola con límite de tiempo y respuesta segura.
3. Techo de memoria de política en 32 768 KiB; 46 MiB (valor por defecto de
   Spring) requiere decisión explícita y medición en hardware objetivo.
4. Renombrar la política al aprobarla; ningún identificador `lab-*` en producto.
5. Rehash BCrypt→Argon2id dentro de la transacción de login, ya probado.

## 8. Criterios de aceptación de D-AUTH-09

| Criterio | 1Z | 2A |
| --- | --- | --- |
| Benchmark reproducible con JSON | Cumplido | Cumplido, con percentiles y límites |
| Interoperabilidad Java ↔ Rust | Cumplido | Cumplido |
| Rehash atómico sin contraseña en claro persistida | Cumplido | Cumplido |
| Efecto de límites de CPU/RAM medido | No | **Cumplido** (2 vCPU / 2 GiB) |
| Ráfaga de intentos simultáneos medida | No | **Cumplido** |
| Perfil de memoria bajo concurrencia | Parcial | **Cumplido** (hallazgo de RSS) |
| Hardware objetivo de PyME | Pendiente | **Pendiente** |
| Política de límite de intentos aprobada | Pendiente | **Pendiente** |
| Aprobación del titular | Pendiente | **Pendiente** |

## 9. Ejecución en el equipo objetivo

```sh
cd backend-rust
POS_BENCH_CPU="modelo exacto del equipo" POS_BENCH_MEMORY_KIB=<KiB> \
POS_BENCH_HARDWARE_CLASS=TARGET_SMALL_BUSINESS \
POS_BENCH_LIMITS="sin límites artificiales" \
cargo run -p pos-credentials --release --example argon2_bench -- \
  --rounds 20 --workers 1,2,4,8 --burst 16 --json target/argon2-target.json
```

El JSON es la evidencia que falta. Un contenedor con `--cpus` y `--memory`
acota el escenario, pero no sustituye al equipo real.
