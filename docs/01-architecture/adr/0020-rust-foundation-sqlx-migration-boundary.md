# ADR-0020: foundation Rust y frontera de migraciones SQLx

## Estado

Registro de implementación parcial de decisiones **ya aceptadas por F4–21 y
F4–18**; no es una nueva aprobación de producto ni de DDL-13. Fecha: 2026-10-27.
No modifica retrospectivamente ADR-0001..0019 ni habilita datos productivos.

## Contexto y autoridad

ADR-0002 describía el único backend existente: Java/PostgreSQL/Flyway. F4–21
ratifica Rust local-first, SQLite STRICT y Tauri/IPC preferido para Free Local;
F4–18 limita Flyway al legado y establece SQLx para los nuevos esquemas. Los
párrafos históricos de propuestas anteriores no invalidan esa ratificación.

Fuentes consultadas, sin reproducir el expediente confidencial:

- [F4–21](https://docs.google.com/document/d/1tIHbbRw5GhKOFkL4ff0Zoc-SmMc-q7J2WwyYXFjqiUE/edit).
- [F4–18](https://docs.google.com/document/d/1IqeQSJlpzaf7qEsA1tcFIL1hVMa1H8lzc6IAarHKzlw/edit).
- [F4–06](https://docs.google.com/document/d/1ykfdYUucRXT-gifnxO3Yk0g1LXW7TaL2DCYMUZcOulc/edit).
- [F1–10](https://docs.google.com/document/d/1JXOYcHnfZfL4KmgUP_QkXxTikK_cy_h-Juowe7JAQqw/edit).
- [F1–13](https://docs.google.com/document/d/1ZtDnxOeVjeBJD20gLI1GxA4cTY-bevRoL2fKUMqZBPk/edit).

## Decisiones materializadas

1. Agregar `backend-rust/` sin sustituir `backend/` ni `frontend/`.
2. Mantener `pos-core` sin dependencias de transporte/persistencia. Crear solo
   adaptador SQLite, proceso de prueba y herramienta de laboratorio necesarios.
3. Separar `migrations/local`, `central` y `control`, cada uno desde `0001`, con
   historial `_sqlx_migrations` propio cuando se ejecute. No convertir ni copiar
   `flyway_schema_history`; no editar V1–V25.
4. No abrir HTTP local durante esta foundation. La elección de transporte desktop
   se implementará por separado sin contradecir la preferencia Tauri/IPC.
5. El DDL inicial es un subconjunto explícito de F1–10, no los 83 candidatos.
   La CI ejecuta `--require-complete`: los casos pendientes **bloquean merge**.
6. Mantener checkout, auth, importer, entitlements y sync sin implementar antes
   que crear fallbacks o apariencias de compatibilidad.

## Consecuencias y verificación

El núcleo exacto y el lab pueden evolucionar de forma aditiva; los fixtures no
se confunden con entidades del producto. El incremento 0b añadió PostgreSQL
17.11 nativo (19 checks), origen Flyway V1–V25 con 37/37 tablas y digest
determinista (PAR-08 PASSED) y AUTH-MIG sintético (8 checks). La prueba de backup
técnico no aprueba IMP-05 ni la restauración de instalaciones reales. La CI
técnica puede pasar, pero el gate normativo sigue bloqueado por 29 casos, el
físico incompleto y ADR-0021. No hay release ni cutover.

Ver [informe de foundation](../../05-development/rust-foundation.md) para tablas,
resultados y bloqueos. Una contradicción de Money, scope, identidad, estados o
credenciales exige change control y actualización del freeze; este ADR no la
resuelve silenciosamente.
