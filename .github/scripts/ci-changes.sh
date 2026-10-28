#!/usr/bin/env bash
#
# Detección de áreas modificadas para condicionar los jobs del CI general.
#
# Decide qué jobs deben ejecutarse según los archivos cambiados y publica los
# resultados en $GITHUB_OUTPUT. La integración E2E no se decide aquí: corre solo
# en el CI robusto de `main` (y en la validación manual).
#
# Reglas:
#   - workflow_dispatch (y schedule, si se habilitara) -> validación completa.
#   - cambio de backend / Rust / frontend / docker / `.github/**` -> ejecuta el job afín.
#   - solo docs -> no ejecuta nada ejecutable.
#   - ruta desconocida (no docs y no mapeada) -> fail-safe: validación completa.
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

# Rutas que no producen artefactos ejecutables (documentación y metadatos): un
# cambio limitado a ellas no necesita jobs pesados.
non_exec_re='^docs/|\.md$|^LICENSE$|^SECURITY\.md$|^\.gitignore$|^\.gitattributes$|^\.editorconfig$|^\.serena/|^\.worktrees/'
# Rutas conocidas que ya mapean a un job concreto.
known_re='^backend/|^backend-rust/|^frontend/|^docker-compose\.yml$|^Dockerfile|/Dockerfile|\.dockerignore$|\.env\.example$|^deploy/|^\.github/'

backend="false"
rust="false"
frontend="false"
docker="false"
ci="false"
if [[ "$full" == "false" && -n "$files" ]]; then
  executable="$(printf '%s\n' "$files" | grep -Ev "$non_exec_re" || true)"
  if [[ -n "$executable" ]]; then
    # Un archivo no reconocido cae a validación completa (fail-safe) en lugar de
    # omitir todos los jobs por error.
    unknown="$(printf '%s\n' "$executable" | grep -Ev "$known_re" || true)"
    if [[ -n "$unknown" ]]; then
      full="true"
    else
      backend=$(flag '^backend/')
      rust=$(flag '^backend-rust/')
      frontend=$(flag '^frontend/')
      docker=$(flag '^docker-compose\.yml$|^Dockerfile|/Dockerfile|\.dockerignore$|\.env\.example$|^deploy/')
      ci=$(flag '^\.github/')
    fi
  fi
fi

backend_run="false"
rust_run="false"
frontend_run="false"
compose_run="false"
if [[ "$full" == "true" || "$backend" == "true" || "$ci" == "true" ]]; then backend_run="true"; fi
if [[ "$full" == "true" || "$rust" == "true" || "$ci" == "true" ]]; then rust_run="true"; fi
if [[ "$full" == "true" || "$frontend" == "true" || "$ci" == "true" ]]; then frontend_run="true"; fi
if [[ "$full" == "true" || "$docker" == "true" || "$ci" == "true" ]]; then compose_run="true"; fi

emit backend "$backend"
emit rust "$rust"
emit frontend "$frontend"
emit docker "$docker"
emit ci "$ci"
emit full "$full"
emit backend_run "$backend_run"
emit rust_run "$rust_run"
emit frontend_run "$frontend_run"
emit compose_run "$compose_run"
