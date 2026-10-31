# Clasificación de los 30 casos normativos — Incremento 1Z

**Ningún caso cambia de estado en 1Z.** La clasificación se conserva y se añade
la evidencia parcial nueva que alimentará la activación cuando el esquema esté
autorizado. `--require-complete` sigue devolviendo `2`.

Total actual: **1 PASSED (PAR-08), 28 BLOCKED, 1 NOT_RUN**.

## 1. Clasificación

| Categoría | Casos | Motivo |
| --- | --- | --- |
| Aprobados | `PAR-08` | Exportador determinista sobre dos instalaciones V1–V25, 37/37 y digest funcional igual |
| Ejecutables pendientes de evidencia (sin cambio normativo) | `DDL13-05`, `DDL13-06`, `DDL13-09`, `DDL13-11` | La prueba existe en subconjunto; falta el esquema completo para que cuente como normativa |
| Bloqueados por decisiones normativas | `PAR-05`, `AS-09`, `DDL13-04`, `DDL13-08`, `DDL13-10`, `PAR-01`–`PAR-04`, `PAR-06`, `PAR-07`, `PAR-09`, `AS-01`–`AS-08`, `AS-10` | Requieren target importado o credenciales/sesiones persistidas sobre `0002`–`0015`, aún candidatas |
| Bloqueados por diferencias físicas | `DDL13-01`, `DDL13-02`, `DDL13-03` | Los roots Local/Central/Control son subconjuntos; F1–10 declara 82 fichas y `unit_cost` incompatible (ADR-0021) |
| No implementados | — | No quedan casos sin instrumento; lo que falta es cobertura, no código nuevo |

`DDL13-07` permanece bloqueado por dependencia del target completo. `DDL13-10`
sigue NOT_RUN: `cargo --offline` no acredita red realmente denegada.

## 2. Evidencia nueva de 1Z que alimentará PAR-05 y AS-09

| Caso | Qué exige | Aporte 1Z | Estado |
| --- | --- | --- | --- |
| `PAR-05` | Estados de usuario, bloqueo, hash y política aprobada, rechazo seguro | Benchmark Argon2id reproducible en dos entornos, interoperabilidad BCrypt↔Argon2id PASS, rehash atómico ya probado en 1H | **BLOCKED** (falta target persistido y política aprobada) |
| `AS-09` | Hash/lock persistidos y rehash atómico conformes | Igual que PAR-05, más la evidencia de que la contraseña en claro nunca se persiste | **BLOCKED** (misma causa) |

Ninguno de los dos se marca aprobado: medir la criptografía no crea las tablas ni
la política de producto.

## 3. Qué haría avanzar cada bloqueo

| Bloqueo | Acción mínima |
| --- | --- |
| `0002`–`0007` sin promover | Aprobación del bloque A del paquete de decisión |
| `0008`–`0015` sin promover | Aprobación de los bloques B y C |
| Roots incompletos | Completar las fichas físicas y resolver ADR-0021 |
| Target import inexistente | Autorizar el import sintético versionado sobre el esquema aprobado |
| D-AUTH-09 | Benchmark en hardware objetivo + aprobación del titular |

Una vez promovidas las migraciones autorizadas, `DDL13-05/06/09/11` pueden
ejecutarse sobre el conjunto completo y, si el resultado es verde, pasar a
PASSED con evidencia exacta.

## 4. Regla de registro

Un caso solo cambia de estado cuando existe una ejecución registrada con salida
verificable. Este documento no adelanta estados ni convierte evidencia parcial en
aprobación.
