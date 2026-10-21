#!/usr/bin/env bash
set -Eeuo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

# One reproducible local/CI gate for the pinned LiteCowork toolchain.
#
# Usage:
#   scripts/check.sh              full gate: toolchain, Rust format/lint/test/build,
#                                 Python tests, contract validators, whitespace
#   scripts/check.sh --preflight  fast gate: toolchain, Python tests, and the
#                                 contract validators, without the Rust compile
#
# The Rust and Python toolchains are pinned by committed files. Every stage runs
# against the pinned versions; a floating interpreter or an unpinned compiler is
# never used, and a missing or mismatched pinned tool fails before any build,
# download, or test work starts.

preflight_only=0
for argument in "$@"; do
  case "$argument" in
    --preflight) preflight_only=1 ;;
    -h | --help)
      cat <<'USAGE'
Usage: scripts/check.sh [--preflight]

  (no argument)  Full gate: pinned toolchain, Rust format/lint/test/build,
                 Python tests, contract validators, and the whitespace check.
  --preflight    Fast gate: pinned toolchain, Python tests, and the contract
                 validators. Skips the Rust compile stages.
USAGE
      exit 0
      ;;
    *)
      printf 'error: unsupported argument %s.\n' "$argument" >&2
      printf "Run 'scripts/check.sh --help' for usage.\n" >&2
      exit 2
      ;;
  esac
done

expected_uv="$(<.uv-version)"
expected_python="$(<.python-version)"
expected_rust="$(sed -n 's/^channel = "\(.*\)"$/\1/p' rust-toolchain.toml)"

# Reports a missing pinned tool instead of letting the shell print a bare
# "command not found" that does not say which pin has to be installed.
require_tool() {
  local tool="$1"
  local pin="$2"
  if ! command -v "$tool" >/dev/null 2>&1; then
    printf 'Required pinned tool %s was not found on PATH.\n' "$tool" >&2
    printf 'Install %s %s before running scripts/check.sh.\n' "$tool" "$pin" >&2
    exit 1
  fi
}

# Prints the version field of a `<tool> --version` line without failing the
# script when the tool cannot report a version at all.
tool_version() {
  local tool="$1"
  "$tool" --version 2>/dev/null | awk 'NR == 1 { print $2; exit }' || true
}

require_tool uv "$expected_uv"
actual_uv="$(tool_version uv)"
if [[ "$actual_uv" != "$expected_uv" ]]; then
  printf 'Expected uv %s, found %s.\n' "$expected_uv" "$actual_uv" >&2
  exit 1
fi

require_tool rustc "$expected_rust"
actual_rust="$(tool_version rustc)"
if [[ -z "$expected_rust" || "$actual_rust" != "$expected_rust" ]]; then
  printf 'Expected Rust %s, found %s.\n' "$expected_rust" "$actual_rust" >&2
  exit 1
fi

require_tool cargo "$expected_rust"
actual_cargo="$(tool_version cargo)"
if [[ "$actual_cargo" != "$expected_rust" ]]; then
  printf 'Expected Cargo %s, found %s.\n' "$expected_rust" "$actual_cargo" >&2
  exit 1
fi

printf 'Rust: %s\n' "$(rustc --version)"
printf 'Cargo: %s\n' "$(cargo --version)"
printf 'uv: %s\n' "$(uv --version)"
printf 'Python pin: %s\n' "$(<.python-version)"

# The locked Python environment is the only interpreter this gate may use. Try
# the network-backed sync first and fall back to the local uv cache so an
# offline host with a warm cache can still run the gate.
if ! uv sync --locked; then
  printf 'uv could not synchronize the locked Python environment; retrying offline.\n' >&2
  uv sync --locked --offline
fi
actual_python="$(uv run --locked python --version 2>&1 | awk 'NR == 1 { print $2; exit }' || true)"
if [[ "$actual_python" != "$expected_python" ]]; then
  printf 'Expected Python %s, found %s.\n' "$expected_python" "$actual_python" >&2
  exit 1
fi
printf 'Python: %s\n' "$actual_python"

# Prove the interpreter is an isolated environment rather than a floating system
# install. A system interpreter reports the same prefix and base prefix.
python_isolation="$(
  uv run --locked python -c 'import sys; print(sys.prefix); print(sys.base_prefix)' 2>/dev/null |
    awk 'NR == 1 { prefix = $0 } NR == 2 { base = $0 } END { print prefix "|" base }' || true
)"
python_prefix="${python_isolation%%|*}"
python_base_prefix="${python_isolation##*|}"
if [[ -z "$python_prefix" || "$python_prefix" == "$python_base_prefix" ]]; then
  printf 'Expected an isolated pinned Python environment, found a floating interpreter at %s.\n' "$python_prefix" >&2
  exit 1
fi
printf 'Python environment: %s\n' "$python_prefix"

# Reports the wall-clock cost of every stage so a slow run is attributable to a
# specific stage instead of looking like a hung gate.
run_stage() {
  local label="$1"
  shift
  local started ended status=0
  started="$(date +%s)"
  printf '==> %s\n' "$label"
  "$@" || status=$?
  ended="$(date +%s)"
  if ((status == 0)); then
    printf '    %s passed in %ss\n' "$label" "$((ended - started))"
  else
    printf '    %s FAILED in %ss (exit %s)\n' "$label" "$((ended - started))" "$status" >&2
  fi
  return "$status"
}

# Cargo parallelism is bounded on shared development hosts so one gate run does
# not starve the other work on the same machine. CI keeps the cargo default.
cargo_jobs=()
if [[ -n "${LITECOWORK_CARGO_JOBS:-}" ]]; then
  cargo_jobs=(--jobs "$LITECOWORK_CARGO_JOBS")
fi

# Generators are run in freshness-check mode: compare their output with the
# current reviewed artifacts, then restore any stale output so local checks do
# not silently rewrite tracked files. A CI failure asks the author to regenerate
# and review those artifacts explicitly.
run_freshness_check() {
  local -a outputs=()
  while (($#)) && [[ "$1" != "--" ]]; do
    outputs+=("$1")
    shift
  done
  if (($# == 0)); then
    printf 'run_freshness_check requires -- before its command.\n' >&2
    return 2
  fi
  shift
  local -a command=("$@")
  local snapshot_dir
  snapshot_dir="$(mktemp -d "${TMPDIR:-/tmp}/litecowork-ci-generated.XXXXXX")"
  local -a existed=()
  local index output
  for index in "${!outputs[@]}"; do
    output="${outputs[$index]}"
    if [[ -f "$output" ]]; then
      mkdir -p "$snapshot_dir/$(dirname "$output")"
      cp --preserve=mode,timestamps -- "$output" "$snapshot_dir/$output"
      existed[$index]=1
    else
      existed[$index]=0
    fi
  done

  local command_status=0
  "${command[@]}" || command_status=$?
  local stale=0
  for index in "${!outputs[@]}"; do
    output="${outputs[$index]}"
    if [[ "${existed[$index]}" == 1 ]]; then
      if ! cmp -s -- "$snapshot_dir/$output" "$output"; then
        cp --preserve=mode,timestamps -- "$snapshot_dir/$output" "$output"
        stale=1
      fi
    elif [[ -e "$output" ]]; then
      rm -- "$output"
      stale=1
    fi
  done
  rm -rf -- "$snapshot_dir"

  if ((stale)); then
    printf 'Generated output is stale; regenerate and review: %s\n' "${outputs[*]}" >&2
    return 1
  fi
  return "$command_status"
}

if ((preflight_only == 0)); then
  run_stage "Rust format" cargo fmt --all -- --check
  run_stage "Rust lint" cargo clippy --workspace --all-targets --locked "${cargo_jobs[@]}" -- -D warnings
  run_stage "Rust tests" cargo test --workspace --locked "${cargo_jobs[@]}"
  run_stage "Rust build" cargo build --workspace --locked "${cargo_jobs[@]}"
fi

run_stage "Python tests" uv run --locked python -m unittest discover -s tests -v
run_stage "V1 contract audit" \
  run_freshness_check \
  implementation/V1-CONTRACT-CHECK.md -- \
  uv run --locked python scripts/audit_v1_contracts.py --strict --write-report
run_stage "V1 markdown inventory" \
  run_freshness_check \
  implementation/V1-AUDIT-INVENTORY.md \
  implementation/V1-MARKDOWN-INVENTORY.csv -- \
  uv run --locked python scripts/audit_v1_markdown.py
run_stage "Architecture coverage" \
  run_freshness_check \
  implementation/ARCHITECTURE-COVERAGE.md \
  implementation/coverage.csv -- \
  uv run --locked python scripts/validate_implementation_coverage.py --write
# Coverage generation updates ARCHITECTURE-COVERAGE.md, which the Markdown
# inventory hashes. Recheck the inventory after that dependency is regenerated.
run_stage "V1 markdown inventory recheck" \
  run_freshness_check \
  implementation/V1-AUDIT-INVENTORY.md \
  implementation/V1-MARKDOWN-INVENTORY.csv -- \
  uv run --locked python scripts/audit_v1_markdown.py
run_stage "V1 design depth audit" \
  run_freshness_check \
  implementation/V1-DESIGN-DEPTH-AUDIT.md -- \
  uv run --locked python scripts/audit_v1_design_depth.py --strict --write
run_stage "Design owner consistency" \
  run_freshness_check \
  implementation/V1-CROSS-CONTRACT-GUARD.json -- \
  uv run --locked python scripts/validate_design_owner_consistency.py
run_stage "Architecture coverage validation" \
  uv run --locked python scripts/validate_implementation_coverage.py
run_stage "Implementation plan validation" \
  uv run --locked python scripts/validate_implementation_plan.py
run_stage "Architecture validation" \
  uv run --locked python scripts/validate_architecture.py

# The reviewed markdown inventory CSV intentionally retains its generated
# CRLF format; do not report its line-ending CR as trailing whitespace.
run_stage "Whitespace check" \
  git -c core.whitespace=cr-at-eol diff --check

printf 'check.sh: all requested stages passed.\n'
