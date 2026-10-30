# syntax=docker/dockerfile:1
# Imagen local de pruebas Linux; no es la distribución Free Local.
FROM node:24-bookworm-slim AS node
FROM rust:1.99-bookworm AS tests

COPY --from=node /usr/local/bin/node /usr/local/bin/node
COPY --from=node /usr/local/lib/node_modules/npm /opt/npm
RUN node /opt/npm/bin/npm-cli.js install --global pnpm@11.28.5 \
    && rustup toolchain install 1.99.1 --profile minimal --component rustfmt --component clippy \
    && apt-get update \
    && apt-get install -y --no-install-recommends python3 procps \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --gid 10001 tester \
    && useradd --uid 10001 --gid tester --create-home tester \
    && mkdir /workspace \
    && chown tester:tester /workspace

ENV CARGO_HOME=/home/tester/.cargo
WORKDIR /workspace
COPY --chown=tester:tester frontend/package.json frontend/pnpm-lock.yaml ./frontend/
USER 10001:10001
RUN pnpm --dir frontend install --frozen-lockfile

USER root
COPY --chown=tester:tester backend-rust/ ./backend-rust/
COPY --chown=tester:tester frontend/ ./frontend/
COPY --chown=tester:tester docs/ ./docs/
COPY --chown=tester:tester backend/src/main/resources/db/migration/ ./backend/src/main/resources/db/migration/
COPY --chown=tester:tester VERSION ./VERSION
USER 10001:10001

# Compilar antes permite repetir la ejecución sin red ni descargas.
RUN cargo build --manifest-path backend-rust/Cargo.toml --locked --workspace --all-targets --all-features
ENV CARGO_NET_OFFLINE=true
CMD ["sh", "/workspace/backend-rust/docker/test-contracts.sh"]
