#!/usr/bin/env bash
set -Eeuo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

expected_uv="$(<.uv-version)"
actual_uv="$(uv --version | awk '{print $2}')"
if [[ "$actual_uv" != "$expected_uv" ]]; then
  printf 'Expected uv %s, found %s.\n' "$expected_uv" "$actual_uv" >&2
  exit 1
fi

expected_rust="$(sed -n 's/^channel = "\(.*\)"$/\1/p' rust-toolchain.toml)"
actual_rust="$(rustc --version | awk '{print $2}')"
if [[ -z "$expected_rust" || "$actual_rust" != "$expected_rust" ]]; then
  printf 'Expected Rust %s, found %s.\n' "$expected_rust" "$actual_rust" >&2
  exit 1
fi

actual_cargo="$(cargo --version | awk '{print $2}')"
if [[ "$actual_cargo" != "$expected_rust" ]]; then
  printf 'Expected Cargo %s, found %s.\n' "$expected_rust" "$actual_cargo" >&2
  exit 1
fi

printf 'Rust: %s\n' "$(rustc --version)"
printf 'Cargo: %s\n' "$(cargo --version)"
printf 'uv: %s\n' "$(uv --version)"
printf 'Python pin: %s\n' "$(<.python-version)"

uv sync --locked
actual_python="$(uv run --locked python --version | awk '{print $2}')"
expected_python="$(<.python-version)"
if [[ "$actual_python" != "$expected_python" ]]; then
  printf 'Expected Python %s, found %s.\n' "$expected_python" "$actual_python" >&2
  exit 1
fi
printf 'Python: %s\n' "$actual_python"

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

cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --workspace --locked

uv run --locked python -m unittest discover -s tests -v
run_freshness_check \
  implementation/V1-CONTRACT-CHECK.md -- \
  uv run --locked python scripts/audit_v1_contracts.py --strict --write-report
run_freshness_check \
  implementation/ARCHITECTURE-COVERAGE.md \
  implementation/coverage.csv -- \
  uv run --locked python scripts/validate_implementation_coverage.py --write
run_freshness_check \
  implementation/V1-AUDIT-INVENTORY.md \
  implementation/V1-MARKDOWN-INVENTORY.csv -- \
  uv run --locked python scripts/audit_v1_markdown.py
run_freshness_check \
  implementation/V1-DESIGN-DEPTH-AUDIT.md -- \
  uv run --locked python scripts/audit_v1_design_depth.py --strict --write
run_freshness_check \
  implementation/V1-CROSS-CONTRACT-GUARD.json -- \
  uv run --locked python scripts/validate_design_owner_consistency.py
uv run --locked python scripts/validate_implementation_coverage.py
uv run --locked python scripts/validate_implementation_plan.py
uv run --locked python scripts/validate_architecture.py

# The reviewed markdown inventory CSV intentionally retains its generated
# CRLF format; do not report its line-ending CR as trailing whitespace.
git -c core.whitespace=cr-at-eol diff --check
