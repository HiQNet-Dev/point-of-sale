# Runtime de desarrollo/pruebas para el backend Rust Local.
#
# No sustituye al stack Java ni define la distribución Free Local: `pos-local`
# abre SQLite, aplica las migraciones aprobadas y espera señales. Este contenedor
# es un arnés de smoke e integración, no una API ni un servicio de red.

# syntax=docker/dockerfile:1
FROM rust:1.99-bookworm AS builder
WORKDIR /src

# El binario embebe las migraciones con `sqlx::migrate!`, por lo que el
# directorio debe existir en tiempo de compilación.
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY apps ./apps
COPY tools ./tools
COPY migrations ./migrations
# Las propuestas candidatas se embeben en `pos-lab` con `include_str!`; no se
# cargan en el migrador productivo.
COPY proposals ./proposals

RUN cargo build --locked --release -p pos-local -p pos-lab

FROM debian:bookworm-slim AS runtime

RUN groupadd --system --gid 10001 pos \
    && useradd --system --uid 10001 --gid pos --home-dir /data --shell /usr/sbin/nologin pos \
    && mkdir -p /data \
    && chown 10001:10001 /data \
    && chmod 0700 /data

COPY --from=builder /src/target/release/pos-local /usr/local/bin/pos-local
COPY --from=builder /src/target/release/pos-lab /usr/local/bin/pos-lab
COPY docker/healthcheck.sh /usr/local/bin/pos-local-health
RUN chmod 0555 /usr/local/bin/pos-local-health /usr/local/bin/pos-local /usr/local/bin/pos-lab

USER 10001:10001

ENV POS_LOCAL_DB_PATH=/data/pos.sqlite \
    POS_LOCAL_BUSY_TIMEOUT_MS=5000

VOLUME ["/data"]

HEALTHCHECK --interval=10s --timeout=3s --start-period=5s --retries=3 \
    CMD ["/usr/local/bin/pos-local-health"]

ENTRYPOINT ["/usr/local/bin/pos-local"]
