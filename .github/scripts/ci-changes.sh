#!/usr/bin/env bash
#
# Detección de áreas modificadas para condicionar los jobs de CI.
#
# Decide qué jobs deben ejecutarse según los archivos cambiados y publica los
# resultados en $GITHUB_OUTPUT. Cada job se ejecuta una sola vez (sin bucles ni
# repeticiones E2E).
#
# Reglas:
#   - schedule / workflow_dispatch -> validación completa.
#   - cambio de backend o de frontend de producción -> incluye un E2E.
#   - cambio del harness E2E o de la propia CI -> incluye un E2E.
#   - solo docs / tests unitarios -> sin E2E.
#
# Se apoya únicamente en `git` (sin actions de terceros).

set -euo pipefail

event_name="${EVENT_NAME:-}"
output_file="${GITHUB_OUTPUT:-/dev/stdout}"

emit() {
  printf '%s=%s\n' "$1" "$2" >>"$output_file"
}

full="false"
if [[ "$event_name" == "schedule" || "$event_name" == "workflow_dispatch" ]]; then
  full="true"
fi

files=""
if [[ "$full" == "false" ]]; then
  if [[ "$event_name" == "pull_request" ]]; then
    base="${BASE_SHA:-}"
  else
    base="${BEFORE_SHA:-}"
  fi
  # Sin base válida (primer push o SHA ausente) se cae a validación completa.
  if [[ -z "$base" || "$base" =~ ^0+$ ]]; then
    full="true"
  elif git rev-parse --verify --quiet "${base}^{commit}" >/dev/null; then
    files="$(git diff --name-only "${base}...HEAD")"
  else
    full="true"
  fi
fi

has() {
  [[ -n "$files" ]] && printf '%s\n' "$files" | grep -Eq "$1"
}

flag() {
  if has "$1"; then echo true; else echo false; fi
}

backend="false"
frontend="false"
frontend_prod="false"
e2e="false"
docker="false"
ci="false"
if [[ "$full" == "false" ]]; then
  backend=$(flag '^backend/')
  frontend=$(flag '^frontend/')
  e2e=$(flag '^frontend/e2e/|^frontend/scripts/e2e/|^frontend/playwright\.config\.|^frontend/tsconfig\.e2e\.json')
  docker=$(flag '^docker-compose\.yml$|^Dockerfile|/Dockerfile|\.dockerignore$|\.env\.example$|^deploy/')
  ci=$(flag '^\.github/')
  # Frontend de producción: archivos de frontend que no son harness E2E ni tests unitarios.
  if [[ -n "$files" ]] && printf '%s\n' "$files" | grep -E '^frontend/' \
    | grep -Ev '^frontend/e2e/|^frontend/scripts/e2e/|^frontend/playwright\.config\.|^frontend/tsconfig\.e2e\.json' \
    | grep -Ev '\.test\.(ts|tsx)$' | grep -q .; then
    frontend_prod="true"
  fi
fi

backend_run="false"
frontend_run="false"
compose_run="false"
if [[ "$full" == "true" || "$backend" == "true" || "$ci" == "true" ]]; then backend_run="true"; fi
if [[ "$full" == "true" || "$frontend" == "true" || "$ci" == "true" ]]; then frontend_run="true"; fi
if [[ "$full" == "true" || "$docker" == "true" || "$ci" == "true" ]]; then compose_run="true"; fi

# E2E: una sola ejecución cuando cambia comportamiento o el harness/CI.
e2e_run="false"
if [[ "$full" == "true" || "$e2e" == "true" || "$ci" == "true" || "$backend" == "true" || "$frontend_prod" == "true" ]]; then
  e2e_run="true"
fi

emit backend "$backend"
emit frontend "$frontend"
emit frontend_prod "$frontend_prod"
emit e2e "$e2e"
emit docker "$docker"
emit ci "$ci"
emit full "$full"
emit backend_run "$backend_run"
emit frontend_run "$frontend_run"
emit compose_run "$compose_run"
emit e2e_run "$e2e_run"
