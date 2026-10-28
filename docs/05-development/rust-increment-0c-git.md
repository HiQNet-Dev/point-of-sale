# Incremento 0c — reconciliación Git y propuesta de protección

**Fecha de consulta: 2026-10-28. Estado: hechos observados y propuesta no aplicada.**
Este informe corrige las afirmaciones de publicación de 0b sin atribuir personas,
intención, permisos históricos ni aprobaciones que no pueden demostrarse.

## 1. Checkout y refs reconciliados antes de editar

- `git status --short`: vacío; ningún cambio local que sobrescribir.
- Rama inicial: `feat/backend/rust-foundation-migration-lab`.
- HEAD y `origin/dev`, tras `git fetch origin` correcto:
  `6a15ee6a21ef80c17ab7276b337946cbcc4a4d37`.
- `dev` local permanecía en `ebf7a30`, seis commits detrás. No se movió esa rama.
- `main` local y remoto no se modificaron; no hay reset, rebase ni cherry-pick.
- `git diff --check`: correcto. Diff del baseline Java→HEAD sobre
  `backend/`, `frontend/`, `docker-compose.yml`: vacío. Flyway V1–V25 intactas.
- `Cargo.lock` versionado; raíces SQLx preexistentes preservadas.
- Después de reconciliar: `git switch -c feat/backend/rust-physical-resolution origin/dev`.
  No se creó otro checkout ni se reutilizó la rama antigua para integrar duplicados.

## 2. Los seis commits, no seis entregables nuevos

| Commit    | Padre     | Propósito observado                 | Evaluación de integración                                                   |
| --------- | --------- | ----------------------------------- | --------------------------------------------------------------------------- |
| `4236fab` | `ebf7a30` | Foundation Rust                     | Ya alcanzable desde `origin/dev`; no repetir.                               |
| `7de03ed` | `4236fab` | Migration Lab                       | Ya integrado; el esquema sigue parcial.                                     |
| `c995bf8` | `7de03ed` | CI Rust/gate obligatorio            | Ya integrado; bloqueo funciona, no fue un fallo técnico de compilación.     |
| `6567f87` | `c995bf8` | Informe/evidencias 0b               | Ya integrado; afirmaciones de ausencia de publicación requieren corrección. |
| `f120de8` | `6567f87` | Referencias documentales de commits | Ya integrado; no añade funcionalidad.                                       |
| `6a15ee6` | `f120de8` | Referencia final documental         | Tip remoto confirmado; no nuevo baseline funcional Java.                    |

Todos son commits de un padre, cadena lineal sin merge commit en este intervalo.
La API `GET /repos/HiQNet-Dev/point-of-sale/commits/{sha}/pulls`, consultada para
**cada uno**, devolvió **0 PR asociados**. `gh pr list --state open` devolvió `[]`.
Eso no prueba que nunca existiera un PR cerrado ajeno a esos commits, pero no hay
PR de integración asociado observado. El reflog local de `origin/dev` registra:
`2026-10-28 00:03:50 -0600: update by push`. El run remoto tiene evento `push` a
dev y ese SHA. En conjunto son evidencia de avance/publicación directa del ref,
no prueba de qué persona, herramienta o excepción administrativa lo hizo.
No se consultó un audit log de organización para atribuir responsables.

## 3. CI remota real

[Run 40641592100](https://github.com/HiQNet-Dev/point-of-sale/actions/runs/40641592100),
evento `push`, SHA completo anterior, estado `completed`, conclusión **failure**.

| Job                                              | Conclusión API | Alcance de la afirmación                                                                                                                                    |
| ------------------------------------------------ | -------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Detect Changes                                   | success        | Detección y tests del workflow publicado.                                                                                                                   |
| Backend (compile, test, package)                 | success        | Validación Java del SHA remoto; no nueva ejecución Java local 0c.                                                                                           |
| Frontend (install, lint, typecheck, test, build) | success        | Regresión React del SHA remoto.                                                                                                                             |
| Validate docker-compose                          | success        | Validación Compose remota.                                                                                                                                  |
| Rust (technical checks + mandatory DDL13 gate)   | failure        | Único step fallido: `Require complete normative migration evidence (blocks merge)`. Los steps fmt/clippy/test/build/lab/audit y verificador fueron success. |
| CI Gate                                          | failure        | Step `Verify required jobs` propagó el fallo.                                                                                                               |
| POS integrated E2E (Chromium + PostgreSQL)       | skipped        | No se ejecutó; conforme al filtro de `dev`, no significa aprobado.                                                                                          |

Artefacto publicado según API: `12795844075`,
`rust-migration-technical-and-normative-evidence`, no expirado al consultar,
digest declarado `sha256:0b76d121feb8ee4bceaaf1857a27d28d5d4339b91a3206efbe0474f9772271c0`.
Se descargó en `backend-rust/target/increment0c-remote-40641592100/` y el
verificador reforzado 0c aceptó ambas salidas: técnica con exit esperado 0 y
gate con exit esperado 2; ambas muestran 41 técnicos, 1 aprobado y 29 pendientes,
con siete SHA-256 de snapshots comprobados. No es un run nuevo de 0c ni paridad
completa. El digest declarado de API no se presenta como firma del aprobador.
Un segundo intento de extracción rehusó sobrescribir los archivos existentes;
se conservaron los de la primera descarga.

### Corrección del reporte anterior

El bloqueo registrado de **un intento** de push de feature no permite concluir
«no publicado / sin CI». La afirmación global de 0b es incorrecta respecto del
estado actual. Se conserva la distinción entre evento observado entonces y estado
remoto demostrado ahora; no se especula sobre acciones intermedias. Tampoco se
confunde la suite Java local bloqueada con el job remoto exitoso.

## 4. Política incumplida y causa observable de falta de enforcement

Respecto de AGENTS/CONTRIBUTING/git-workflow y F4–21 IMP-01:

- El trabajo de feature quedó en `dev` sin PR asociado observado, sin squash de
  feature vía PR y sin CI/gate normativo verde del estado integrado.
- El primer esquema Local parcial ya está en integración aunque IMP-01 y el
  gate completo documentado siguen bloqueados. Que CI falle no revierte el push.
- No hay evidencia de autorización de excepción de release/sincronización.
  No se atribuye responsabilidad ni se afirma que existiera un bypass.

Lecturas API (sin operaciones PUT/PATCH/POST/DELETE de configuración):

| Consulta                                          | Resultado observado                                                      |
| ------------------------------------------------- | ------------------------------------------------------------------------ |
| `branches/dev/protection`                         | HTTP 404, mensaje explícito `Branch not protected`.                      |
| `rulesets` (incluye padres por defecto según API) | `[]`.                                                                    |
| `rules/branches/dev`                              | `[]`: ninguna regla **activa** aplicable, incluidas las de organización. |

El último endpoint no muestra reglas disabled/evaluate; no se infiere su ausencia
histórica. La ausencia actual de protección explica que el workflow por sí solo
no impida actualizar `dev`. No prueba cómo estaba configurado al ocurrir el push.

## 5. Propuesta sobre `dev` rojo — requiere decisión del titular

| Alternativa                                                       | Ventaja                                                                               | Riesgo / procedimiento                                                                                                                                                                                                                           |
| ----------------------------------------------------------------- | ------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| A. Conservar foundation experimental en dev temporalmente         | Mantiene continuidad y evita un revert/reapply grande. Java/React remotos correctos.  | Integración no apta para release; rojo normativo debe seguir visible. Detener promoción a main y merges de esquema; resolver ADR y gates en ramas. Un PR docs-only verde no sanea los gates pendientes del árbol.                                |
| B. Restaurar integración verde con PR de reversión no destructiva | Vuelve a retirar del árbol integrado el conjunto experimental hasta cerrar sus gates. | PR revisado que revierta coherentemente los seis commits en orden inverso, conservándolos en historia/feature; comprobar Java/React/Compose/E2E. Reintroducir después mediante PR completo con gate, no duplicar cherry-picks ni quitar solo CI. |

**Recomendación provisional: A**, únicamente si el titular acepta explícitamente
un período de integración experimental y sin release. Si `dev` debe permanecer
siempre verde, **B es la alternativa correcta** y requiere autorización para el PR
de reversión. No se ejecutó ninguna reversión ni se modificó el significado de PASS.
Ni feature flag ni docs-only sustituyen el requisito de esquema. Esta propuesta
no convalida retrospectivamente la integración anterior.

## 6. Protección recomendada — NO APLICADA

1. Ruleset activo para `refs/heads/dev` y protección separada para `main` conforme
   a releases/hotfix. Exigir PR antes de actualizar; sin push directo ni bypass
   rutinario de administradores/apps. Excepciones de emergencia auditadas y acotadas.
2. Exigir **al menos una aprobación humana independiente**, invalidar aprobación
   al cambiar el diff y exigir aprobación del último push por alguien distinto.
   Si falta un segundo revisor, resolver organización del equipo, no autoaprobar.
3. `CI Gate` obligatorio, del emisor GitHub Actions verificado y con rama al día;
   no usar nombres ambiguos ni depender solo de jobs condicionales. El gate debe
   seguir rechazando Rust omitido cuando corresponde.
4. Conversaciones resueltas, bloquear force-push y eliminación. Mantener squash
   para features. Evaluar CODEOWNERS con equipos reales para migraciones/seguridad;
   no inventar equipos ni activarlo sin responsables disponibles.
5. No exigir historia lineal global sobre `main` sin evaluar los merge commits
   de release ya convencionados. No activar merge queue sin soporte `merge_group`
   en workflow y pruebas; no es necesario para resolver este incidente.
6. Tras autorización/configuración, verificar con las APIs anteriores y PR de
   prueba controlado que rojo/review ausente impiden merge. No probar con push
   directo real a dev/main ni debilitar temporalmente los checks.

Fuentes oficiales consultadas:
[Protected branches](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-protected-branches/about-protected-branches),
[Branch protection REST](https://docs.github.com/en/rest/branches/branch-protection#get-branch-protection),
[Rules for branch REST](https://docs.github.com/en/rest/repos/rules#get-rules-for-a-branch).
Configuración remota, aprobación ADR y política sobre dev requieren autorización
separada; no se consideran concedidas por solicitar la investigación.
