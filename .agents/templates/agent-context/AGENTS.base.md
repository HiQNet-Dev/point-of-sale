# AGENTS.md (base universal)

Reglas universales para agentes que trabajan en un repositorio. Copia este
bloque o fusiona sus secciones en el `AGENTS.md` raíz del proyecto y añade sólo
las reglas específicas del proyecto debajo.

## Descubrimiento incremental

1. **Git primero.** Antes de editar, revisa `git status` y el diff de los
   archivos afectados. Conserva cambios y configuraciones existentes.
2. **Mapa antes que exploración.** Empieza por el mapa de contexto
   (`docs/agent-context/PROJECT_MAP.md` o equivalente). No recorras el repo
   completo en cada sesión.
3. **Búsqueda dirigida.** Localiza símbolos con LSP o MCP de símbolos y lee sólo
   la implementación, las interfaces y las pruebas relacionadas. Amplía la
   búsqueda únicamente si falta evidencia.
4. **No re-descubras.** Reutiliza el mapa y los documentos canónicos.
5. **Mapa mantenible.** Actualízalo sólo cuando cambien de forma estable la
   arquitectura, los comandos o las fuentes de verdad. No es bitácora ni copia
   del repositorio.
6. **Salida concisa.** Entrega un plan breve y un resumen final de archivos,
   validaciones y riesgos. Sin razonamiento interno extenso.

## Cambios

- Haz el cambio mínimo coherente; respeta arquitectura, convenciones y
  dependencias existentes.
- No introduzcas abstracciones, capas ni dependencias sin requisito real.
- No reescribas código no relacionado.
- Actualiza la documentación o el contrato afectado en el mismo cambio.

## Seguridad y datos

- Nunca expongas ni commitees credenciales, `.env`, tokens, datos reales ni logs
  sensibles.
- Trata auth, permisos, entrada, archivos, consultas y ejecución de comandos
  como superficies sensibles.
- No configures modelos, proveedores ni credenciales por suposición.

## Validación

- Ejecuta sólo los checks relevantes (tests, lint, typecheck, build).
- Añade regresión práctica para bugs corregidos.
- Si algo falla, no lo ignores ni lo marques como skip.

## Git

- No hagas commit, push, force-push ni reescribas historia publicada sin
  petición explícita.
- No hagas deploy, cambios de infraestructura ni operaciones destructivas sin
  autorización.
