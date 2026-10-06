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

cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --workspace --locked

uv run --locked python -m unittest discover -s tests -v
uv run --locked python scripts/validate_architecture.py
uv run --locked python scripts/validate_implementation_plan.py

git diff --check
