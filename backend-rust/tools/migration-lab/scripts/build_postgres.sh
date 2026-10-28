#!/bin/sh
# Build exclusivamente local; no sudo, Docker, servicios ni datos existentes.
set -eu
umask 077
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
workspace=$(CDPATH= cd -- "$script_dir/../../.." && pwd)
build="$workspace/target/pg17-build"
version=17.11
archive="postgresql-$version.tar.bz2"
sha256=dd27f2b3c59e73ed14aa3324901242bf69a032a6347805f274e6260322d42979
jobs=${POS_LAB_PG_BUILD_JOBS:-2}
case "$jobs" in ''|*[!0-9]*|0) echo 'POS_LAB_PG_BUILD_JOBS debe ser entero positivo' >&2; exit 1 ;; esac
for command in curl sha256sum tar make cc timeout flock; do
    command -v "$command" >/dev/null || { echo "Falta herramienta: $command" >&2; exit 1; }
done
mkdir -p "$build"
exec 9>"$build/build.lock"
flock -n 9 || { echo 'Ya hay una compilación PostgreSQL en este directorio' >&2; exit 1; }
if [ -x "$build/install/bin/postgres" ] && [ -x "$build/install/bin/pg_dump" ] && [ -x "$build/install/bin/pg_restore" ] && [ -x "$build/install/bin/initdb" ] && [ -x "$build/install/bin/pg_ctl" ]; then
    for binary in postgres pg_dump pg_restore initdb pg_ctl; do
        observed=$(timeout 5 "$build/install/bin/$binary" --version)
        case "$observed" in
            *"(PostgreSQL) $version") ;;
            *) echo "Instalación existente no corresponde a PostgreSQL $version" >&2; exit 1 ;;
        esac
    done
    printf '%s\n' "$observed"
    printf 'POS_LAB_PG_BIN=%s\n' "$build/install/bin"
    exit 0
fi
cd "$build"
if [ ! -f "$archive" ]; then
    curl --fail --location --proto '=https' --proto-redir '=https' --connect-timeout 10 --max-time 120 \
        "https://ftp.postgresql.org/pub/source/v$version/$archive" --output "$archive.part"
    mv "$archive.part" "$archive"
fi
printf '%s  %s\n' "$sha256" "$archive" | sha256sum --check --strict
if [ ! -d "postgresql-$version" ]; then tar -xjf "$archive"; fi
cd "postgresql-$version"
timeout 120 ./configure --prefix="$build/install" --without-icu --without-readline --without-zlib >"$build/configure.log" 2>&1
timeout 600 make -j "$jobs" >"$build/make.log" 2>&1
timeout 120 make install >"$build/install.log" 2>&1
timeout 5 "$build/install/bin/postgres" --version
printf 'POS_LAB_PG_BIN=%s\n' "$build/install/bin"
