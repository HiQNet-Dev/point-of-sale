#!/bin/sh
# Smoke check del contenedor Local: el archivo SQLite existe y tiene contenido,
# lo que implica que `open` y `migrate` completaron. No sustituye a un servicio
# de salud: `pos-local` no expone red y no debe exponerla en Free Local.
set -eu

db="${POS_LOCAL_DB_PATH:-/data/pos.sqlite}"

if [ ! -s "$db" ]; then
    echo "sqlite no inicializado en $db" >&2
    exit 1
fi

exit 0
