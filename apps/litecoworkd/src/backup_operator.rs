//! E13-S01 — the local backup/restore recovery surface of `litecoworkd`.
//!
//! The recovery surface is a locally authenticated command line on the daemon
//! binary. It is deliberately separate from the Operator HTTP API: a restore
//! replaces the installation's durable state, so it must run with the daemon
//! stopped and must never be reachable over the Operator socket.
//!
//! Every command prints exactly one JSON document on stdout. The container
//! passphrase is read from a file that the operator controls; it is never
//! echoed, never logged, and never written into the archive or its name.

use std::{
    fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

use serde_json::{Value, json};
use storage_sqlite::{
    BackupCreateRequest, BackupPassphrase, LocalBackupService, OsWorkspaceBlobKeyProvider,
};

/// Usage text for the backup recovery surface.
pub const HELP: &str = "litecoworkd backup — local backup, verification and restore

USAGE:
    litecoworkd backup create --data-dir <dir> --workspace <id> --backup-id <id> --passphrase-file <path>
    litecoworkd backup verify --archive <path> --passphrase-file <path>
    litecoworkd backup restore --archive <path> --destination <dir> --passphrase-file <path>
    litecoworkd backup upgrade-check --data-dir <dir>

`create` writes an encrypted backup container into <dir>/backups and registers a
verified manifest row. `verify` re-reads the container and re-checks every
digest without changing anything. `restore` verifies and stages the container
before it replaces <destination>, which must be an empty or missing directory.
The Runtime must be stopped: restore refuses to run while the destination
installation holds its single-instance lock.
";

/// Name of the daemon single-instance lock file inside a state directory.
const RUNTIME_LOCK_FILE: &str = "runtime.lock";

/// Runs a backup subcommand. Returns the process exit code.
pub fn run(arguments: &[String]) -> ExitCode {
    if arguments.is_empty() || arguments == ["--help"] || arguments == ["-h"] {
        print!("{HELP}");
        return ExitCode::SUCCESS;
    }
    match execute(arguments) {
        Ok(document) => {
            println!("{document}");
            ExitCode::SUCCESS
        }
        Err(message) => {
            let document = json!({
                "status": "error",
                "error": message,
            });
            eprintln!("{document}");
            ExitCode::FAILURE
        }
    }
}

fn execute(arguments: &[String]) -> Result<String, String> {
    let Some(subcommand) = arguments.first().map(String::as_str) else {
        return Err("a backup subcommand is required".to_owned());
    };
    let options = parse_options(&arguments[1..])?;
    match subcommand {
        "create" => create(&options),
        "verify" => verify(&options),
        "restore" => restore(&options),
        "upgrade-check" => upgrade_check(&options),
        other => Err(format!("unsupported backup subcommand '{other}'")),
    }
}

/// Parses the `--name value` option pairs of a subcommand.
fn parse_options(arguments: &[String]) -> Result<Vec<(String, String)>, String> {
    let mut options = Vec::new();
    let mut index = 0;
    while index < arguments.len() {
        let argument = &arguments[index];
        let Some(name) = argument.strip_prefix("--") else {
            return Err(format!("unexpected argument '{argument}'"));
        };
        if name.is_empty() {
            return Err("an option name is required".to_owned());
        }
        let Some(value) = arguments.get(index + 1) else {
            return Err(format!("option '--{name}' requires a value"));
        };
        options.push((name.to_owned(), value.clone()));
        index += 2;
    }
    Ok(options)
}

fn required<'a>(options: &'a [(String, String)], name: &str) -> Result<&'a str, String> {
    options
        .iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.as_str())
        .ok_or_else(|| format!("option '--{name}' is required"))
}

fn optional<'a>(options: &'a [(String, String)], name: &str) -> Option<&'a str> {
    options
        .iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.as_str())
}

/// Reads the container passphrase from the operator controlled file.
///
/// The passphrase is never echoed. A missing or empty file is an error: an
/// empty passphrase would produce an archive that anyone can open.
fn read_passphrase(path: &Path) -> Result<BackupPassphrase, String> {
    let bytes = fs::read(path)
        .map_err(|error| format!("could not read the passphrase file: {error}"))?;
    let text = String::from_utf8(bytes)
        .map_err(|_| "the passphrase file is not valid UTF-8".to_owned())?;
    let trimmed = text.trim_end_matches(['\r', '\n']);
    if trimmed.is_empty() {
        return Err("the passphrase file is empty".to_owned());
    }
    Ok(BackupPassphrase::new(trimmed))
}

fn create(options: &[(String, String)]) -> Result<String, String> {
    let data_directory = PathBuf::from(required(options, "data-dir")?);
    let workspace_id = required(options, "workspace")?.to_owned();
    let backup_id = required(options, "backup-id")?.to_owned();
    let passphrase = read_passphrase(&PathBuf::from(required(options, "passphrase-file")?))?;

    let service = LocalBackupService::new(&data_directory);
    let request = BackupCreateRequest {
        backup_id,
        workspace_id,
        passphrase,
        kdf_iterations: storage_core::BACKUP_KDF_DEFAULT_ITERATIONS,
        now: now_timestamp(),
    };
    let outcome = service
        .create_backup(&OsWorkspaceBlobKeyProvider, &request)
        .map_err(|error| format!("backup creation failed: {error}"))?;
    Ok(render(json!({
        "status": "ok",
        "operation": "backup.create",
        "backup_id": outcome.backup_id,
        "workspace_id": outcome.workspace_id,
        "schema_version": outcome.schema_version,
        "archive_path": outcome.archive_path,
        "archive_size_bytes": outcome.archive_size_bytes,
        "blob_count": outcome.blob_count,
        "database_digest": outcome.database_digest,
        "integrity_digest": outcome.integrity_digest,
        "created_at": outcome.created_at,
        "manifest_row_recorded": outcome.manifest_row_recorded,
    })))
}

fn verify(options: &[(String, String)]) -> Result<String, String> {
    let archive = PathBuf::from(required(options, "archive")?);
    let passphrase = read_passphrase(&PathBuf::from(required(options, "passphrase-file")?))?;
    let service = LocalBackupService::new(std::env::current_dir().unwrap_or_default());
    let verification = service
        .verify_backup(&archive, &passphrase)
        .map_err(|error| format!("backup verification failed: {error}"))?;
    Ok(render(json!({
        "status": "ok",
        "operation": "backup.verify",
        "backup_id": verification.backup_id,
        "workspace_id": verification.workspace_id,
        "schema_version": verification.schema_version,
        "database_digest": verification.database_digest,
        "integrity_digest": verification.integrity_digest,
        "blob_count": verification.blob_count,
        "key_reference_count": verification.key_reference_count,
        "excluded_table_count": verification.excluded_table_count,
        "verified_at": verification.verified_at,
        "schema_compatible": verification.compatibility.is_compatible(),
        "schema_compatibility_reason": verification.compatibility.reason(),
    })))
}

fn restore(options: &[(String, String)]) -> Result<String, String> {
    let archive = PathBuf::from(required(options, "archive")?);
    let destination = PathBuf::from(required(options, "destination")?);
    let passphrase = read_passphrase(&PathBuf::from(required(options, "passphrase-file")?))?;
    let service = LocalBackupService::new(std::env::current_dir().unwrap_or_default());
    let outcome = service
        .restore_backup(
            &archive,
            &passphrase,
            &destination,
            &OsWorkspaceBlobKeyProvider,
            Some(&destination.join(RUNTIME_LOCK_FILE)),
        )
        .map_err(|error| format!("backup restore failed: {error}"))?;
    Ok(render(json!({
        "status": "ok",
        "operation": "backup.restore",
        "backup_id": outcome.backup_id,
        "workspace_id": outcome.workspace_id,
        "schema_version": outcome.schema_version,
        "restored_blob_count": outcome.restored_blob_count,
        "revoked_grant_count": outcome.revoked_grant_count,
        "dropped_lease_count": outcome.dropped_lease_count,
        "restored_at": outcome.restored_at,
        "new_runtime_identity_required": outcome.new_runtime_identity_required,
    })))
}

fn upgrade_check(options: &[(String, String)]) -> Result<String, String> {
    let data_directory = PathBuf::from(required(options, "data-dir")?);
    let service = LocalBackupService::new(&data_directory);
    let compatibility = service
        .check_upgrade_compatibility()
        .map_err(|error| format!("upgrade check failed: {error}"))?;
    Ok(render(json!({
        "status": "ok",
        "operation": "backup.upgrade-check",
        "schema_compatible": compatibility.is_compatible(),
        "schema_compatibility_reason": compatibility.reason(),
    })))
}

fn now_timestamp() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_owned())
}

fn render(value: Value) -> String {
    serde_json::to_string_pretty(&value)
        .unwrap_or_else(|_| "{\"status\":\"error\",\"error\":\"could not encode the result\"}".to_owned())
}
