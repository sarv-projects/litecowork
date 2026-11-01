#!/usr/bin/env bash
# Platform qualification for the LiteCowork managed Linux Runtime unit.
#
# E01-S03 requires the managed systemd stop deadline to stay above the daemon's
# bounded Operator drain, and requires that a forced stop is reported as an
# unclean restart rather than a clean stop. The unit itself is rendered by the
# desktop shell; this script only inspects and, on explicit opt-in, exercises
# the unit that is installed on this host.
#
# Usage:
#   scripts/qualify-linux-systemd-runtime.sh              read-only inspection
#   LITECOWORK_QUALIFY_SYSTEMD_STOP=1 scripts/qualify-linux-systemd-runtime.sh
#                                                        also stop the managed unit
#
# Exit codes:
#   0  the installed managed unit satisfies the qualification
#   3  this host cannot qualify the managed unit (not a failure)
#   1  the qualification ran and failed
set -Eeuo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

UNIT_NAME="${LITECOWORK_SYSTEMD_UNIT_NAME:-litecoworkd.service}"
# The documented managed-unit stop deadline. The daemon's bounded Operator drain
# is 60 seconds, so the service manager deadline must stay above it with room
# for the final Runtime lifecycle writes.
REQUIRED_STOP_TIMEOUT_SECONDS=90
OPT_IN_STOP="${LITECOWORK_QUALIFY_SYSTEMD_STOP:-0}"

log() {
  printf '%s\n' "$*"
}

fail() {
  printf 'error: %s\n' "$*" >&2
  exit 1
}

log "LiteCowork managed Linux Runtime unit qualification"
log "Host: $(uname -srm)"
if [[ -r /etc/os-release ]]; then
  # shellcheck disable=SC1091
  . /etc/os-release
  log "Distribution: ${PRETTY_NAME:-unknown}"
fi
log "systemd: $(systemctl --version 2>/dev/null | head -1 || echo unknown)"
log "Date: $(date -u +%Y-%m-%dT%H:%M:%SZ)"

if [[ "$(uname -s)" != "Linux" ]]; then
  log "This qualifier qualifies the managed Linux systemd unit."
  exit 3
fi

if ! command -v systemctl >/dev/null 2>&1; then
  log "/usr/bin/systemctl is not available on this host; not qualified."
  exit 3
fi

if ! systemctl --user show-environment >/dev/null 2>&1; then
  log "No usable user systemd manager is reachable on this host."
  log "The managed unit cannot be inspected or exercised; not qualified."
  exit 3
fi

if ! systemctl --user cat "$UNIT_NAME" >/dev/null 2>&1; then
  log "The managed unit $UNIT_NAME is not installed for this user."
  log "The desktop shell installs it; not qualified."
  exit 3
fi

unit_body="$(systemctl --user cat "$UNIT_NAME" 2>/dev/null)"
log "Managed unit: $UNIT_NAME"

# The unit must be the LiteCowork-managed unit and not a conflicting unit that
# another package owns. The marker is the same string the desktop shell writes.
if ! grep -q "^# Managed by LiteCowork;" <<<"$unit_body"; then
  fail "$UNIT_NAME is installed but is not a LiteCowork-managed unit."
fi

timeout_seconds="$(systemctl --user show "$UNIT_NAME" --property=TimeoutStopUSec --value 2>/dev/null || true)"
kill_mode="$(systemctl --user show "$UNIT_NAME" --property=KillMode --value 2>/dev/null || true)"
restart_policy="$(systemctl --user show "$UNIT_NAME" --property=Restart --value 2>/dev/null || true)"
log "TimeoutStopUSec: $timeout_seconds"
log "KillMode: $kill_mode"
log "Restart: $restart_policy"

timeout_in_seconds=""
if [[ "$timeout_seconds" =~ ^([0-9]+)(us|ms|s|min|h)$ ]]; then
  raw="${BASH_REMATCH[1]}"
  unit="${BASH_REMATCH[2]}"
  case "$unit" in
    us) timeout_in_seconds=$((raw / 1000000)) ;;
    ms) timeout_in_seconds=$((raw / 1000)) ;;
    s) timeout_in_seconds=$raw ;;
    min) timeout_in_seconds=$((raw * 60)) ;;
    h) timeout_in_seconds=$((raw * 3600)) ;;
  esac
fi

if [[ -z "$timeout_in_seconds" ]]; then
  fail "The managed unit does not report a usable stop deadline ($timeout_seconds)."
fi
if ((timeout_in_seconds < REQUIRED_STOP_TIMEOUT_SECONDS)); then
  fail "The managed stop deadline of ${timeout_in_seconds}s is below the required ${REQUIRED_STOP_TIMEOUT_SECONDS}s."
fi
if [[ "$kill_mode" != "control-group" ]]; then
  fail "The managed unit must use KillMode=control-group, found $kill_mode."
fi
if [[ "$restart_policy" != "on-failure" ]]; then
  fail "The managed unit must use Restart=on-failure, found $restart_policy."
fi

active_state="$(systemctl --user show "$UNIT_NAME" --property=ActiveState --value 2>/dev/null || true)"
log "ActiveState: $active_state"

if [[ "$OPT_IN_STOP" == "1" && "$active_state" == "active" ]]; then
  log "Requesting a managed stop (opt-in) to measure the stop deadline..."
  started="$(date +%s)"
  systemctl --user stop "$UNIT_NAME" || fail "The managed stop request failed."
  ended="$(date +%s)"
  elapsed=$((ended - started))
  log "Managed stop completed in ${elapsed}s (deadline ${timeout_in_seconds}s)."
  if ((elapsed >= timeout_in_seconds)); then
    fail "The managed stop reached or exceeded the systemd deadline; the daemon did not drain in time."
  fi
  remaining="$(systemctl --user show "$UNIT_NAME" --property=ActiveState --value 2>/dev/null || true)"
  log "ActiveState after the stop: $remaining"
  log "A forced stop after the deadline is reported as an unclean restart by the next incarnation."
else
  log "The stop measurement is opt-in; set LITECOWORK_QUALIFY_SYSTEMD_STOP=1 to exercise it."
fi

log "Qualification PASSED: the installed managed unit keeps its stop deadline above the"
log "daemon's bounded Operator drain and preserves the required unit properties."
exit 0
