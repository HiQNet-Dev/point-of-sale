# Importación del bundle 2G — procedimiento preparado

**Estado: PREPARADO, NO EJECUTADO.**
El bundle `hiqnet-rust-2g.bundle` no está disponible en este equipo ni en el
remoto a la fecha de redacción (2026-10-31). Este documento describe el
procedimiento autorizado para recuperar e integrar 2E–2G sin reimplementarlos.

Base actual: `e2de3f55`.
Rama aislada de trabajo: `fix/gitleaks-synthetic-vectors-2i`.
No se modifica `dev`, `main` ni el PR #84 durante la importación.

## 1. Condiciones previas

- Bundle generado en el equipo original que contiene `c794814`, `eb32f06` y
  `3f8c94f` (o sus commits equivalentes).
- SHA-256 del bundle comunicado por un canal confiable.
- SHAs completos de 40 caracteres de 2E, 2F y 2G.
- Autorización expresa para publicar la rama de respaldo o transferir el bundle.
- Árbol de integración limpio (`git status` sin cambios ajenos).
- Confirmación de que el bundle no contiene secretos, `target/`, `node_modules/`
  ni artefactos de build.

## 2. En el equipo original (solo lectura + respaldo)

```bash
# Verificar que los commits existen antes de empaquetar.
git cat-file -t <sha-2e>
git cat-file -t <sha-2f>
git cat-file -t <sha-2g>

# Crear referencias de respaldo locales.
git branch backup/rust-2g-20261031 <sha-2g>

# Empaquetar toda la historia y los refs necesarios.
git bundle create hiqnet-rust-2g.bundle --all
sha256sum hiqnet-rust-2g.bundle
```

Transferencia autorizada, en orden de preferencia:

1. Rama privada de respaldo en `origin`:
   `git push origin backup/rust-2g-20261031`
2. Bundle custodiado por canal administrativo privado.

No usar force-push ni reescribir historia publicada.

## 3. En el equipo de integración: verificar antes de importar

```bash
sha256sum hiqnet-rust-2g.bundle
git bundle verify hiqnet-rust-2g.bundle
git bundle list-heads hiqnet-rust-2g.bundle
```

Comparar el SHA-256 con el comunicado. Si no coincide, detenerse.

Crear respaldo del estado actual antes de tocar refs:

```bash
git branch backup/pre-2g-20261031 HEAD
git rev-parse HEAD > pre-2g-head.txt
```

## 4. Importar a refs temporales, sin cambiar la rama actual

```bash
git fetch hiqnet-rust-2g.bundle \
  "refs/heads/*:refs/remotes/2g/*"
```

Si el bundle solo contiene una rama:

```bash
git fetch hiqnet-rust-2g.bundle <rama>:refs/remotes/2g/<rama>
```

Verificar que la historia contiene los commits:

```bash
git cat-file -t <sha-2e>
git cat-file -t <sha-2f>
git cat-file -t <sha-2g>
git merge-base --is-ancestor <sha-2e> <sha-3g>
git merge-base --is-ancestor <sha-2f> <sha-3g>
```

## 5. Comparar grafos antes del merge

```bash
git log --graph --oneline --decorate -n 40 refs/remotes/2g/<rama>
git rev-list --left-right --count HEAD...refs/remotes/2g/<rama>
git diff --stat HEAD...refs/remotes/2g/<rama>
```

Revisar:

- Que 2E, 2F y 2G sean ancestros del head importado.
- Que no se hayan perdido commits de la rama actual.
- Que el diff no incluya artefactos ni secretos.
- Que la reconciliación Organization/Branch de 2F exista en el bundle. Si existe,
  **se reutiliza**. No se implementa una segunda reconciliación.

## 6. Integración no destructiva

```bash
git switch fix/gitleaks-synthetic-vectors-2i
git switch -c integration/rust-2g-recovery
git merge --no-ff refs/remotes/2g/<rama>
```

Reglas de resolución:

- Conflictos semánticos, revisados archivo por archivo.
- Prohibido `ours` o `theirs` globales.
- Prohibido `reset --hard` y force-push.
- Si el merge no puede completarse con seguridad: `git merge --abort` y detenerse.
- Mantener los padres y la trazabilidad del merge.

## 7. Validación obligatoria antes de cualquier push

```bash
# Rust
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace --all-features
cargo build --locked --workspace --all-features

# Interoperabilidad de credenciales
node backend-rust/tools/credential-interop/run.mjs

# Gitleaks 8.24.3 sobre el árbol versionado, sin artefactos de build
git archive HEAD -o scan.tar
mkdir scan && tar -xf scan.tar -C scan
gitleaks dir scan/backend-rust --redact --no-banner
```

Checklist de contenido recuperado:

- [ ] Documentación 2E y 2F.
- [ ] Propuesta CI de 2G.
- [ ] `proposals/local-auth/0017_bootstrap_authorization.proposed.sql`.
- [ ] Emisor nativo de tokens.
- [ ] Login/sesiones y logout/revocación.
- [ ] Reconciliación Organization/Branch única (2F).
- [ ] Guardas negativas de migraciones y Tauri.
- [ ] 17 casos de uso y 17 handlers Purchasing, sin registrar.
- [ ] Backup/restauración.
- [ ] Solo `0001` en el migrador Local productivo.

## 8. Publicación

Solo con autorización expresa posterior:

```bash
git push origin integration/rust-2g-recovery
```

No fusionar el PR #84, no tocar `dev`/`main`, no ejecutar Actions y no promover
migraciones. La autorización de publicación no autoriza el merge.

## 9. Reversión

- Antes del merge: `git merge --abort`.
- Después del merge local: mantener `backup/pre-2g-20261031` y revertir el merge
  con `git revert -m 1 <merge-sha>` si se autoriza.
- Nunca `reset --hard` sobre refs publicadas.
