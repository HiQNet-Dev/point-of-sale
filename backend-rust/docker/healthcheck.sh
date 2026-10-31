#!/bin/sh
# Healthcheck funcional del contenedor Local.
#
# No se limita a comprobar la existencia del archivo: `pos-local --health` abre
# SQLite, valida `PRAGMA integrity_check` y exige al menos una migración aplicada
# en `_sqlx_migrations`. Nunca crea la base: un volumen vacío es un fallo.
#
# No sustituye a un servicio de salud de red: `pos-local` no expone HTTP y la
# distribución Free Local no debe exponerlo.
set -eu

exec /usr/local/bin/pos-local --health
