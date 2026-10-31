# Bloque A — matriz técnica de preparación 2D

**No aprobado. No promueve migraciones.** Complementa el expediente canónico 1I,
ADR-0022, el paquete 1Y y [ADR-0025 propuesto](../01-architecture/adr/0025-free-local-first-provisioning.md).
D ya está aceptado como diseño; esa aceptación no resuelve D-INST/D-AUTH físicos.

## Matriz de autoridad, discrepancias y controles

| Candidato | Contrato/decisiones | Implementación observada y diferencia relevante | Evidencia/condición de aprobación |
| --- | --- | --- | --- |
| 0002 installation_identity | F1–10 §16.3; D-INST-01/02 | UUID, FK a organización, generation>=1, estado ACTIVE. Catálogo completo de estados y ámbito/formato de huella pendientes; una huella pública no acredita hardware ni autorización | Aceptar catálogo/fingerprint y política de instalación; FK/ownership y reapertura probados, no una prueba de clonación imposible |
| 0003 pos_user | D-AUTH-01/11 | Organización obligatoria, email canónico y UNIQUE por org. `lower()` SQLite no representa Unicode como Java/Rust; la normalización autoritativa está en aplicación | Aceptar política Unicode y riesgo de escritura SQL directa; no restringir silenciosamente a ASCII |
| 0003 local_credential | F1–10 §16.5; D-AUTH-02/05/09 | PASSWORD/PIN, BCrypt/Argon2id, hash y parámetros separados; versión inicial 1 y ownership compuesto. Catálogo físico ACTIVE/PASSWORD_RESET_REQUIRED; dominio/lab conservan RESET_REQUIRED/DISABLED | Mapper canónico 2D traduce PASSWORD_RESET_REQUIRED; disabled se resuelve por pos_user. No añadir DISABLED físico. Parámetros comerciales requieren hardware/aceptación |
| 0003 local_session | F1–10 §16.45; D-AUTH-03/04/05 | Estado físico ACTIVE/REVOKED/EXPIRED, coherencia revoked_at, versión 1. Organización derivada de usuario, no nueva columna. Expiración temporal se evalúa aun si state=ACTIVE | Sesión expirada/revocada rechazada y RBAC DB-authoritative existentes; faltan TTL/invalidation/login comercial y atribución de eventos prelogin |
| 0004 RBAC | D-AUTH-06 | 59 permisos de V5–V25, roles y asignaciones same-org; no grants de cliente. Bootstrap canónico asigna catálogo en su UoW | Catálogo/grants y retiro de permisos probados. Usuario/rol administrativos conservan revisión 0: D-AUTH-05 fija 1 para credential/session, no para todos los agregados |
| 0005 memberships | D-AUTH-06 | FKs compuestas de usuario/sucursal/rol, pertenencia activa, override de organización; no promover grant de sucursal a organización | Mantener negativas cross-org, estado operativo y recuperación explícita; no allow-all ni membership de cliente como autoridad |
| 0006 audit_event | D-AUTH-07/08; ADR-0022 | USER/SYSTEM, session_id nullable, atribución same-user y triggers append-only. Bootstrap SYSTEM sin usuario/sesión; no es inmutabilidad criptográfica | Reversión de provisioning/credencial/grants ante fallo de auditoría. Aceptar atribución y catálogo preautenticación antes de login comercial |
| 0007 bootstrap_record | D-AUTH-10; ADR-0025 propuesto | PENDING/COMPLETED y FKs same-org; no contiene emisión/TTL/consumo de token externo. El esquema por sí solo no exige contraseña; 2D la hace obligatoria en Application y condiciona finalización a su persistencia | Pruebas de contraseña recuperable tras commit, IDs diferentes, concurrencia, rollback y abandono. Elegir autorización inicial; no usar tabla vacía como permiso |

Los timestamps físicos de credenciales/sesiones son texto UTC de segundos; el
adaptador compartido convierte a UtcSeconds sin floats. Los CHECKs de forma no
sustituyen validación semántica de entrada ni prueban historia económica o paridad
completa. No se alteraron las propuestas para ocultar estas discrepancias.

## Procedencia de archivos, sin promoción

SHA-256 de bytes de las propuestas auditadas (no son los checksums SHA-384 de
SQLx ni autorización de ejecución):

| Archivo | SHA-256 |
| --- | --- |
| `0002_installation_identity.proposed.sql` | `31e9fe3b44b9f087dfbb230fe7195036ffcf39b04f05bb8bac58f683f6a899d9` |
| `0003_user_credential_session.proposed.sql` | `c00f5c5a8ada931b31c41cc14a79ce9600bab08f7553e02b8e6d6624358cb4a1` |
| `0004_rbac_catalog.proposed.sql` | `b4817d9837a04e46fcfa2bfb586bba63f2e7acb83edbd567018048af016ad50a` |
| `0005_branch_membership.proposed.sql` | `80e41b76a364641bd247117283f0f8d583d48f58d5bab4839528b0d358325268` |
| `0006_audit_event.proposed.sql` | `d4219dc60f3e80cb0b19d3ee3fd9f8503ff7b6a371fc85e6171bcded246ff167` |
| `0007_bootstrap_record.proposed.sql` | `96dbb338125d7a250d5e35a2836f23091a60325c2536fa89308d1f7d4f97df3b` |

Solo `migrations/local/0001` está autorizado. Las pruebas aplican estas propuestas
manualmente en SQLite temporal; el migrador de producto no las descubre.

## Condiciones precisas para activar A

1. Aceptación expresa y trazable de D-INST y D-AUTH aplicables, incluida la elección
   de normalización y SYSTEM. Aceptar A no equivale a release/merge/distribución.
2. Decisión del primer provisioning según ADR-0025: autoridad, custodia, caducidad,
   consumo atómico y recuperación. Una elección de token exige aprobar su físico;
   no se simula con los campos actuales de 0007.
3. D-AUTH-09 con hardware objetivo, interoperabilidad y política comercial versionada
   registrada por un cambio explícito. No renombrar LAB_V1 ni usarlo automáticamente.
4. Composición canónica de credenciales/login, emisión/revocación/expiración de
   sesiones y auditoría, sin grants o identidades sintéticas.
5. Promoción selectiva solo del conjunto aceptado, respetando dependencias y bytes;
   clean install y upgrade desde 0001, SQLx checksums, FKs/constraints/índices,
   transacciones, concurrencia y reapertura. No alterar migraciones históricas.
6. ACL efectiva, validación de archivos, coordinación de herramientas/lock y
   procedimiento de backup/restauración autorizado antes de datos reales.
7. Contratos IPC, ACLs, React y negativas de autorización sobre ese runtime. CI
   normal en SHA final solo cuando el titular autorice ejecutarlo.

## Evidencia y límites de este incremento

2D añade credencial inicial verificada/persistida en la UoW de bootstrap,
fallos por etapa, verificación tras reinicio, bloqueo persistente y mapper de
estado canónico. No se emite una sesión comercial ni se cierra la política de
cuentas inexistentes/limites globales por tener primitivas de credenciales.

La aprobación de estas propuestas no sustituye los 30 casos DDL13/PAR/AS. Sin
nuevos targets/evidencia normativa no cambian los estados ni `--require-complete`.
