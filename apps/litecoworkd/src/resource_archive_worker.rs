//! Fixed, bounded bridge from the local Runtime to the isolated ZIP preview capability.
//!
//! This bridge accepts bytes and a pinned source identity only. It never accepts a host
//! path, URL, executable, environment override, or extraction mode. The Python CLI invokes
//! the Bubblewrap supervisor; the daemon owns a second deadline and kills its whole process
//! group if the adapter stalls or emits an oversized response.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
#[cfg(all(target_os = "linux", feature = "source-zip-worker"))]
use std::{
    io::{Read, Write},
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
use unicode_casefold::UnicodeCaseFold;
use unicode_normalization::UnicodeNormalization;

const MAX_ARCHIVE_BYTES: usize = 16 * 1024 * 1024;
const MAX_REQUEST_BYTES: usize = 24 * 1024 * 1024;
#[cfg(all(target_os = "linux", feature = "source-zip-worker"))]
const MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;
#[cfg(all(target_os = "linux", feature = "source-zip-worker"))]
const OUTER_DEADLINE: Duration = Duration::from_secs(15);
#[cfg(all(target_os = "linux", feature = "source-zip-worker"))]
const PYTHON: &str = "/usr/bin/python3";
#[cfg(all(target_os = "linux", feature = "source-zip-worker"))]
const CLI: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../capabilities/zip_intake/runtime_cli.py"
);

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SourceRevision {
    pub workspace_id: String,
    pub resource_id: String,
    pub revision_id: String,
    pub content_sha256: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ZipLimits {
    pub max_archive_bytes: usize,
    pub max_entries: usize,
    pub max_entry_bytes: usize,
    pub max_expanded_bytes: usize,
    pub max_compression_ratio: usize,
    pub max_path_bytes: usize,
    pub max_path_depth: usize,
    pub max_seconds: f64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ZipEntryReport {
    pub entry_index: usize,
    pub name_sha256: String,
    pub virtual_path: Option<String>,
    pub compressed_bytes: usize,
    pub declared_bytes: usize,
    pub crc32: u32,
    pub status: String,
    pub reason: Option<String>,
    pub observed_bytes: Option<usize>,
    pub content_sha256: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ZipManifest {
    pub source: SourceRevision,
    pub provider_version: String,
    pub archive_bytes: usize,
    pub limits: ZipLimits,
    pub entries: Vec<ZipEntryReport>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum ZipWorkerError {
    ArchiveRejected(String),
    Timeout,
    OutputLimit,
    Unavailable,
    Failed,
}

pub(super) fn preview(
    source: &SourceRevision,
    archive: &[u8],
) -> Result<ZipManifest, ZipWorkerError> {
    if archive.len() > MAX_ARCHIVE_BYTES {
        return Err(ZipWorkerError::ArchiveRejected(
            "ARCHIVE_SIZE_LIMIT".to_owned(),
        ));
    }
    let request = json!({
        "operation": "preview",
        "source": source,
        "archive_b64": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, archive),
    });
    let request = serde_json::to_vec(&request).map_err(|_| ZipWorkerError::Failed)?;
    if request.len() > MAX_REQUEST_BYTES {
        return Err(ZipWorkerError::ArchiveRejected(
            "REQUEST_SIZE_LIMIT".to_owned(),
        ));
    }

    #[cfg(not(all(target_os = "linux", feature = "source-zip-worker")))]
    {
        let _ = request;
        Err(ZipWorkerError::Unavailable)
    }

    #[cfg(all(target_os = "linux", feature = "source-zip-worker"))]
    {
        let response = run_cli(&request)?;
        parse_manifest(response, source, archive.len())
    }
}

#[cfg(all(target_os = "linux", feature = "source-zip-worker"))]
fn run_cli(request: &[u8]) -> Result<Value, ZipWorkerError> {
    if !std::path::Path::new(PYTHON).is_file() || !std::path::Path::new(CLI).is_file() {
        return Err(ZipWorkerError::Unavailable);
    }
    let mut command = Command::new(PYTHON);
    command
        .arg("-I")
        .arg(CLI)
        .current_dir("/")
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    use std::os::unix::process::CommandExt;
    command.process_group(0);
    let child = command.spawn().map_err(|_| ZipWorkerError::Unavailable)?;
    let mut child = ChildGuard::new(child);
    let mut stdin = child
        .child
        .stdin
        .take()
        .ok_or(ZipWorkerError::Unavailable)?;
    let mut stdout = child
        .child
        .stdout
        .take()
        .ok_or(ZipWorkerError::Unavailable)?;

    let oversized = Arc::new(AtomicBool::new(false));
    let reader_oversized = Arc::clone(&oversized);
    let reader = thread::spawn(move || -> std::io::Result<Vec<u8>> {
        let mut bytes = Vec::with_capacity(MAX_RESPONSE_BYTES.min(64 * 1024));
        stdout
            .by_ref()
            .take((MAX_RESPONSE_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
        if bytes.len() > MAX_RESPONSE_BYTES {
            reader_oversized.store(true, Ordering::Release);
        }
        Ok(bytes)
    });
    let request = request.to_vec();
    let writer = thread::spawn(move || -> std::io::Result<()> {
        stdin.write_all(&request)?;
        stdin.flush()?;
        Ok(())
    });

    let deadline = Instant::now() + OUTER_DEADLINE;
    let exit_status = loop {
        if oversized.load(Ordering::Acquire) {
            child.terminate();
            return Err(ZipWorkerError::OutputLimit);
        }
        if Instant::now() >= deadline {
            child.terminate();
            return Err(ZipWorkerError::Timeout);
        }
        match child.child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => thread::sleep(Duration::from_millis(10)),
            Err(_) => {
                child.terminate();
                return Err(ZipWorkerError::Failed);
            }
        }
    };

    while !reader.is_finished() || !writer.is_finished() {
        if oversized.load(Ordering::Acquire) {
            child.terminate();
            return Err(ZipWorkerError::OutputLimit);
        }
        if Instant::now() >= deadline {
            child.terminate();
            return Err(ZipWorkerError::Timeout);
        }
        thread::sleep(Duration::from_millis(10));
    }
    child.disarm();
    let write_result = writer.join().map_err(|_| ZipWorkerError::Failed)?;
    if write_result.is_err() {
        return Err(ZipWorkerError::Failed);
    }
    let output = reader
        .join()
        .map_err(|_| ZipWorkerError::Failed)?
        .map_err(|_| ZipWorkerError::Failed)?;
    if output.len() > MAX_RESPONSE_BYTES {
        return Err(ZipWorkerError::OutputLimit);
    }
    if !exit_status.success() {
        return Err(ZipWorkerError::Failed);
    }
    let response: Value = serde_json::from_slice(&output).map_err(|_| ZipWorkerError::Failed)?;
    if !response.is_object() {
        return Err(ZipWorkerError::Failed);
    }
    if response.get("ok") == Some(&Value::Bool(false)) {
        let code = response
            .get("code")
            .and_then(Value::as_str)
            .ok_or(ZipWorkerError::Failed)?;
        return match code {
            "TIMEOUT" => Err(ZipWorkerError::Timeout),
            "DEPENDENCY_UNAVAILABLE" => Err(ZipWorkerError::Unavailable),
            "CANCELLED" => Err(ZipWorkerError::Failed),
            value if safe_reason_code(value) => {
                Err(ZipWorkerError::ArchiveRejected(value.to_owned()))
            }
            _ => Err(ZipWorkerError::Failed),
        };
    }
    if response.get("ok") != Some(&Value::Bool(true))
        || response.as_object().is_none_or(|object| object.len() != 2)
    {
        return Err(ZipWorkerError::Failed);
    }
    response
        .get("manifest")
        .cloned()
        .ok_or(ZipWorkerError::Failed)
}

#[cfg(all(target_os = "linux", feature = "source-zip-worker"))]
struct ChildGuard {
    child: Child,
    armed: bool,
}

#[cfg(all(target_os = "linux", feature = "source-zip-worker"))]
impl ChildGuard {
    fn new(child: Child) -> Self {
        Self { child, armed: true }
    }

    fn terminate(&mut self) {
        if let Some(group) =
            rustix::process::Pid::from_raw(self.child.id() as rustix::process::RawPid)
        {
            let _ = rustix::process::kill_process_group(group, rustix::process::Signal::KILL);
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
        self.armed = false;
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

#[cfg(all(target_os = "linux", feature = "source-zip-worker"))]
impl Drop for ChildGuard {
    fn drop(&mut self) {
        if self.armed {
            self.terminate();
        }
    }
}

fn parse_manifest(
    value: Value,
    expected_source: &SourceRevision,
    expected_archive_bytes: usize,
) -> Result<ZipManifest, ZipWorkerError> {
    let manifest: ZipManifest =
        serde_json::from_value(value).map_err(|_| ZipWorkerError::Failed)?;
    if &manifest.source != expected_source
        || manifest.archive_bytes != expected_archive_bytes
        || manifest.provider_version != "litecowork-zip-intake/0.1.0"
        || manifest.limits.max_archive_bytes != MAX_ARCHIVE_BYTES
        || manifest.limits.max_entries != 2048
        || manifest.limits.max_entry_bytes != 8 * 1024 * 1024
        || manifest.limits.max_expanded_bytes != 32 * 1024 * 1024
        || manifest.limits.max_compression_ratio != 100
        || manifest.limits.max_path_bytes != 1024
        || manifest.limits.max_path_depth != 32
        || manifest.limits.max_seconds != 10.0
        || manifest.entries.len() > manifest.limits.max_entries
    {
        return Err(ZipWorkerError::Failed);
    }
    let mut declared_total = 0usize;
    let mut compressed_total = 0usize;
    let mut paths: HashMap<String, Vec<&ZipEntryReport>> = HashMap::new();
    for (index, entry) in manifest.entries.iter().enumerate() {
        declared_total = declared_total
            .checked_add(entry.declared_bytes)
            .ok_or(ZipWorkerError::Failed)?;
        compressed_total = compressed_total
            .checked_add(entry.compressed_bytes)
            .ok_or(ZipWorkerError::Failed)?;
        if entry.entry_index != index
            || entry.name_sha256.len() != 64
            || !entry
                .name_sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
            || entry.compressed_bytes > manifest.archive_bytes
            || entry.declared_bytes > manifest.limits.max_expanded_bytes
            || !matches!(entry.status.as_str(), "READY" | "DIRECTORY" | "REJECTED")
            || entry
                .reason
                .as_deref()
                .is_some_and(|reason| !safe_reason_code(reason))
            || entry
                .virtual_path
                .as_deref()
                .is_some_and(|path| !safe_virtual_path(path, &manifest.limits))
            || (entry.status == "REJECTED" && entry.reason.is_none())
            || (entry.status != "REJECTED" && entry.reason.is_some())
            || entry.observed_bytes.is_some()
            || entry.content_sha256.is_some()
        {
            return Err(ZipWorkerError::Failed);
        }
        match entry.status.as_str() {
            "READY" => {
                if entry.virtual_path.is_none()
                    || entry.declared_bytes > manifest.limits.max_entry_bytes
                    || entry.declared_bytes
                        > manifest
                            .limits
                            .max_compression_ratio
                            .saturating_mul(entry.compressed_bytes.max(1))
                {
                    return Err(ZipWorkerError::Failed);
                }
            }
            "DIRECTORY" => {
                // ZIP writers may DEFLATE an empty directory marker. It must
                // have zero expanded bytes, but its compressed stream need not
                // be empty.
                if entry.virtual_path.is_none() || entry.declared_bytes != 0 {
                    return Err(ZipWorkerError::Failed);
                }
            }
            "REJECTED" => {}
            _ => return Err(ZipWorkerError::Failed),
        }
        if let Some(path) = entry.virtual_path.as_deref() {
            paths
                .entry(portable_path_key(path))
                .or_default()
                .push(entry);
        }
    }
    for (key, colliding) in &paths {
        if colliding.len() > 1 && !all_path_conflicts(colliding) {
            return Err(ZipWorkerError::Failed);
        }
        let parts = key.split('/').collect::<Vec<_>>();
        for depth in 1..parts.len() {
            let parent = parts[..depth].join("/");
            if let Some(ancestors) = paths.get(&parent)
                && ancestors.iter().any(|entry| entry.status != "DIRECTORY")
                && (!all_path_conflicts(ancestors) || !all_path_conflicts(colliding))
            {
                return Err(ZipWorkerError::Failed);
            }
        }
    }
    if declared_total > manifest.limits.max_expanded_bytes
        || compressed_total > manifest.archive_bytes
    {
        return Err(ZipWorkerError::Failed);
    }
    Ok(manifest)
}

fn safe_reason_code(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value.is_ascii()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
}

fn safe_virtual_path(value: &str, limits: &ZipLimits) -> bool {
    if value.is_empty()
        || value.starts_with('/')
        || value
            .chars()
            .any(|character| matches!(character, '\\' | ':'))
        || value.len() > limits.max_path_bytes
        || value.chars().any(|character| character.is_control())
    {
        return false;
    }
    let parts = value.split('/').collect::<Vec<_>>();
    parts.len() <= limits.max_path_depth
        && parts.iter().all(|part| {
            let normalized_component: String = part.nfkc().case_fold().collect();
            let device_stem = normalized_component.split('.').next().unwrap_or_default();
            !part.is_empty()
                && part.nfc().collect::<String>() == *part
                && *part != "."
                && *part != ".."
                && !part.ends_with(' ')
                && !part.ends_with('.')
                && !is_windows_reserved_component(device_stem)
                && !part
                    .chars()
                    .any(|character| matches!(character, '<' | '>' | '"' | '|' | '?' | '*'))
        })
}

fn portable_path_key(path: &str) -> String {
    path.nfkc().case_fold().collect()
}

fn all_path_conflicts(entries: &[&ZipEntryReport]) -> bool {
    entries
        .iter()
        .all(|entry| entry.status == "REJECTED" && entry.reason.as_deref() == Some("PATH_CONFLICT"))
}

fn is_windows_reserved_component(stem: &str) -> bool {
    matches!(stem, "con" | "prn" | "aux" | "nul")
        || (stem.len() == 4
            && (stem.starts_with("com") || stem.starts_with("lpt"))
            && stem.as_bytes()[3].is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::{SourceRevision, ZipLimits, parse_manifest, safe_reason_code, safe_virtual_path};
    use serde_json::json;

    fn source() -> SourceRevision {
        SourceRevision {
            workspace_id: "workspace-1".to_owned(),
            resource_id: "resource-1".to_owned(),
            revision_id: "revision-1".to_owned(),
            content_sha256: "a".repeat(64),
        }
    }

    fn limits() -> ZipLimits {
        ZipLimits {
            max_archive_bytes: 16 * 1024 * 1024,
            max_entries: 2048,
            max_entry_bytes: 8 * 1024 * 1024,
            max_expanded_bytes: 32 * 1024 * 1024,
            max_compression_ratio: 100,
            max_path_bytes: 1024,
            max_path_depth: 32,
            max_seconds: 10.0,
        }
    }

    fn manifest(path: &str) -> serde_json::Value {
        json!({
            "source": source(),
            "provider_version": "litecowork-zip-intake/0.1.0",
            "archive_bytes": 128,
            "limits": limits(),
            "entries": [{
                "entry_index": 0,
                "name_sha256": "b".repeat(64),
                "virtual_path": path,
                "compressed_bytes": 10,
                "declared_bytes": 11,
                "crc32": 1,
                "status": "READY",
                "reason": null,
                "observed_bytes": null,
                "content_sha256": null,
            }],
        })
    }

    #[test]
    fn manifest_parser_binds_the_exact_revision_and_rejects_hostile_paths() {
        let expected = source();
        assert!(parse_manifest(manifest("docs/readme.txt"), &expected, 128).is_ok());
        for path in [
            "../outside.txt",
            "/absolute.txt",
            "dir\\outside.txt",
            "dir/../outside.txt",
        ] {
            assert!(
                parse_manifest(manifest(path), &expected, 128).is_err(),
                "{path}"
            );
        }
        assert!(parse_manifest(manifest("docs/readme.txt"), &expected, 127).is_err());
    }

    #[test]
    fn manifest_parser_rejects_windows_reserved_components() {
        let expected = source();
        for path in ["CON.txt", "dir/NUL", "PRN.log", "COM1.csv", "LPT9"] {
            assert!(
                parse_manifest(manifest(path), &expected, 128).is_err(),
                "reserved Windows path component accepted: {path}"
            );
        }
    }

    #[test]
    fn manifest_parser_accepts_compressed_empty_directory_markers() {
        let expected = source();
        let mut value = manifest("folder");
        value["entries"][0]["status"] = json!("DIRECTORY");
        value["entries"][0]["declared_bytes"] = json!(0);
        value["entries"][0]["compressed_bytes"] = json!(2);

        assert!(parse_manifest(value, &expected, 128).is_ok());
    }

    #[test]
    fn manifest_parser_requires_nfkc_casefold_path_conflicts_to_be_quarantined() {
        let expected = source();
        for (first_path, second_path) in [("A.txt", "Ａ.txt"), ("Straße.txt", "STRASSE.txt")] {
            let mut value = manifest(first_path);
            let mut second = value["entries"][0].clone();
            second["entry_index"] = json!(1);
            second["name_sha256"] = json!("c".repeat(64));
            second["virtual_path"] = json!(second_path);
            value["entries"].as_array_mut().unwrap().push(second);

            assert!(
                parse_manifest(value, &expected, 128).is_err(),
                "normalization alias accepted: {first_path:?} and {second_path:?}"
            );
        }
    }

    #[test]
    fn manifest_parser_rejects_unquarantined_file_ancestor_conflicts() {
        let expected = source();
        let mut value = manifest("docs");
        let mut child = value["entries"][0].clone();
        child["entry_index"] = json!(1);
        child["name_sha256"] = json!("c".repeat(64));
        child["virtual_path"] = json!("docs/readme.txt");
        value["entries"].as_array_mut().unwrap().push(child);

        assert!(parse_manifest(value, &expected, 128).is_err());
    }

    #[test]
    fn worker_reason_codes_are_safe_tokens_only() {
        assert!(safe_reason_code("UNSAFE_PATH"));
        assert!(!safe_reason_code("archive member: /etc/passwd"));
        assert!(!safe_reason_code(""));
        assert!(!safe_virtual_path("a/../b", &limits()));
    }

    #[test]
    fn preview_parser_rejects_payload_fields_and_status_reason_contradictions() {
        let expected = source();
        let mut malformed_digest = manifest("docs/readme.txt");
        malformed_digest["entries"][0]["content_sha256"] = json!("SHA256:bad");
        assert!(parse_manifest(malformed_digest, &expected, 128).is_err());

        let mut unexpected_length = manifest("docs/readme.txt");
        unexpected_length["entries"][0]["observed_bytes"] = json!(11);
        assert!(parse_manifest(unexpected_length, &expected, 128).is_err());

        let mut missing_rejection_reason = manifest("docs/readme.txt");
        missing_rejection_reason["entries"][0]["status"] = json!("REJECTED");
        assert!(parse_manifest(missing_rejection_reason, &expected, 128).is_err());

        let mut reason_on_ready = manifest("docs/readme.txt");
        reason_on_ready["entries"][0]["reason"] = json!("UNSAFE_PATH");
        assert!(parse_manifest(reason_on_ready, &expected, 128).is_err());

        let mut extracted_preview_entry = manifest("docs/readme.txt");
        extracted_preview_entry["entries"][0]["status"] = json!("EXTRACTED");
        assert!(parse_manifest(extracted_preview_entry, &expected, 128).is_err());
    }

    #[test]
    fn duplicate_preview_paths_must_all_be_rejected_as_conflicts() {
        let expected = source();
        let mut unsafe_collision = manifest("docs/readme.txt");
        let mut second = unsafe_collision["entries"][0].clone();
        second["entry_index"] = json!(1);
        unsafe_collision["entries"]
            .as_array_mut()
            .expect("entries array")
            .push(second.clone());
        assert!(parse_manifest(unsafe_collision, &expected, 128).is_err());

        let mut reported_collision = manifest("docs/readme.txt");
        reported_collision["entries"][0]["status"] = json!("REJECTED");
        reported_collision["entries"][0]["reason"] = json!("PATH_CONFLICT");
        second["status"] = json!("REJECTED");
        second["reason"] = json!("PATH_CONFLICT");
        reported_collision["entries"]
            .as_array_mut()
            .expect("entries array")
            .push(second);
        assert!(parse_manifest(reported_collision, &expected, 128).is_ok());
    }
}
