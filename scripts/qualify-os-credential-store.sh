#!/usr/bin/env bash
# Platform qualification for the LiteCowork OS credential-store boundary.
#
# The provider code in `crates/storage-sqlite/src/os_key_provider.rs` is covered
# by deterministic CODE tests that inject a credential store. Those tests prove
# the provider's rules; they do not prove that a real operating-system
# credential service accepts, retains, and releases the same material. This
# script runs the ignored qualification test against the live platform
# provider and records the exact host, provider, and result.
#
# Usage:
#   scripts/qualify-os-credential-store.sh
#
# Exit codes:
#   0  the live platform credential store passed the qualification
#   3  the host has no usable credential service (not qualified, not a failure)
#   1  the qualification ran and failed
set -Eeuo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

log() {
  printf '%s\n' "$*"
}

fail() {
  printf 'error: %s\n' "$*" >&2
  exit 1
}

log "LiteCowork OS credential-store qualification"
log "Host: $(uname -srm)"
if [[ -r /etc/os-release ]]; then
  # shellcheck disable=SC1091
  . /etc/os-release
  log "Distribution: ${PRETTY_NAME:-unknown}"
fi
log "Rust: $(rustc --version)"
log "Cargo: $(cargo --version)"
log "Date: $(date -u +%Y-%m-%dT%H:%M:%SZ)"

case "$(uname -s)" in
  Linux) ;;
  *)
    log "This qualifier qualifies the Linux Secret Service backend."
    log "Other platforms are qualified by their own platform lane."
    exit 3
    ;;
esac

# The provider constructs the persistent Secret Service credential directly, so
# qualification needs a reachable session bus and the Secret Service collection.
session_bus="${DBUS_SESSION_BUS_ADDRESS:-}"
if [[ -z "$session_bus" && -S "/run/user/$(id -u)/bus" ]]; then
  session_bus="unix:path=/run/user/$(id -u)/bus"
  export DBUS_SESSION_BUS_ADDRESS="$session_bus"
fi

if [[ -z "$session_bus" ]]; then
  log "No D-Bus session bus is available on this host."
  log "The Secret Service credential backend cannot be reached; not qualified."
  exit 3
fi
log "Session bus: $session_bus"

secret_service_probe=0
if command -v busctl >/dev/null 2>&1; then
  if busctl --user --no-pager list org.freedesktop.secrets >/dev/null 2>&1; then
    secret_service_probe=1
    log "Secret Service: reachable (org.freedesktop.secrets)"
  fi
elif command -v gdbus >/dev/null 2>&1; then
  if gdbus call --session --dest org.freedesktop.secrets \
    --object-path /org/freedesktop/secrets \
    --method org.freedesktop.DBus.Peer.Ping >/dev/null 2>&1; then
    secret_service_probe=1
    log "Secret Service: reachable (org.freedesktop.secrets)"
  fi
fi

if ((secret_service_probe == 0)); then
  log "The org.freedesktop.secrets service is not reachable on this session bus."
  log "The Secret Service credential backend cannot be exercised; not qualified."
  exit 3
fi

if ! command -v secret-tool >/dev/null 2>&1; then
  log "note: secret-tool is not installed; the qualification does not require it."
fi

log "Running the live credential-store qualification test..."
set +e
LITECOWORK_RUN_LINUX_SECRET_SERVICE_TEST=1 cargo test --locked --offline --jobs 2 \
  -p storage-sqlite --test linux_secret_service -- --ignored --nocapture --test-threads=1
status=$?
set -e

if ((status != 0)); then
  log "Qualification FAILED (cargo exit $status)."
  exit 1
fi

log "Qualification PASSED: the live Secret Service stored, retained, and released"
log "the provider's versioned blob keys, and rejected a malformed active pointer."
exit 0
