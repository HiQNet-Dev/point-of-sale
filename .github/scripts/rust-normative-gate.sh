#!/usr/bin/env bash
#
# Nivel 2: gate normativo de release.
#
# Ejecuta el laboratorio normativo real con `--require-complete` sin alterar su
# semántica y clasifica su código de salida:
#
#   0 -> PASSED   cobertura normativa completa con evidencia válida
#   2 -> BLOCKED  ejecución técnica válida, cobertura normativa incompleta
#   * -> FAILED   fallo técnico o evidencia inválida
#
# El exit real se publica como output y se conserva en la evidencia; nunca se
# usa `continue-on-error` ni `|| true`. Un FAILED siempre rompe el paso. La
# política por destino (dev vs main) la aplica `ci-gate`, no este script.

set -euo pipefail

output_dir="target/migration-gate-evidence"

set +e
cargo run --locked -p migration-lab -- \
  --output "$output_dir" \
  --postgres-bin target/pg17-build/install/bin \
  --java-source --require-complete
exit_code=$?
set -e

case "$exit_code" in
  0) status=PASSED ;;
  2) status=BLOCKED ;;
  *) status=FAILED ;;
esac

printf 'normative_status=%s normative_exit=%s\n' "$status" "$exit_code"

if [[ -n "${GITHUB_OUTPUT:-}" ]]; then
  printf 'status=%s\n' "$status" >>"$GITHUB_OUTPUT"
  printf 'exit_code=%s\n' "$exit_code" >>"$GITHUB_OUTPUT"
fi

if [[ -n "${GITHUB_STEP_SUMMARY:-}" ]]; then
  {
    printf '## Rust Normative Release Gate: %s\n\n' "$status"
    printf '| Elemento | Valor |\n| --- | --- |\n'
    printf '| Exit code del laboratorio | `%s` |\n' "$exit_code"
    printf '| Estado normativo | **%s** |\n\n' "$status"
    if [[ "$status" == "BLOCKED" ]]; then
      printf 'La cobertura normativa sigue incompleta. Este estado **no** autoriza\n'
      printf 'la integración a `main`, ni un release, ni la activación comercial.\n'
    fi
  } >>"$GITHUB_STEP_SUMMARY"
fi

if [[ "$status" == "FAILED" ]]; then
  printf 'Fallo técnico del laboratorio normativo (exit %s)\n' "$exit_code" >&2
  exit 1
fi
