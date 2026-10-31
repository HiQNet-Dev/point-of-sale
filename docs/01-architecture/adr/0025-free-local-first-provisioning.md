# ADR-0025 — Autorización del primer provisioning Free Local

**PROPOSED. Requiere decisión expresa del titular.** Incremento 2D.
No cambia el freeze, no aprueba A/D-AUTH-09 y no activa comandos comerciales.
La aceptación previa de D y CC21-03 conserva exactamente su alcance.

## Contexto y límite del código actual

`authorize_bootstrap` emite `BootstrapProvisioning` después de verificar un secreto
de despliegue. Una base sin usuarios no demuestra que quien llama sea su titular.
Free Local no puede reemplazar el secreto por un botón, un actor fijo o una variable
pública del renderer. Tampoco debe requerir un servidor HTTP o un servicio Pro.

2D conecta provisioning y contraseña inicial comprobada dentro de la misma UoW.
Eso prueba integridad de los efectos, **no la autoridad del operador que empieza**.
El producto sigue cerrado; no se implementa un emisor de tokens o bypass comercial.

## Modelo de amenazas común

- Renderer comprometido, llamada IPC fabricada o sitio remoto: no deben producir
  capacidad, elegir rutas SQLite ni suministrar actor/grants como autoridad.
- Otro usuario local: ACL/directorio privado y canal nativo deben impedir leer o
  reemplazar token, SQLite y ficheros auxiliares. La ACL efectiva está pendiente.
- Operador sin autorización, replay y dos procesos iniciales: deben fallar cerrados
  aunque cambien IDs, organización o nombre de archivo.
- Interrupción antes/después del commit: no debe dejar un administrador sin
  contraseña ni consumir el derecho de provisioning sin completar los efectos.
- Malware con control de la cuenta propietaria o administrador del SO puede
  modificar SQLite/memoria. Este diseño no promete protección contra ese atacante
  sin controles externos de SO/cifrado/arranque; tampoco convierte la huella pública
  en una prueba de hardware o una barrera contra clonación.

## Opciones evaluadas

| Propiedad | A — Asistencia local nativa | B — Token externo del instalador autorizado |
| --- | --- | --- |
| Iniciador | Titular/administrador que abre un asistente nativo autorizado | Proceso nativo de instalación/recuperación autorizado por el titular |
| Prueba de autoridad | Identidad/consentimiento verificados por el SO y autorización de instalación; no un botón del WebView | Autorización del emisor y posesión del token entregado por canal administrativo externo al renderer |
| Generación | Nonce de 32 bytes CSPRNG para vincular la interacción/capacidad | Token de al menos 32 bytes CSPRNG, codificación sin pérdida; sin valores fijos |
| Custodia | Solo memoria del backend/canal nativo privado; sin localStorage, URL, logs o env pública | Entrega por pipe/canal administrativo protegido o soporte de instalación privado; persistir solo digest/verificador, no token crudo |
| Caducidad propuesta | Capacidad de proceso de corta duración, ligada al asistente y sesión SO | 10 minutos, ligado a instalación/nonce/finalidad; duración es propuesta, no política aceptada |
| Consumo | Uso único del derecho en la UoW del provisioning | Marcar consumido en la misma UoW que organización, contraseña, grants y auditoría |
| Replay | Rechazar nonce consumido, interacción cancelada y destino diferente | Rechazar token expirado/consumido, instalación/finalidad diferente y payload no autorizado |
| Interrupción | Antes del commit: rollback y nueva verificación nativa; después: instalación provisionada | Antes del commit: no consumir definitivamente, permitir reintento hasta expirar; después: consumido y bootstrap cerrado |
| Recuperación | Verificación administrativa de nuevo; no abrir el asistente por tabla vacía | Emisión administrativa nueva, revocación del token anterior y evidencia auditable; no reutilizar un token perdido |
| Instalación existente | Nunca crear otro administrador inicial ni reiniciar bootstrap | Igual; el token no puede transformar una instalación existente en nueva |
| Riesgo principal | Un diálogo nativo o UAC por sí solo no acredita titularidad; falta definir verificación real multiplataforma | Robo/copia del token o compromiso del emisor; requiere custodia y estado transaccional aprobados |

Para A no se acepta como prueba «el proceso se ejecuta bajo el usuario actual» sin
validación adicional. Debe definirse quién autorizó ese proceso y cómo se evita
que un renderer automatice o salte la comprobación. No se añade un allow-all a
`BootstrapSecretVerifier`.

Para B, «externo» significa fuera de la interfaz web, no un servidor obligatorio.
Puede emitirlo un instalador/helper nativo autorizado y offline. El token no entra
por DTO React ni se guarda en una variable `VITE_*`; el helper entrega la prueba al
backend por un canal privado. La UI solo puede aportar los datos y la contraseña
de configuración después de que el backend haya acreditado esa autoridad.

## Recomendación y decisión mínima solicitada

**Recomendar B para la primera implementación verificable**, condicionada a:

1. Aceptar quién es el emisor nativo autorizado y cómo acredita titularidad.
2. Aceptar binding a instalación/finalidad, expiración y recuperación administrativa.
3. Aprobar la representación física del digest, emisión/expiración/consumo y sus
   controles de acceso. `0007` no contiene hoy ese expediente de token: no se
   introduce como JSON arbitrario en auditoría ni se afirma que ya exista.
4. Cerrar ACL/TOCTOU y coordinación de herramientas antes de custodiar el token.
5. Registrar consumo y bootstrap en la **misma** UoW; no consumir en una base/pool
   separado y después intentar crear la organización.

No se solicita volver a aprobar D o CC21-03. Elegir B no aprueba A, D-AUTH-09,
las migraciones, distribución, merge, cutover o release. A queda como alternativa
si el titular elige una verificación nativa distinta con autoridad demostrable.

### Tiempo, secretos y recuperación

- Usar tiempo monotónico para la ventana del proceso y UTC persistido para
  recuperación; una regresión sospechosa del reloj debe bloquear y requerir
  recuperación, no alargar indefinidamente la autorización. Falta aprobar el
  criterio de detección del rollback del reloj.
- Comparar verificadores sin filtrar prefijos ni introducir diferencias visibles
  por token/usuario. Los errores públicos no incluyen email, hash, token o ruta.
- Los rechazos de autorización deben dejar evidencia de seguridad sin inventar
  un actor USER ni abusar de `bootstrap.completed` como evento de fallo SYSTEM.
  El catálogo de esos eventos preautenticación y su atribución siguen pendientes.
- Si se pierde el token antes del commit, solo una recuperación administrativa
  puede sustituirlo. Si existe bootstrap/identidad parcial, se bloquea y repara
  con procedimiento autorizado; no se vacían tablas ni se genera otra identidad.

## Política criptográfica comercial propuesta, aún no registrada

Versión propuesta: **`free-local-argon2id-v1`**. No figura en
`Argon2Policy::from_id`; hay una prueba que preserva este bloqueo. No es un alias
comercial de `LAB_V1` ni puede emplearse en el runtime sin revisión explícita.

| Elemento | Propuesta v1 | Aceptación/evidencia pendiente |
| --- | --- | --- |
| Algoritmo/formato | Argon2id v19, PHC interoperable | Conserva pruebas Java/Rust; no implica aprobación comercial |
| Parámetros | m=19456 KiB, t=2, p=1; sal CSPRNG 16 bytes, salida 32 bytes | Medición en hardware objetivo Free Local |
| Admisión | Máximo 4 verificaciones, cola 8, espera 2 s | Latencia/memoria/saturación y UI bajo carga representativa |
| Cuenta | Candidata: 5 fallos y enfriamiento 300 s; persistencia/CAS y reset tras éxito | Titular debe aceptar umbral, duración y resistencia a bloqueo malicioso |
| Instalación | Presupuesto global acotado y cola compartida para cuentas conocidas/desconocidas | Falta aprobar ventana/rate y persistencia; el semáforo solo limita concurrencia |
| Cuenta inexistente | Mismo error público y presupuesto criptográfico/admisión comparable | Verificación de timing/no enumeración y política de auditoría prelogin |
| Sesión | Handle CSPRNG, hash almacenado, expiración/revocación DB-authoritative | Aceptar TTL, invalidación tras cambios de credencial/rol y rehash/login atómico |

La prueba técnica de bloqueo de 2D inyecta 1 fallo/60 s para probar los límites sin
esperas: **no fija los parámetros de producto**. Se conserva BCrypt legado solo
para verificación/transición autorizada, no para crear una contraseña inicial nueva.

Falta ejecutar la medición aislada ya disponible en un equipo PyME representativo;
un contenedor limitado o el equipo de desarrollo no sustituyen ese requisito.
D-AUTH-09 sigue bloqueada hasta evidencia y aceptación del titular.

## Estado de implementación y siguiente activación

Implementado: prueba criptográfica obligatoria y no serializable para la contraseña
inicial; persistencia atómica junto a bootstrap y auditoría; fallos por etapa,
concurrencia y recuperación. Preparación de snapshot/restauración verificada,
sin comandos administrativos expuestos. El backend no carga propuestas en producto.

Pendiente antes del primer arranque comercial: elección A/B, expediente/token o
verificación SO real, aprobación A/D-AUTH-09, política comercial registrada,
composición de login/sesiones, ACL y transporte/ACLs explícitos. La pantalla actual
informa backend de identidad deshabilitado, no simula una sesión o un asistente.
