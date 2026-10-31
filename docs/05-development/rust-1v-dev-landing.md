# Estrategia de aterrizaje a `dev` — Incremento 1V

**Estado: propuesta. No se ha integrado nada en `dev` ni en `main`.**

## 1. Auditoría de la cadena #64–#80

La cadena Rust es lineal: cada PR tiene como base la rama del anterior. Verificado
con `git merge-base --is-ancestor`:

| Comprobación | Resultado |
| --- | --- |
| HEAD de #64 contenido en #80 | Sí |
| HEAD de #79 contenido en #80 | Sí |
| `merge-base(dev, tip)` | `dev` HEAD (`329c0984`) |
| Commits de `dev` ausentes en el tip | 0 |
| Commits del tip ausentes en `dev` | 86 |
| Commits que aporta #80 sobre #79 | 16 |

**Conclusión:** #64–#79 no contienen commits únicos respecto de #80. Integrar la
cadena PR por PR duplicaría trabajo y generaría conflictos artificiales. **Solo
el tip necesita aterrizar.**

## 2. Unidad de aterrizaje

| Unidad | Contenido | Base |
| --- | --- | --- |
| A. Consolidación | #80 (arquitectura hexagonal) = 86 commits sobre `dev` | lista |
| B. Arnés ejecutable | #82 (pos-lab + Docker + paquete 1V) | apilada sobre A |
| C. Evidencia CI | #81 (validación temporal contra `dev`) | temporal, no se fusiona |

## 3. Secuencia propuesta

1. **Titular acepta** `docs/05-development/rust-1v-approval-resolution.md`.
2. Se promueven a `migrations/local/` solo las migraciones autorizadas, en el
   orden de la sección 4 del paquete.
3. Se ejecuta CI sobre el tip resultante: el job Rust debe pasar **incluido**
   `--require-complete`. Mientras el gate siga devolviendo exit 2, no hay merge.
4. Se integra el tip en `dev` mediante PR (squash o merge commit, según
   `docs/05-development/git-workflow.md`). Sin force-push ni reescritura.
5. Se cierran #64–#79 como **superseded**, citando el SHA que las contiene.
6. #75 y #81 (validación) se cierran al terminar su función.

## 4. Qué impide el aterrizaje hoy

| Bloqueo | Efecto | Ruta de cierre |
| --- | --- | --- |
| Gate normativo `--require-complete` | CI rojo en el job Rust | Aprobación física + promoción de migraciones |
| Aprobación del titular | No se puede promover DDL | Completar el bloque de aceptación del paquete 1V |
| D-AUTH-09 | Bloquea autenticación comercial, no el DDL | Benchmark + interoperabilidad Argon2id |

## 5. Lo que sí puede aterrizar sin aprobación física

Nada del backend Rust, porque el gate normativo es global al job. Alternativas
descartadas y por qué:

- **Desactivar el gate temporalmente:** prohibido; oculta un control.
- **`continue-on-error` o `|| true`:** prohibido; falsea el resultado.
- **Mover el gate a otro workflow:** equivaldría a debilitarlo.

Por tanto, la única vía legítima es la aprobación del paquete físico.

## 6. Verificación previa al merge

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`
- [ ] `cargo test --locked --workspace --all-features`
- [ ] `cargo build --locked --workspace --all-features`
- [ ] Migration Lab con PostgreSQL 17
- [ ] `cargo audit`
- [ ] Gitleaks
- [ ] Gate normativo en verde (sin `--require-complete` relajado)
- [ ] Docker: build, `up`, healthcheck, `pos-lab` smoke, `restart`, `down`/`up`
