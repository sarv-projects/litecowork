# LiteCowork desktop/local V1 release qualification matrix

This file is generated. Do not edit it by hand: run the command below to regenerate it.

```
.venv/bin/python scripts/v1_platform_matrix.py --out implementation/QUALIFICATION-MATRIX.md --evidence-out /tmp/litecowork-release-evidence
```

- Generated at: `2026-10-21T19:18:31Z`
- Probing host: `Linux 6.18.54.1-microsoft-standard-WSL2 x86_64` (python 3.13.12)
- A cell is `PASSED` only when the probe actually ran on this host and observed success. Every other cell carries the reason it is not a pass.
- `NOT RUN` means the probe exists but was not executed here.
- `BLOCKED` means the environment cannot run the probe at all.

## Toolchain observed on the probing host

| Tool | Observed |
|---|---|
| cargo | `cargo 1.98.1 (797e8a9bc 2026-08-05)` |
| node | `v22.23.1` |
| pnpm | `12.11.2` |
| python | `Python 3.12.3` |
| rustc | `rustc 1.98.1 (48a229cea 2026-09-01)` |

## Platform capability matrix

| Capability | linux/x86_64 (wsl2) | windows/x86_64 | macos/aarch64 |
|---|---|---|---|
| native_conversation_send | BLOCKED | BLOCKED | BLOCKED |
| producer_scoped_planner_isolation | BLOCKED | BLOCKED | BLOCKED |
| workspace_root_revalidation | BLOCKED | BLOCKED | BLOCKED |
| coworker_private_learner | BLOCKED | BLOCKED | BLOCKED |
| browser_takeover | BLOCKED | BLOCKED | BLOCKED |
| terminal_pty | BLOCKED | BLOCKED | BLOCKED |
| effect_reconciliation | BLOCKED | BLOCKED | BLOCKED |
| independent_verification | BLOCKED | BLOCKED | BLOCKED |
| backup_restore | PASSED | BLOCKED | BLOCKED |
| packaging | PASSED | BLOCKED | BLOCKED |
| secret_scan | PASSED | BLOCKED | BLOCKED |

> **Host note:** this probing host is a WSL2 environment, not a native Linux desktop kernel. Process isolation, file identity, PTY and browser takeover behaviour on WSL2 is not the behaviour on a native Linux desktop, so a cell probed here is not a native Linux desktop qualification.

### Per-cell evidence for the probed host

#### native_conversation_send

- State: **BLOCKED**
- Command: `see implementation/reviews/E13-S03.md`
- Observation:

```
no org.freedesktop.secrets service is registered
```
- Reason: the OS credential store is unavailable on this host, so the daemon cannot reach a state where this capability can be exercised

#### producer_scoped_planner_isolation

- State: **BLOCKED**
- Command: `see implementation/reviews/E13-S03.md`
- Observation:

```
no org.freedesktop.secrets service is registered
```
- Reason: the OS credential store is unavailable on this host, so the daemon cannot reach a state where this capability can be exercised

#### workspace_root_revalidation

- State: **BLOCKED**
- Command: `see implementation/reviews/E13-S03.md`
- Observation:

```
no org.freedesktop.secrets service is registered
```
- Reason: the OS credential store is unavailable on this host, so the daemon cannot reach a state where this capability can be exercised

#### coworker_private_learner

- State: **BLOCKED**
- Command: `see implementation/reviews/E13-S03.md`
- Observation:

```
no org.freedesktop.secrets service is registered
```
- Reason: the OS credential store is unavailable on this host, so the daemon cannot reach a state where this capability can be exercised

#### browser_takeover

- State: **BLOCKED**
- Command: `see implementation/reviews/E13-S03.md`
- Observation:

```
no org.freedesktop.secrets service is registered
```
- Reason: the OS credential store is unavailable on this host, so the daemon cannot reach a state where this capability can be exercised

#### terminal_pty

- State: **BLOCKED**
- Command: `see implementation/reviews/E13-S03.md`
- Observation:

```
no org.freedesktop.secrets service is registered
```
- Reason: the OS credential store is unavailable on this host, so the daemon cannot reach a state where this capability can be exercised

#### effect_reconciliation

- State: **BLOCKED**
- Command: `see implementation/reviews/E13-S03.md`
- Observation:

```
no org.freedesktop.secrets service is registered
```
- Reason: the OS credential store is unavailable on this host, so the daemon cannot reach a state where this capability can be exercised

#### independent_verification

- State: **BLOCKED**
- Command: `see implementation/reviews/E13-S03.md`
- Observation:

```
no org.freedesktop.secrets service is registered
```
- Reason: the OS credential store is unavailable on this host, so the daemon cannot reach a state where this capability can be exercised

#### backup_restore

- State: **PASSED**
- Command: `cargo test --locked --offline --jobs 2 -p storage-sqlite backup::`
- Observation:

```
warning: `storage-sqlite` (lib) generated 12 warnings (7 duplicates)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 2m 21s
     Running unittests src/lib.rs (target/debug/deps/storage_sqlite-67974cd58333153e)
     Running tests/agent_registry_cache.rs (target/debug/deps/agent_registry_cache-0129b60b8c5171a0)
     Running tests/linux_secret_service.rs (target/debug/deps/linux_secret_service-f423a68f2ff2610d)
     Running tests/windows_credential_manager.rs (target/debug/deps/windows_credential_manager-55114d3642447fc3)
```

#### packaging

- State: **PASSED**
- Command: `/home/sarvesh/business_Dev/litecowork/.venv/bin/python scripts/v1_packaging_check.py`
- Observation:

```
[PASS] backup recovery surface: `litecoworkd backup` exposes create, verify, restore and upgrade-check
[PASS] state directory permissions: the daemon creates a 0700 state directory and a 0600 database
[PASS] service manager unit: the unit file is present and systemd-analyze verify accepted it (the ExecStart path is not installed on this host)
[PASS] updater trust: no updater endpoint is configured, so no unsigned update can be offered
[PASS] desktop build inputs: the desktop build inputs are present
8/8 checks passed
```

#### secret_scan

- State: **PASSED**
- Command: `/home/sarvesh/business_Dev/litecowork/.venv/bin/python scripts/v1_secret_scan.py`
- Observation:

```
scanned 1 files, 0 findings
```

## Agent and harness matrix

An agent is only supported when its exact version passes the native harness conformance on the exact operating system. A detected version is an observation, not a qualification.

| Agent | Executable | Observed version | State |
|---|---|---|---|
| aider | `aider` | `aider 0.86.2` | PRESENT |
| amp | `-` | `-` | NOT INSTALLED |
| antigravity | `agy` | `1.2.1` | PRESENT |
| auggie-cli | `-` | `-` | NOT INSTALLED |
| claude-code | `-` | `-` | NOT INSTALLED |
| cline | `cline` | `3.0.73` | PRESENT |
| codex | `codex` | `codex-cli 0.163.0` | PRESENT |
| command-code | `commandcode` | `1.46.0` | PRESENT |
| copilot | `copilot` | `GitHub Copilot CLI 1.0.78.` | PRESENT |
| cursor-cli | `-` | `-` | NOT INSTALLED |
| devin-cli | `-` | `-` | NOT INSTALLED |
| droid | `droid` | `0.151.0` | PRESENT |
| goose | `-` | `-` | NOT INSTALLED |
| grok-build | `grok` | `grok 1.0.44 (5b807183dd79) [stable]` | PRESENT |
| junie-cli | `junie` | `-` | PRESENT BUT UNREPORTED |
| kimi-code | `-` | `-` | NOT INSTALLED |
| kiro-cli | `kiro-cli` | `kiro-cli 2.16.1` | PRESENT |
| mimocode | `mimo` | `0.1.14` | PRESENT |
| minimax-code | `-` | `-` | NOT INSTALLED |
| mistral-vibe | `vibe-acp` | `vibe-acp 2.25.0` | PRESENT |
| muse-code | `muse` | `Muse Code 1.4.11 (1.4.11-R6667.1)` | PRESENT |
| oh-my-pi | `omp` | `omp/18.12.2` | PRESENT |
| opencode | `opencode` | `opencode v2.1.2` | PRESENT |
| pi | `pi` | `1.3.0` | PRESENT |
| qwen | `qwen` | `0.21.3` | PRESENT |

## Browser matrix

| Browser | Executable | Observed version | State |
|---|---|---|---|
| chromium | `-` | `-` | NOT INSTALLED |
| firefox | `-` | `-` | NOT INSTALLED |
| google-chrome | `google-chrome` | `Google Chrome 149.0.7827.200` | PRESENT |
| microsoft-edge | `-` | `-` | NOT INSTALLED |

## Cells that are not a pass

| Capability | State | Reason |
|---|---|---|
| native_conversation_send | BLOCKED | the OS credential store is unavailable on this host, so the daemon cannot reach a state where this capability can be exercised |
| producer_scoped_planner_isolation | BLOCKED | the OS credential store is unavailable on this host, so the daemon cannot reach a state where this capability can be exercised |
| workspace_root_revalidation | BLOCKED | the OS credential store is unavailable on this host, so the daemon cannot reach a state where this capability can be exercised |
| coworker_private_learner | BLOCKED | the OS credential store is unavailable on this host, so the daemon cannot reach a state where this capability can be exercised |
| browser_takeover | BLOCKED | the OS credential store is unavailable on this host, so the daemon cannot reach a state where this capability can be exercised |
| terminal_pty | BLOCKED | the OS credential store is unavailable on this host, so the daemon cannot reach a state where this capability can be exercised |
| effect_reconciliation | BLOCKED | the OS credential store is unavailable on this host, so the daemon cannot reach a state where this capability can be exercised |
| independent_verification | BLOCKED | the OS credential store is unavailable on this host, so the daemon cannot reach a state where this capability can be exercised |
