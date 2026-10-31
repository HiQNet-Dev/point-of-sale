# Evidencia criptográfica D-AUTH-09 — Incremento 1Z

> **Superado en precisión por [`rust-auth-benchmark-2a.md`](rust-auth-benchmark-2a.md)**,
> que añade percentiles, límites de CPU/RAM, ráfaga de intentos y perfil de
> memoria. Este documento se conserva como histórico del primer instrumento.

**Estado: evidencia técnica registrada. D-AUTH-09 continúa BLOQUEADA.** Este
documento no aprueba parámetros: aporta las mediciones y la interoperabilidad que
faltaban para que el titular pueda decidir.

Artefactos reproducibles (JSON generado por el propio binario):

- `docs/05-development/evidence-1z/1z-argon2-bench-windows.json`
- `docs/05-development/evidence-1z/1z-argon2-bench-container-linux.json`
- `docs/05-development/evidence-1z/1z-credential-interop.json`

## 1. Instrumentos

| Instrumento | Comando |
| --- | --- |
| Benchmark Argon2id | `cargo run -p pos-credentials --release --example argon2_bench -- --rounds 5 --workers 1,2,4,8 --json <ruta>` |
| Interoperabilidad Java ↔ Rust | `node backend-rust/tools/credential-interop/run.mjs` |

El benchmark mide dos rutas distintas:

1. La **ruta pública real** (`hash_password` + `verify_credential`) para las
   políticas registradas `LAB_V0` y `LAB_V1`.
2. **Candidatos adicionales** construidos con `argon2::Params` solo para medir,
   incluidos dos que la política actual rechaza: sirven para cuantificar el coste
   de subir memoria o iteraciones.

La interoperabilidad usa las clases reales del backend: `BCryptPasswordEncoder` y
`Argon2PasswordEncoder` de `spring-security-crypto`, resueltas desde el
repositorio Maven local (sin red). Versiones observadas: `spring-security-crypto
7.1.1`, `spring-core 7.0.9`, `bcprov-jdk18on 1.78`, coherentes con Spring Boot
4.1.1 declarado en `backend/pom.xml`.

## 2. Entornos medidos

| Entorno | CPU | Lógicos | Memoria | Clase |
| --- | --- | --- | --- | --- |
| Windows dev | AMD64 Family 25 Model 80 | 12 | 32 GiB | `DEV_WORKSTATION` |
| Linux contenedor | AMD Ryzen 5 5600G | 12 | 16 GiB | `CONTAINER_LINUX` |

Rust 1.99.1, perfil `release`. El contenedor se ejecutó con `--network none` y
usuario no privilegiado; el pico de RSS solo puede leerse con `/proc`.

**Ninguno de los dos entornos es el hardware objetivo de una PyME.** El 5600G es
una referencia razonable de escritorio económico, pero la aceptación de
D-AUTH-09 exige al menos una medición en la clase de equipo que se venderá.

## 3. Resultados

### 3.1 Políticas registradas (ruta pública)

| Política | m KiB | t | p | hash ms (Win) | verify ms (Win) | hash ms (Linux) | verify ms (Linux) |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `lab-argon2-v0` | 8 192 | 1 | 1 | 4.44 | 4.06 | 6.71 | 4.54 |
| `lab-argon2-v1` | 19 456 | 2 | 1 | 21.35 | 17.32 | 17.31 | 17.82 |

### 3.2 Candidatos (medición cruda)

| Candidato | m KiB | t | p | verify ms (Win) | verify ms (Linux) | Aceptado por política |
| --- | --- | --- | --- | --- | --- | --- |
| `cap-32m-2t-1p` | 32 768 | 2 | 1 | 30.42 | 43.16 | Sí |
| `cap-32m-3t-1p` | 32 768 | 3 | 1 | 43.90 | 58.74 | Sí |
| `spring-default-46m-1t-1p` | 47 104 | 1 | 1 | 25.87 | 43.17 | **No** (techo 32 768) |
| `strong-64m-3t-1p` | 65 536 | 3 | 1 | 95.26 | 127.45 | **No** |

El valor por defecto de Spring para Argon2 (46 MiB) queda **fuera** del techo
actual. Aceptarlo como política comercial obligaría a elevar
`MAX_ARGON2_MEMORY_KIB`, lo que es una decisión normativa, no un ajuste técnico.

### 3.3 Concurrencia (`lab-argon2-v1`)

| Workers | Operaciones | ms por operación (Win) | ms por operación (Linux) | Fallos |
| --- | --- | --- | --- | --- |
| 1 | 5 | 17.52 | 24.64 | 0 |
| 2 | 10 | 9.92 | 12.88 | 0 |
| 4 | 20 | 7.15 | 8.30 | 0 |
| 8 | 40 | 5.95 | — | 0 |

Con 8 verificaciones simultáneas no hubo errores ni degradación: Argon2id es
CPU-bound y el sistema operativo reparte cargas. **Memoria agregada** en el pico:
≈ `workers × m`. En el contenedor el RSS máximo del proceso completo fue
460 176 KiB (≈ 449 MiB) durante la fase más exigente, que incluye el barrido de
candidatos y hasta 4 verificaciones concurrentes de 19 MiB.

## 4. Interoperabilidad con el backend Java

```json
{
  "status": "PASS",
  "direction": ["java_bcrypt_to_rust", "rust_argon2_to_java"],
  "policies": { "rust": "lab-argon2-v1", "java": "Argon2PasswordEncoder(16,32,1,19456,2)" }
}
```

Comprobaciones ejecutadas con contraseñas sintéticas y costes BCrypt 10 y 12:

| Comprobación | Resultado |
| --- | --- |
| Rust verifica un BCrypt generado por Java | `VERIFIED_REHASH` |
| Rust rechaza ese BCrypt con otra contraseña | `REJECTED` |
| Java verifica un Argon2id generado por Rust | `true` |
| Java rechaza ese Argon2id con otra contraseña | `false` |
| Prefijo BCrypt legado (`$2a$`) | Confirmado |
| Formato PHC (`$argon2id$v=19$`) | Confirmado |

Esto demuestra que la migración BCrypt → Argon2id puede hacerse **sin conocer la
contraseña en claro** fuera del instante de autenticación: el backend Java sigue
emitiendo hashes verificables y Rust puede rehashear dentro de la misma
transacción de login (`credential_lab::authenticate_and_upgrade`, ya probado).

## 5. Recomendación para D-AUTH-09 (propuesta, no aprobada)

1. **Política comercial candidata:** `Argon2id, v=19, m=19456 KiB, t=2, p=1`,
   salida 32 bytes, sal 16 bytes. Coincide con el mínimo recomendado por OWASP y
   con `LAB_V1`, que ya está probado de extremo a extremo.
2. **Mantener el techo en 32 768 KiB** mientras el hardware objetivo sea
   desconocido: 19 MiB × 3 logins simultáneos ≈ 57 MiB, un valor razonable en
   equipos de 4 GB.
3. **`p = 1`**: los equipos pequeños tienen 2–4 núcleos; aumentar carriles
   multiplica memoria por verificación sin mejorar la latencia percibida.
4. **Renombrar** la política aprobada (por ejemplo `argon2id-v1`) al promoverla:
   un identificador `lab-*` en producción sería una etiqueta engañosa.
5. **No** usar el valor por defecto de Spring (46 MiB) salvo que el benchmark en
   hardware objetivo demuestre que el pico de memoria es aceptable.

## 6. Criterios de aceptación propuestos

D-AUTH-09 puede aprobarse cuando exista evidencia de:

| Criterio | Estado 1Z |
| --- | --- |
| Benchmark reproducible con JSON verificable | **Cumplido** |
| Interoperabilidad bidireccional con el verificador Java | **Cumplido** |
| Rehash atómico sin contraseña en claro persistida | **Cumplido** (pruebas 1H existentes) |
| Medición en hardware objetivo de PyME | **Pendiente** |
| Latencia de verificación ≤ 250 ms en ese hardware | **Pendiente** |
| Memoria por login concurrente documentada para ese hardware | **Pendiente** |
| Comportamiento de bloqueo y errores bajo carga concurrente | Parcial (0 fallos; sin medición en hardware objetivo) |
| Aprobación explícita del titular | **Pendiente** |

Mientras falten las filas marcadas como pendientes, `D-AUTH-09` permanece
bloqueada y ninguna configuración `LAB_*` se considera política comercial.

## 7. Reproducir en el equipo objetivo

```sh
cd backend-rust
POS_BENCH_CPU="modelo exacto" POS_BENCH_MEMORY_KIB=<KiB> \
POS_BENCH_HARDWARE_CLASS=TARGET_SMALL_BUSINESS \
cargo run -p pos-credentials --release --example argon2_bench -- \
  --rounds 10 --workers 1,2,4 --json target/argon2-target.json
```

El JSON resultante es la evidencia que falta. No requiere red ni Java; la
interoperabilidad ya se ejecutó y solo debe repetirse si cambia la versión de
Spring Security del backend.
