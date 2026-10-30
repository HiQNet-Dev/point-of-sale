# Arnés Docker del backend Rust Local

Configuración **separada** del stack Java (`docker-compose.yml`). No lo
sustituye, no publica puertos y no define la distribución Free Local.

## Archivos

| Archivo | Propósito |
| --- | --- |
| `backend-rust/Dockerfile` | Build multi-stage (`rust:1.99-bookworm` → `debian:bookworm-slim`) |
| `backend-rust/docker/healthcheck.sh` | Smoke: comprueba que SQLite existe y tiene contenido |
| `backend-rust/.dockerignore` | Excluye `target/`, propuestas y archivos SQLite |
| `docker-compose.rust.yml` | Servicio `pos-local` con volumen persistente |

## Garantías del runtime

- Usuario sin privilegios `10001:10001`.
- Root filesystem de solo lectura, `/tmp` en `tmpfs`.
- `no-new-privileges`.
- Volumen `pos-local-data` en `/data`, permisos `0700`.
- Sin `ports:`: `pos-local` no ofrece HTTP.

## Procedimiento reproducible

```bash
docker compose -f docker-compose.rust.yml build
docker compose -f docker-compose.rust.yml up -d
docker compose -f docker-compose.rust.yml ps
docker compose -f docker-compose.rust.yml logs
docker compose -f docker-compose.rust.yml down
```

## Evidencia ejecutada

Con Docker 29.9.2 (Linux):

1. `build` completó y etiquetó `hiqnet-pos-local:dev`.
2. `up -d` dejó el contenedor `healthy`; log `{"event":"ready"}`.
3. `run --rm --entrypoint //bin/sh pos-local -c id` confirmó `uid=10001(pos)`.
4. Tras `stop`, el log mostró `shutdown_requested` y `stopped`, con salida `0`.
5. Tras reiniciar y detener, `sha256sum /data/pos.sqlite` fue idéntico
   (`50cae0fcdff0146a62c24b9db66604e2d1b7697ae0fddc1adcb2d2c8780f3566`) y la
   migración `0001` seguía presente, lo que demuestra idempotencia y persistencia.

## Límites

- El healthcheck verifica inicialización, no salud de negocio: no hay API.
- Solo se aplica `0001`; `0002`–`0015` siguen fuera de `migrations/local`.
- En Windows/MSYS, `docker compose run` requiere `//bin/sh` para evitar la
  conversión de rutas del host.
