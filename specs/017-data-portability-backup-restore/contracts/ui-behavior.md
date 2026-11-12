# UI behavior - Settings Storage & Backup and Deletion status (Feature 017)

**Status:** IN_REVIEW - pending owner acceptance of B-41; does not authorize implementation; not a claim of implementation or qualification. Binds UI to owner contracts; not pixels.
**Normative UI indexes:** [V1-UI-SCREEN-CONTRACT.md](../../../docs/V1-UI-SCREEN-CONTRACT.md) (Settings category 13 "Storage & Backup"), [V1-SETTINGS-CONTRACT-LEDGER.md](../../../docs/V1-SETTINGS-CONTRACT-LEDGER.md) (rows `previewBackup`, `exportWorkspace`, `verifyManifest`, `importBackup`, `previewRestore`, `confirmRestore`, `inspectStorageUsage`), [DESKTOP-SHELL-LLD.md](../../../docs/DESKTOP-SHELL-LLD.md).
**Wire:** [api-events.md](api-events.md).

The UI renders operation and manifest projections and sends typed commands. It never handles archive bytes, keys or paths beyond a display label, and it renders errors by code, never by English message text.

## 1. Screen and controls

Settings > Storage & Backup (new category registered in the Feature 011 Settings shell). The category is omitted on a release row where its routes are not served.

| Control | Command | Enabled when | Pending | Success | Failure |
|---|---|---|---|---|---|
| Create backup | `preview_workspace_backup` then `create_workspace_backup` | Workspace selected, no nonterminal operation, Runtime ready | Progress bar with phase and bytes; Cancel visible | Row added with "Verified" and time | Typed error message and repair action |
| Mode choice (Installation backup / Portable export) | part of create | Portable shown only when served on this row | n/a | n/a | n/a |
| Passphrase + confirm | part of create, verify, import | Length at least 12 (daemon enforces) | Field cleared on submit | n/a | `BACKUP_PASSPHRASE_REJECTED` re-prompts |
| Cancel | `cancel_backup_operation` | Operation QUEUED or RUNNING | "Cancelling" | "Cancelled" or committed result | `STALE_VERSION` refreshes |
| Verify archive | `verify_backup_archive` | No nonterminal operation | Progress | "Archive is valid" with authenticated Workspace name and schema; verification does not import data | Typed error |
| Import Workspace | `import_workspace_backup` | A completed Verify result identifies the exact Workspace; owner confirms its name, selects the archive again and re-enters the passphrase | Progress | New Workspace listed; prompt to re-authenticate Connections | `WORKSPACE_IMPORT_CONFLICT` explains no overwrite; mismatched archive id is rejected |
| Restore installation (offline) | `restore_installation_offline` | Row has qualified quiescence gate | "Stopping Runtime", "Restoring", "Restarting" | Restore receipt shown | Abort message; original installation restarted |
| Check data compatibility | `check_data_compatibility` | Always | Spinner | Verdict and reason | Typed error |
| Deletion status (sub-view) | `list_workspace_deletion_status` | Workspace selected | Loading rows | Rows by state | Typed error |
| Retry purge | owner route via `context_document_deletion_bridge.rs` | Row state PENDING or FAILED | "Retrying" | Row stays PENDING until ACKNOWLEDGED | Typed error |

Destructive confirmations: Import and Restore require typing the Workspace name (import) or the word RESTORE (restore) and show what will not be carried (secrets, grants, sessions, leases).

## 2. States

| Surface | States | Meaning |
|---|---|---|
| Backup list | `LOADING`, `EMPTY` ("No backups yet"), `READY`, `ERROR`, `DENIED`, `OFFLINE`, `STALE` | EMPTY shows no size or date placeholders; STALE shows the observation age |
| Manifest row | `VERIFIED`, `MISSING` | MISSING means the archive file is gone; not restorable |
| Operation | `QUEUED`, `RUNNING(phase)`, `CANCELLING`, `SUCCEEDED`, `FAILED(code)`, `CANCELLED`, `INTERRUPTED` | INTERRUPTED comes from the startup sweep |
| Deletion row | `PENDING`, `FAILED`, `ACKNOWLEDGED` | Only ACKNOWLEDGED displays "Purged" |
| Runtime | `UNAVAILABLE`, `DEGRADED` | Create/import disabled with the reason; offline restore still possible when the gate is qualified |

## 3. Flow transitions

| Step | Surface | Transition |
|---|---|---|
| F1 | Open category | LOADING -> EMPTY or READY |
| F2 | Create | Preview -> passphrase -> 202 -> RUNNING -> SUCCEEDED or FAILED |
| F3 | Cancel | RUNNING -> CANCELLING -> CANCELLED or committed result |
| F4 | App closed mid-operation | On reopen, operation shown from the daemon; never inferred success |
| F5 | Daemon crashed mid-operation | INTERRUPTED with "Start again" |
| F6 | Import | Verify selected file and passphrase -> show authenticated Workspace identity -> owner confirms exact name -> re-enter passphrase -> import only if the manifest identity still matches and is absent locally -> new Workspace or conflict |
| F7 | Offline restore | Choose file -> passphrase -> typed RESTORE -> stop gate -> restore -> restart -> receipt |
| F8 | Deletion status | Rows -> Retry -> PENDING until ACKNOWLEDGED |

## 4. Accessibility

Every control has an accessible name; disabled controls state why. Progress uses a polite live region announcing phase changes, not every byte. Keyboard order follows the visual order; Escape closes dialogs and returns focus to the invoking control. Typed confirmations have labelled inputs. Color is never the only state signal. Text scales to 200% without loss.

## 5. Privacy in the UI

The passphrase input is masked, has autocomplete off, is cleared on submit and on unmount, and is never stored in component state after the command returns. File paths are shown as the file name only.
