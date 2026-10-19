# PROJECT_MAP — <proyecto>

Índice de contexto para agentes. No es una copia del repo ni una bitácora:
sólo apunta a las fuentes de verdad. Actualízalo únicamente cuando cambien de
forma estable la arquitectura, los comandos o las fuentes de verdad.
Objetivo: menos de 150 líneas.

## Propósito y stack

<Qué hace el proyecto y, en una tabla, capa / tecnología / raíz.>

## Rutas de entrada por tipo de tarea

| Tarea | Empieza por |
| --- | --- |
| <Feature backend / frontend / etc.> | `<ruta/>` |
| <Contrato de API> | `<ruta/>` |
| <Migración / esquema> | `<ruta/>` |
| <Seguridad / permisos> | `<ruta/>` |
| <Pruebas> | `<ruta/>` |
| <Entorno local / CI> | `<ruta/>` |

## Documentos y archivos canónicos

| Tema | Fuente |
| --- | --- |
| Reglas obligatorias | `AGENTS.md` |
| Arquitectura | `<ruta>`, `docs/.../adr/` |
| Contrato / API | `<ruta>` (fuente de verdad) |
| Esquema de datos | `<ruta>` |
| Seguridad | `<ruta>` |
| Estilo de código | `<ruta>` |
| Flujo Git / releases | `<ruta>` |
| Build / CI | `<manifests>` |

## Módulos / features actuales

<Lista breve de módulos y su paquete o carpeta. Sin detallar implementación.>

## Comandos de validación (comprobados)

```bash
# <backend>
<comando verificado>
# <frontend>
<comando verificado>
```

No ejecutes la suite completa para cambios que sólo son de configuración o
documentación; usa el check específico y justifícalo.

## Herramientas de símbolos disponibles

<LSP, indexador o MCP de símbolos disponible y para qué usarlo.>

## Reglas críticas (consultar la fuente, no copiar aquí)

<3-6 reglas con enlace al documento que las define.>
