# Política de seguridad de HiQNet POS

## Reportar una vulnerabilidad

**No abras issues públicos con detalles de vulnerabilidades.** Escríbenos a la
organización HiQNet (canal privado) con:

- descripción del problema;
- pasos de reproducción;
- impacto estimado;
- versión afectada (ver `VERSION`).

Nos comprometemos a:

- confirmar la recepción en tiempo razonable;
- mantener la confidencialidad mientras se investiga y corrige;
- publicar el detalle únicamente tras la corrección si procede.

## Principios de seguridad

La base: **deny by default** y **least privilege**. Detalle completo en
`docs/04-security/security-model.md`. Resumen:

- Autorización siempre server-side; nunca confiar en el frontend.
- Scoping por organización/sucursal en cada operación; sin IDs mágicos ni fallbacks.
- Secretos solo por variables de entorno; nada en Git (`.env`, tokens, credenciales).
- Operaciones monetarias transaccionales, auditadas y sin hard delete.
- Sin credenciales de desarrollo reutilizadas fuera del entorno local.

## Alcance

Este repositorio mantiene `0.1.0` (release técnica privada de la Foundation;
pre-1.0, no comercial). Las vulnerabilidades se tratan igual que en etapa
estable: reporta y corregimos antes de release comercial.