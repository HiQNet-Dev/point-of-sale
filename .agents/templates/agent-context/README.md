# Templates de contexto para agentes

Fragmentos portables para preparar otro proyecto (nuevo o existente) sin
depender de este repositorio. No contienen secretos ni rutas absolutas.

| Archivo | Uso |
| --- | --- |
| `AGENTS.base.md` | Reglas universales. Fusiona sus secciones en el `AGENTS.md` raíz del proyecto y añade las reglas específicas debajo. No lo sobrescribas si ya existe. |
| `PROJECT_MAP.template.md` | Estructura vacía para crear `docs/agent-context/PROJECT_MAP.md` en el proyecto destino. |
| `zed-profiles.json` | Fragmento de `agent.profiles` de Zed. Fusiónalo dentro de `agent` en `.zed/settings.json` preservando LSP, tasks y otras claves existentes. |

## Cómo usarlos

1. Invoca la Skill `project-agent-bootstrap` (global). Sigue su flujo.
2. Si el proyecto tiene su propio `AGENTS.md`, añade sólo lo que falte.
3. Copia `PROJECT_MAP.template.md` como `docs/agent-context/PROJECT_MAP.md` y
   rellénalo con comandos realmente comprobados.
4. Para Zed, parte de `zed-profiles.json` y fusiona; no reemplaces el archivo
   completo. Si `.zed/` está ignorado por Git, no cambies el ignore.
5. En OpenCode, verifica primero los agentes globales
   (`opencode debug agents`) para no colisionar con `.opencode/agents/<nombre>.md`.
