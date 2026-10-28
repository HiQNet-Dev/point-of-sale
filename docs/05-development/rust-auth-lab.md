# Rust auth lab — transición técnica sintética F4–19

## Alcance y fuente

Fuente leída mediante [F4–19, exportación TXT](https://docs.google.com/document/d/1TTiJo3tHgbEdB043mLQKTayi8zEmP_WIk0vdshP2Bgk/export?format=txt).
Las ratificaciones aceptan BCrypt → Argon2id tras login válido y excluyen las
sesiones y el secreto JWT Java. No aprueban parámetros comerciales: benchmark en
hardware objetivo, interoperabilidad y lockout siguen siendo gates técnicos.

`backend-rust/tools/migration-lab/src/auth.rs` es **solo un fixture ejecutable de
máquina de estados en memoria**, separado de `pos-core` y del dominio. No incorpora
endpoints, BD, importador completo, autenticación productiva, JWT ni sesiones
reales. Las sesiones locales son un contador de eventos autorizados, sin handles.
No lee variables de secretos, datos de usuarios, tokens ni URLs de servicios.

Su única API de integración es:

```rust
pub(crate) async fn run() -> Result<serde_json::Value, crate::LabError>;
```

Ejecuta el trabajo CPU-bound con `tokio::task::spawn_blocking`. El JSON contiene
`cases` **exactamente** `AUTH-MIG-01` … `AUTH-MIG-08`, estados `PASSED`/`FAILED`,
política y benchmark. Un fallo de un caso produce `FAILED`, no un éxito vacío.
Errores operativos del benchmark/worker se devuelven como `LabError::Invalid`
con códigos fijos; el runner los conserva como fallo técnico.

El resultado declara `scope: synthetic_technical`, `normative_status: BLOCKED`
y `as09_normative: false`. **No acredita AS-09 ni paridad normativa**, aunque
pasen todos los casos: falta el importador completo y su transacción persistente.
El runner ya registra `mod auth`, ejecuta el módulo y escribe
`auth-transition.json`; no lo convierte en evidencia normativa. La integración
no habilita autenticación productiva ni sesiones reales.

## Casos e invariantes

| ID          | Comprobación técnica                                                                                                                                                                                                                         |
| ----------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| AUTH-MIG-01 | Los 15 vectores Spring BCrypt válidos se verifican y reemplazan por Argon2id verificable; cambian algoritmo/policy/hash, `credential_version` y `state_version` aumentan una vez, failures vuelve a cero y se registra una sesión local.     |
| AUTH-MIG-02 | Password BCrypt incorrecta: conserva hash/algoritmo/policy/credential_version, aumenta failures y state_version; cero rehash y cero sesiones.                                                                                                |
| AUTH-MIG-03 | Argon2id vigente válido: conserva hash y credential_version; registra sesión sin rehash y actualiza state_version.                                                                                                                           |
| AUTH-MIG-04 | Argon2id con policy antigua reconocida: verifica primero y después reemplaza por la policy actual verificable, en un único commit versionado.                                                                                                |
| AUTH-MIG-05 | Lock futuro: rechazo antes del verificador costoso, sin rehash; conserva todos los campos, incluidos contador, lock y versiones.                                                                                                             |
| AUTH-MIG-06 | INACTIVE con BCrypt o Argon2id antiguo: rechazo antes de verificar; sin rehash, sesión ni mutaciones.                                                                                                                                        |
| AUTH-MIG-07 | Hash corrupto, algoritmo desconocido o policy desconocida: `ResetRequired` interno, equivalente al `PASSWORD_RESET_REQUIRED` de F4–19; sin sesión ni fallback. Conserva hash/contador; versiona el cambio de estado y repetirlo es un no-op. |
| AUTH-MIG-08 | No existe conversión de sesiones Java: un handle opaco sintético se rechaza incondicionalmente; importación sintética empieza con cero sesiones locales. Solo un login local válido registra una nueva.                                      |

AUTH-MIG-08 no fabrica ni certifica un JWT Java válido: no hay firma, claves,
refresh endpoint o validador Java. Comprueba la frontera de **no importación**
del fixture, no revocación persistente del sistema completo.

`prepare` trabaja sobre una copia privada. `commit` comprueba la versión esperada
y reemplaza el estado completo de una vez: hash, algoritmo, policy, versiones,
failures, lock y contador de sesiones. Un conflicto descarta todo, incluida la
sesión preparada. Se prueban dos upgrades competidores, desactivación competidora
y overflow sin commit parcial. No es una transacción SQL ni una prueba multihilo;
la implementación productiva necesitará CAS/transacción y revocación persistente.

La política de lockout es **solo del fixture**: cinco fallos, lock de 60 segundos,
reloj explícito determinista. Un lock futuro conserva contador; en su instante de
expiración permite verificar y un éxito limpia lock/failures. No se afirma paridad
completa con las reglas Java de producción.

## Vectores Spring, longitud y UTF-8

`fixtures/auth/spring-vectors.json` contiene únicamente material **sintético,
público y no reutilizable como credenciales reales**. Sus hashes fueron generados
independientemente del verificador Rust por Spring Security BCrypt **7.1.1**
(JAR de la caché Maven local), con `BCrypt.gensalt`, y comprobados con
`BCrypt.checkpw`: password correcta aceptada e incorrecta rechazada.

`fixtures/auth/GenerateSpringVectors.java` permite regenerarlos con ese JAR:

```sh
# Desde backend-rust/tools/migration-lab/fixtures/auth; imprime datos sintéticos.
java --class-path "$HOME/.m2/repository/org/springframework/security/spring-security-crypto/7.1.1/spring-security-crypto-7.1.1.jar" GenerateSpringVectors.java
```

Hay cinco entradas por variante `$2a$`, `$2b$`, `$2y$`: texto con caracteres no
ASCII y fronteras ASCII/UTF-8 de 71 y 72 **bytes**. El coste 4 mantiene rápidas las
pruebas; **no es un coste comercial aprobado**. Los salts de fixture los generó
Spring aleatoriamente; no son los salts utilizados para los upgrades Argon2id.

- BCrypt acepta hasta 72 bytes incluidos, sin normalización ni truncado manual.
- Entradas de 73/74 bytes y el caso de 71 bytes ASCII seguido de un carácter UTF-8
  multibyte se rechazan antes de verificar, sin cambiar estado/contador.
- Se rechaza NUL embebido; Argon2id tiene además un límite de entrada de 1024 bytes.
- `bcrypt 0.17.1::non_truncating_verify` cuenta su NUL añadido y también rechaza
  una entrada de exactamente 72 bytes. Por eso se usa `bcrypt::verify` **solo tras
  el gate explícito de ≤72 bytes**, probado contra Spring. Nunca se trunca una
  password de usuario para migrarla.
- No se aceptan `$2x$`, wrappers `{bcrypt}`, otros formatos o policies desconocidas.
  Requerirían vectores y decisión explícita; no se eliminan prefijos a ciegas.

## Política explícita y límites antes de verificar

| policy_id       | Uso                                      | m (KiB) |   t |   p |
| --------------- | ---------------------------------------- | ------: | --: | --: |
| `lab-argon2-v0` | Solo origen antiguo sintético reconocido |    8192 |   1 |   1 |
| `lab-argon2-v1` | Destino **EXPLICIT_LAB_ONLY**            |   19456 |   2 |   1 |

Ambas usan Argon2id v19, salida de 32 bytes y salt CSPRNG de 16 bytes. No se llama
`Argon2::default`/`Params::default`; cada operación recibe una policy explícita.
Solo `lab-argon2-v1` puede ser destino de login; la antigua solo permite verificar
y migrar. `commercial_approved` permanece **false**, también en los tests: no hay
un test que exija o simule aprobación del owner.

La elección v1 toma el piso de referencia OWASP citado por F4–19. Las mediciones
siguientes caracterizan ese candidato; **no son selección optimizada ni aprobación
para hardware de producto**. v0 es deliberadamente débil y exclusivamente sintética.

RustCrypto usa los parámetros del PHC para verificar, aunque el objeto `Argon2`
tenga otros configurados. Por ello, **antes de `verify_password`** se comprueba:

- String PHC de hasta 256 bytes, algoritmo exactamente `argon2id`, versión 19.
- Solo `m/t/p`, límites m≤32768 KiB, t≤3, p≤2 y mínimos no nulos.
- Policy registrada **y coincidencia exacta** de m/t/p con ella: no basta estar
  bajo el techo. También se exigen salida de 32 bytes y salt decodificado de 16.
- Un PHC válido con policy desconocida, policy/hash inconsistentes, versión,
  algoritmo o parámetros adicionales no admitidos exige reset, sin fallback.
- BCrypt: longitud/estructura/alfabeto, variante admitida y coste 4..12 antes de
  `bcrypt::verify`; coste 31 y variante 2x se rechazan sin trabajo BCrypt.

Los tests incluyen m/t/p extremos, versión/algoritmo distintos, parámetros extra,
salt/salida inválidos, entrada excesiva y mismatch de policy. Los límites reducen
el trabajo **por intento**; no sustituyen rate limiting, concurrencia acotada ni
presupuesto de RAM de un futuro servicio. Ningún endpoint productivo se habilita.

## APIs y dependencias exactas para el integrador

Se consultó `cargo info argon2@0.5.3`, `cargo info bcrypt@0.17.1` y
`cargo info rand_core@0.6.4`, además del código documentado de esas versiones.
Son versiones fijadas para las APIs verificadas, **no una afirmación de que sean
las últimas** (cargo informó argon2 0.6.0 y bcrypt 0.19.3 como más recientes).
No se modificó la resolución del workspace.

Agregar al paquete `migration-lab` (o declarar equivalentes en el workspace):

```toml
argon2 = { version = "=0.5.3", features = ["std", "zeroize"] }
bcrypt = "=0.17.1"
rand_core = { version = "=0.6.4", features = ["getrandom"] }
```

Ya se requieren las dependencias existentes `serde` con `derive`, `serde_json` y
`tokio` con runtime. El harness temporal usó sus versiones del workspace:
serde 1.0.229, serde_json 1.0.154, tokio 1.54.1 y thiserror 2.0.21 para un
`LabError::Invalid(String)` compatible con el existente.

Argon2 0.5.3 resuelve `password-hash 0.5.0`, que usa **rand_core 0.6**.
El módulo importa `OsRng`/`RngCore` desde
`argon2::password_hash::rand_core`, no desde un rand_core moderno incompatible.
La dependencia directa 0.6.4 habilita `getrandom` para esa misma versión.
`OsRng.try_fill_bytes` permite devolver un error fijo si falta entropía;
`SaltString::encode_b64` y `PasswordHasher::hash_password` son APIs de biblioteca.
No se implementan RNG, KDF, comparación de hashes ni codificación manuales.
Se usa un salt nuevo por hash; no se reutiliza explícitamente ni se introduce pepper.

Referencias:
[BCrypt 0.17.1](https://docs.rs/bcrypt/0.17.1/src/bcrypt/lib.rs.html),
[Argon2 0.5.3](https://docs.rs/argon2/0.5.3/src/argon2/lib.rs.html),
[OWASP Password Storage](https://cheatsheetseries.owasp.org/cheatsheets/Password_Storage_Cheat_Sheet.html).

## Validación y benchmark observados

Antes de implementar se ejecutó una prueba del informe contra una implementación
mínima: compiló y falló por aserción **0 casos frente a 8** (`first_red` real).
Después pasó la suite del módulo. No se sustituyó ninguna policy por otra más
barata para hacer verdes los tests.

El módulo ya está integrado en `migration-lab`: `main.rs` registra `mod auth;`,
ejecuta `auth::run()` en cada corrida y agrega `AUTH-MIG-01..08` a los checks
técnicos. La suite completa del workspace ejecuta los tests de auth; el runner
también requiere que el informe contenga exactamente esos ocho IDs.

La validación inicial se hizo con el `auth.rs` real en un crate temporal, sin
fingir storage ni autenticación. La suite del módulo pasó **11 tests**, con cero
fallos y cero ignorados; Clippy con warnings/unwrap/expect/panic denegados y
formato Rust también pasaron. La medición release inicial se lanzó sin red en un
namespace aislado:

```sh
# En el harness temporal, ya retirado:
unshare --user --map-root-user --net cargo test --offline --release --manifest-path Cargo.toml
unshare --user --map-root-user --net cargo run --offline --release --manifest-path Cargo.toml --quiet
```

Observación del host actual: Linux x86_64, AMD Ryzen 5 5600G, 12 CPUs lógicas
disponibles, rustc 1.99.1. **No es hardware objetivo de producto**. La ejecución
observó los ocho casos `PASSED` y `normative_status: BLOCKED`.

La medición release inicial (harness temporal) fue:

| Policy          | Hash p50 / p95 (ms) | Verify p50 / p95 (ms) |        Workspace medido |
| --------------- | ------------------: | --------------------: | ----------------------: |
| `lab-argon2-v0` |       2.513 / 3.843 |         2.519 / 2.580 |   8388608 bytes (8 MiB) |
| `lab-argon2-v1` |     15.422 / 18.076 |       15.499 / 18.851 | 19922944 bytes (19 MiB) |

La ejecución **integrada en debug** registrada en `auth-transition.json` midió
`lab-argon2-v1` hash p50/p95 ≈ **398,07/400,73 ms** y verify ≈ **398,32/401,07 ms**.
La diferencia de perfil es esperada y demuestra por qué el benchmark no es un
SLO: hay que medir release en hardware objetivo antes de habilitar usuarios.
Son muestras de una ejecución, no thresholds ni SLOs: un worker secuencial,
20 muestras por operación y una warmup, percentiles nearest-rank. Hash incluye
CSPRNG y asignación de workspace; verify usa un PHC válido previamente generado
y admitido por los guards. No mide el login completo ni carga concurrente.

La memoria se mide sobre la asignación real de `Vec<argon2::Block>` suministrada
a `hash_password_into_with_memory`, mediante `size_of_val`, en cada muestra. No
se presenta un cálculo teórico como RSS: overhead del proceso, stacks, allocator
y pico RSS **no están medidos** (`peak_rss_bytes: null`). La ruta PHC normal usa
la asignación interna de la biblioteca con los mismos parámetros admitidos.

El namespace de red es evidencia técnica de esta ejecución, no un harness
normativo mantenido ni una aprobación de DDL13-10/AS-09. El módulo no contiene
operaciones de red, pero no certifica por sí solo el aislamiento del proceso.

Después de integrar, ejecutar desde `backend-rust/`:

```sh
cargo test -p migration-lab auth::tests
```

## Salida segura y pendientes

El JSON se construye mediante una lista permitida de campos, nunca serializando
el estado. Credenciales y vectores no implementan `Debug`/`Serialize`; los errores
criptográficos se descartan en favor de códigos fijos. Los tests comprueban que
el informe no incluya hashes, passwords sintéticas, salts ni handles. No se
publican dumps de estado ni excepciones que contengan su entrada.

Pendiente fuera de este incremento: importador completo/DDL, atomicidad
persistente, sesiones opacas/revocación, permisos/RBAC, vectores del sistema
completo y benchmark con presupuesto UX/RAM y concurrencia en hardware objetivo
antes de habilitar usuarios reales. El runner ya está integrado y reporta
`security-report.json`/`auth-transition.json`; **AS-09 sigue BLOCKED**.
