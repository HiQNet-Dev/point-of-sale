# D-AUTH-09 — Medición Argon2id en hardware Windows (2026-10-31)

**Estado: MEDIDA, pendiente de ratificación del titular.**

Este documento registra la medición reproducible que faltaba para resolver
D-AUTH-09. No cambia la política de nuevas credenciales: el provisioning sigue
usando `lab-argon2-v1` hasta que el titular acepte la política propuesta.

## Entorno

| Elemento | Valor |
| --- | --- |
| Sistema | Windows (x86_64) |
| CPU | AMD64 Family 25 Model 80 Stepping 0 (12 CPU lógicas) |
| Rust | 1.99.0 / herramienta `pos-credentials` `argon2_bench` |
| Comando | `cargo run -p pos-credentials --release --example argon2_bench -- --rounds 10 --workers 1,4 --burst 16 --json target/argon2-bench-windows.json` |
| Límites externos | `UNSPECIFIED` (no se aplicó cgroup ni afinidad) |

## Resultados (ruta pública `hash_password` / `verify_credential`)

| Política | m (KiB) | t | p | verify p95 (ms) | hash p95 (ms) |
| --- | --- | --- | --- | --- | --- |
| `lab-argon2-v0` | 8 192 | 1 | 1 | 9,4 | 9,3 |
| `lab-argon2-v1` | 19 456 | 2 | 1 | 17,7 | 17,7 |
| `cap-32m-2t-1p` | 32 768 | 2 | 1 | 30,2 | 37,8 |
| `cap-32m-3t-1p` | 32 768 | 3 | 1 | 51,1 | 49,7 |
| `spring-default-46m-1t-1p` | 47 104 | 1 | 1 | 27,2 | 26,3 |
| `strong-64m-3t-1p` | 65 536 | 3 | 1 | 102,3 | 101,7 |

Concurrencia: con 4 trabajadores la política V1 sostuvo ~129 operaciones/s con
p95 de 34,6 ms; la saturación es una decisión de admisión de la aplicación, no
del algoritmo.

## Propuesta

`local-argon2-v2` = **32 768 KiB, t = 3, p = 1** (`verify` p95 ≈ 51 ms, `hash`
p95 ≈ 50 ms en este equipo).

Motivos:

- Iguala o supera el mínimo recomendado por OWASP (19 MiB, t=2).
- Cabe dentro del techo técnico vigente de `Argon2Policy` (32 MiB, 3 iteraciones,
  2 carriles), sin relajar límites.
- 51 ms es aceptable para un login interactivo y queda lejos de una latencia que
  invite a desactivar el coste.
- Es más fuerte que `spring-default` (46 MiB, t=1) en iteraciones y similar en
  memoria.

## Qué falta para aceptarla

1. Repetir la medición en al menos una clase de hardware adicional (equipo de
   gama baja objetivo) y registrar la clase en el JSON.
2. Decidir si las credenciales `lab-argon2-v1` existentes se rehashean en el
   siguiente login exitoso (`VerificationOutcome::Verified { rehash_required }`
   ya lo detecta; el rehash no se aplica todavía).
3. Ratificación explícita del titular para promover `local-argon2-v2` como
   política comercial de nuevas credenciales.

## Reproducción

```sh
cd backend-rust
cargo run -p pos-credentials --release --example argon2_bench -- \
  --rounds 20 --workers 1,4,16 --burst 32 --json target/argon2-bench.json
```

Los parámetros de hardware se declaran con `POS_BENCH_CPU`, `POS_BENCH_MEMORY_KIB`
y `POS_BENCH_HARDWARE_CLASS`; los límites de CPU/RAM se aplican externamente.
