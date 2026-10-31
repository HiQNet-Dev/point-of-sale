# Evidencia criptográfica D-AUTH-09 — Incremento 2B

**Estado: D-AUTH-09 continúa BLOQUEADA.** Este documento añade el aislamiento de
memoria por proceso que faltaba en 2A y la política de admisión implementada.
No aprueba parámetros ni habilita autenticación comercial.

Relacionado: [evidencia 2A](rust-auth-benchmark-2a.md),
[evidencia 1Z](rust-auth-benchmark-1z.md),
[código de credenciales](../../backend-rust/crates/pos-credentials),
[adaptador de laboratorio](../../backend-rust/crates/pos-storage-sqlite/src/credential/lab.rs).

## 1. Hallazgo que motiva este incremento

2A midió ≈1.46 GiB de RSS tras ocho verificaciones simultáneas y lo atribuyó a
«memoria retenida por el asignador». El número era real, pero la medición no
separaba tres cosas distintas:

1. memoria base del proceso;
2. coste criptográfico de **una** operación Argon2id (`m × 1`);
3. memoria que el asignador retiene tras varias operaciones en **el mismo**
   proceso, incluidas las fases previas del benchmark (candidatos de hasta 64 MiB
   y rondas repetidas).

`VmHWM` es monótono: medir todo en un proceso confunde las tres. 2B añade un modo
que ejecuta cada escenario en un proceso hijo nuevo.

## 2. Instrumento

```sh
cargo run -p pos-credentials --release --example argon2_bench -- \
  --rounds 5 --workers 1,4 --burst 4 --memory-isolation \
  --json target/argon2-isolation-2b.json
```

`--memory-isolation` lanza `--memory-child <workers>` en procesos independientes
para `workers ∈ {0, 1, 2, 4, 8}`. Cada hijo informa:

| Campo | Significado |
| --- | --- |
| `base_rss_kib` | Proceso recién arrancado, sin criptografía |
| `after_hash_rss_kib` | Tras **una** operación Argon2id |
| `crypto_cost_kib` | Diferencia anterior: coste real de la política |
| `peak_rss_kib` | Pico tras `workers` verificaciones simultáneas |
| `allocator_retained_kib` | Pico menos el coste esperado de una operación |
| `rss_available` | `false` cuando el sistema no expone `/proc` (Windows) |

## 3. Resultado — Linux, 2 vCPU / 2 GiB

Contenedor `rust:1.99` con `--cpus 2 --memory 2g`, política candidata
`Argon2id v=19, m=19 456 KiB, t=2, p=1`, sal 16 B, salida 32 B.

| Escenario | Base | Tras 1 hash | Pico | Retención del asignador |
| --- | --- | --- | --- | --- |
| Sin verificación | 2 308 KiB | 21 824 KiB | 21 824 KiB | 0 KiB |
| 1 verificación | 2 232 KiB | 21 872 KiB | 21 996 KiB | 124 KiB |
| 2 verificaciones | 2 208 KiB | 21 848 KiB | 41 552 KiB | 19 704 KiB |
| 4 verificaciones | 2 172 KiB | 21 688 KiB | 80 420 KiB | 58 732 KiB |
| 8 verificaciones | 2 312 KiB | 21 892 KiB | 158 328 KiB | 136 436 KiB |

Lecturas:

1. **Memoria base ≈ 2.2 MiB.** El proceso de verificación no es caro por sí mismo.
2. **Coste Argon2id ≈ 19.5 MiB**, coherente con `m = 19 456 KiB`. No hay sorpresa
   criptográfica: la política hace lo que declara.
3. **La retención crece con la concurrencia**, aproximadamente `(workers − 1) × m`
   en este proceso aislado: 2 → ~19 MiB, 4 → ~59 MiB, 8 → ~136 MiB.
4. El valor de 1.46 GiB de 2A corresponde a la acumulación del mismo proceso tras
   medir candidatos de 64 MiB y rondas repetidas, no al coste de ocho
   verificaciones: con procesos aislados, ocho verificaciones pican ~158 MiB.

## 4. Latencia y throughput (2 vCPU)

| Workers | p50 | p99 | Throughput | Fallos |
| --- | --- | --- | --- | --- |
| 1 | 17.5 ms | 28.3 ms | 51 /s | 0 |
| 4 | 44.7 ms | 81.3 ms | 78 /s | 0 |

Con dos núcleos, cuatro verificaciones simultáneas saturan la CPU: el throughput
deja de crecer y la latencia p99 se triplica. Ocho verificaciones no aportan
throughput y empeoran la latencia y el pico de memoria.

## 5. Política de admisión implementada

`VerificationGate` (adaptador de laboratorio) mantiene el semáforo y añade:

| Elemento | Comportamiento |
| --- | --- |
| Slots concurrentes | `max_concurrent` verificaciones criptográficas a la vez |
| Cola acotada | `queue_capacity` intentos en espera; el siguiente recibe saturación inmediata |
| Espera máxima | `max_wait`; al expirar responde saturación, sin bloquear el hilo de interfaz |
| Saturación segura | `IdentityError::VerificationSaturated`: no revela si el usuario existe ni el estado de su credencial |
| Cancelación | La reserva de cola se libera al abandonar la espera; el permiso viaja con el trabajo bloqueante |
| Bloqueo por cuenta | Se conserva `LockoutPolicy` con intentos fallidos y `locked_until` |

`VerificationGate::new(n)` mantiene la semántica histórica del laboratorio
(espera sin límite) y **no** debe usarse en producto. La creación con política se
hace con `VerificationGate::with_admission(AdmissionPolicy { … })`.

Pruebas añadidas (4): cola acotada y liberación del slot, rechazo de límites
degenerados, saturación inmediata con cola llena, expiración de la espera sin
retener permisos, y slot libre que nunca cuenta como cola.

## 6. Recomendación técnica (sigue requiriendo aprobación)

1. Mantener `Argon2id m=19456, t=2, p=1` como **candidata**, no como política
   comercial.
2. Admitir como máximo **4 verificaciones concurrentes** en equipos de 2 núcleos:
   pico de ~80 MiB y el mejor throughput medido.
3. Cola acotada (≈8) y espera máxima de 2 s con respuesta de saturación.
4. Límite de intentos por cuenta y por instalación en la capa de aplicación.
5. Repetir esta misma medición aislada en el hardware objetivo de PyME antes de
   aprobar `D-AUTH-09`; el contenedor acota el escenario, no sustituye al equipo.

## 7. Criterios de aceptación

| Criterio | Estado |
| --- | --- |
| Benchmark reproducible con percentiles | Cumplido (2A) |
| Interoperabilidad BCrypt ↔ Argon2id | Cumplido (1Z/2A) |
| Rehash atómico sin contraseña en claro persistida | Cumplido |
| Memoria base separada del coste criptográfico | **Cumplido (2B)** |
| Retención del asignador medida por proceso | **Cumplido (2B)** |
| Límites 1/2/4/8 evaluados | **Cumplido (2B)** |
| Política de admisión implementada y probada | **Cumplido (2B)** |
| Hardware objetivo de PyME | Pendiente |
| Aprobación del titular | Pendiente |
