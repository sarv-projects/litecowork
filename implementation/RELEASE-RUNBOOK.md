# LiteCowork desktop/local V1 — release, recovery and restore runbook

This runbook is the exact recovery procedure for the local desktop product. It
is written for the repository owner. Every command below is copy-pasteable and
every claim in it was executed on the reference Linux host unless the text says
otherwise.

Scope: desktop/local V1 only. Cloud continuation and remote Runtime are
post-V1 and are not covered here.

- Evidence and story status: [`implementation/reviews/E13-S01.md`](reviews/E13-S01.md),
  [`E13-S02.md`](reviews/E13-S02.md), [`E13-S03.md`](reviews/E13-S03.md),
  [`E13-S04.md`](reviews/E13-S04.md).
- Generated qualification matrix: [`implementation/QUALIFICATION-MATRIX.md`](QUALIFICATION-MATRIX.md).
- Contract authority: [`docs/STORAGE.md`](../docs/STORAGE.md) "Backup and disaster
  recovery", [`docs/SECURITY.md`](../docs/SECURITY.md),
  [`implementation/RELEASE.md`](RELEASE.md).

---

## 1. What the local install owns

| Path (inside the state directory) | Contents | In a backup? |
|---|---|---|
| `litecowork.sqlite3` (+ `-wal`, `-shm`) | the durable store, the event journal and the projections | filtered snapshot only |
| `blobs/<scope>/<purpose>/<digest>.blob` | encrypted content-addressed blobs | yes, Workspace scope only |
| `runtime-state.json` | the installation's Runtime id and lifecycle state | no (it is installation-local) |
| `runtime.lock` | the single-instance lock | no |
| `backups/<id>.lcbbackup` | the encrypted backup containers this install produced | no (it is the output) |

The OS credential store holds the workspace blob keys. **The backup never
contains key material.** A backup whose referenced key is unavailable cannot be
restored, and the restore refuses it instead of producing a half-restored
installation.

---

## 2. Back up

Prerequisites:

- the daemon is stopped (`litecoworkd status --data-dir <dir>` shows the
  installation; stop the daemon before a backup if you want a quiesced copy —
  the backup itself is safe to run against a live daemon),
- a passphrase file that only the owner can read. The passphrase is **never**
  written into the archive, into the archive name, or into a log line.

```bash
# 1. Create a passphrase file with 0600 permissions, owned by the runtime user.
umask 077
printf '%s' 'your-passphrase' > "$HOME/.litecowork-backup-passphrase"

# 2. Create the backup.
./target/release/litecoworkd backup create \
  --data-dir "$HOME/.local/share/LiteCowork" \
  --workspace "<workspace-id>" \
  --backup-id "2026-10-20-pre-release" \
  --passphrase-file "$HOME/.litecowork-backup-passphrase"
```

The command prints one JSON document. A successful run reports
`"status":"ok"`, the archive path, the archive size, the blob count and the
integrity digest. `"manifest_row_recorded":true` means the verified manifest was
also written to the `workspace_backup_manifests` table of the live store.

The archive is written to `<data-dir>/backups/<backup-id>.lcbbackup`. The
archive name contains the backup id only.

---

## 3. Verify a backup (read only)

```bash
./target/release/litecoworkd backup verify \
  --archive "$HOME/.local/share/LiteCowork/backups/2026-10-20-pre-release.lcbbackup" \
  --passphrase-file "$HOME/.litecowork-backup-passphrase"
```

The verification re-reads the container, authenticates every chunk, re-computes
every digest, re-checks the manifest integrity digest and reports the schema
compatibility verdict. It changes nothing.

A verification fails when:

| Failure | Reported as |
|---|---|
| wrong passphrase | `backup manifest failed authentication; the archive is corrupt or the passphrase is wrong` |
| corrupted archive | `a backup payload failed authentication` or `the backup database snapshot does not match its manifest digest` |
| foreign or newer schema | `schema_compatible:false` with the reason |
| truncated archive | `the backup archive is truncated` |

---

## 4. Restore

Prerequisites:

- the destination is a **fresh installation**: an empty or non-existent
  directory. A restore into a populated directory is refused so an existing
  installation can never be half-overwritten,
- the destination Runtime is stopped. A restore refuses to run while the
  destination holds its single-instance lock,
- the OS credential store of the destination holds the workspace blob key that
  the backup references. Losing that key makes the backup unrestorable.

```bash
# 1. Stop the destination Runtime (if one is running).
systemctl --user stop litecoworkd    # or: pkill -f 'litecoworkd run'

# 2. Restore into the fresh installation directory.
./target/release/litecoworkd backup restore \
  --archive "/media/backup/2026-10-20-pre-release.lcbbackup" \
  --destination "$HOME/.local/share/LiteCowork" \
  --passphrase-file "$HOME/.litecowork-backup-passphrase"
```

What the restore does, in order:

1. refuses if the destination installation holds its single-instance lock,
2. refuses if the destination directory is not empty,
3. reads and authenticates the whole container and re-computes every digest,
4. stages the snapshot and every blob envelope into a private staging directory,
5. re-verifies the staged database (`PRAGMA integrity_check`,
   `PRAGMA foreign_key_check`) and re-checks the schema version against the
   Runtime's supported range,
6. revokes every restored capability grant and fences every restored Runtime
   incarnation, so **no pre-backup authority is live again**,
7. proves that every restored blob still decrypts with the referenced key
   version and still matches its recorded digest,
8. only then replaces the destination.

If any step fails, the destination is left exactly as it was and the staging
directory is removed.

A restored installation has **no `runtime-state.json`**, so the daemon
registers a **new Runtime identity and a new Runtime incarnation** on its first
start. The restored database keeps the historical rows for provenance, but the
restored Runtime rows are marked `STOPPED`/`OFFLINE` with no current
incarnation, and the previous `current_incarnation_id` is cleared.

The restore reports `"new_runtime_identity_required":true`.

After the restore, start the Runtime normally:

```bash
./target/release/litecoworkd run --data-dir "$HOME/.local/share/LiteCowork"
```

Providers must be re-attached before an Environment can be used again. Local
bindings (agent endpoint bindings, provider input bindings, environment
provider bindings, resource location bindings) are never restored.

---

## 5. Safe upgrades

```bash
./target/release/litecoworkd backup upgrade-check --data-dir "$HOME/.local/share/LiteCowork"
```

The upgrade check refuses an unknown schema version:

- a database written by a **newer** Runtime is refused (`schema version N is
  newer than the maximum supported version M`) — the Runtime does not know the
  newer invariants,
- a database **older** than the oldest supported baseline is refused.

The check does not migrate anything. The daemon applies the repository's own
migration chain when it opens the store, and it refuses the same way.

An **incompatible downgrade is refused**: a store written by a newer Runtime
cannot be opened by an older one.

---

## 6. Incident playbooks

The playbooks below extend [`implementation/RELEASE.md`](RELEASE.md) with the
exact local commands.

### Lost or corrupt disk

First safe action: stop the Runtime, do not start it against the damaged disk.

```bash
systemctl --user stop litecoworkd
./target/release/litecoworkd backup verify --archive "<archive>" --passphrase-file "<file>"
```

Recovery proof: the restore refuses a corrupt archive before it touches the
destination, and the restored installation starts with a new Runtime identity.

### Bad update or migration

First safe action: stop the Runtime and keep the pinned work.

```bash
./target/release/litecoworkd backup upgrade-check --data-dir "<data-dir>"
```

Recovery proof: the upgrade check refuses an unknown schema version instead of
mutating the store. Restore the previous installation from the last verified
backup.

### Suspected secret exposure

First safe action: stop the Runtime and rotate the OS credential store entry
for the affected workspace. The backup archives are encrypted with an operator
passphrase that is not in the store; rotate that passphrase too.

### Private logs

Diagnostics exports are owner-previewed and redacted. The secret scan over the
build output is the check that no credential-shaped material shipped:

```bash
.venv/bin/python scripts/v1_secret_scan.py
```

---

## 7. Uninstall and data retention

Uninstalling the desktop bundle does **not** delete the state directory. The
owner chooses:

| Choice | Command | Effect |
|---|---|---|
| keep the data | stop the Runtime, remove the bundle | the state directory and the OS credential store entry stay |
| delete the data | stop the Runtime, remove the bundle, then `rm -rf "$HOME/.local/share/LiteCowork"` | the local store and the local blobs are deleted; the OS credential store entry must be removed separately |

---

## 8. Known limitations of this runbook

These are true today and are not workarounds for a missing feature:

1. The backup container is **passphrase encrypted**. The passphrase is supplied
   by the operator and is never stored. There is no key escrow and no
   unattended backup yet.
2. The blob scope of a backup is **one Workspace**. The database snapshot
   currently carries the rows of every Workspace in the installation, because
   row-level Workspace filtering of the snapshot is not implemented yet. A
   multi-Workspace installation should be treated as a single backup unit until
   that filtering lands.
3. The backup is a **local** file. There is no cloud copy and no scheduled
   backup yet.
4. The restore does not restore the OS credential store. On a different OS user
   or a different machine, the workspace blob key is missing and the restore
   refuses with `the encryption key for ... is unavailable`.
5. The restore is refused while the destination Runtime holds its
   single-instance lock. There is no forced restore.
6. No installer, signature, updater or notarization exists in this repository.
   The service-manager unit is shipped as an artifact
   (`scripts/packaging/systemd/litecoworkd.service`) but is never installed by
   the repository.
