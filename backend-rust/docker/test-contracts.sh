#!/bin/sh
# Fallar en el primer error; no convertir fallos técnicos o normativos en PASS.
set -eu

printf '\n=== Linux local: usuario sin privilegios ===\n'
test "$(id -u)" = 10001
id
printf '\n=== Rust: formato, Clippy, pruebas, build ===\n'
cd /workspace/backend-rust
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace --all-features
cargo build --locked --workspace --all-features

printf '\n=== TypeScript: integridad y pruebas reales de contrato ===\n'
cd /workspace/frontend
pnpm lint
pnpm typecheck
pnpm test
pnpm test:rust-contracts
pnpm build
printf '\nPASS: Linux Docker local, contratos y SQLite sintético. No equivale a CI ni aprobación normativa.\n'
