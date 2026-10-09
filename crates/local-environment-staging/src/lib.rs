//! Bounded materialization of already-authorized, exact-pinned Resource bytes.
//!
//! This crate is below Resource authorization and does not resolve logical Resource
//! paths. Callers must obtain bytes from the ResourceStore for the exact Workspace,
//! Resource, revision, and digest. The staged directory and read-only permissions are
//! conveniences for preparation and preview only; they are not OS confinement, a write
//! quota, an isolation attestation, or grounds for admitting an Agent or Attempt.
//! Non-Unix targets fail closed until owner-only staging ACL behavior is qualified.

use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use storage_core::{PinnedResourceRef, ResourceStore, StoreError, StoredResourceContent, TaskView};
use unicode_casefold::UnicodeCaseFold;
use unicode_normalization::UnicodeNormalization;

static ROOT_NONCE: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreparationStatus {
    /// Inputs were copied and checked. No isolation or dispatch proof was produced.
    PreparedOnly,
}

#[derive(Clone, Debug)]
pub struct PinnedInput<'a> {
    pub workspace_id: String,
    pub resource_id: String,
    pub revision_id: String,
    /// Expected format is `sha256:<lowercase hex>`.
    pub content_digest: String,
    /// Relative destination within the staged read-only input tree.
    pub relative_path: String,
    /// Exact bytes already resolved and authorized by the caller's ResourceStore.
    pub bytes: &'a [u8],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StagingLimits {
    pub max_files: usize,
    pub max_file_bytes: u64,
    pub max_total_input_bytes: u64,
    pub max_path_bytes: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StagingError {
    InvalidParent,
    InvalidWorkspace,
    UnsupportedPlatform,
    InvalidPin,
    InvalidDigest,
    DigestMismatch,
    InvalidRelativePath(String),
    DuplicatePin,
    DuplicatePath,
    InputLimitExceeded,
    Io(String),
}

/// Minimal read boundary used by Task input preparation. Production callers normally use
/// a ResourceStore; the narrow port also keeps this adapter independently testable.
pub trait ExactResourceReader {
    fn read_exact(
        &self,
        workspace_id: &str,
        resource_id: &str,
        revision_id: &str,
        maximum_bytes: u64,
    ) -> Result<Option<StoredResourceContent>, StoreError>;
}

impl<R: ResourceStore> ExactResourceReader for R {
    fn read_exact(
        &self,
        workspace_id: &str,
        resource_id: &str,
        revision_id: &str,
        maximum_bytes: u64,
    ) -> Result<Option<StoredResourceContent>, StoreError> {
        self.read_resource_content_bounded(
            workspace_id,
            resource_id,
            Some(revision_id),
            maximum_bytes,
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TaskInputPreparationError {
    StaleTaskRevision,
    InvalidTaskInput,
    DuplicatePin,
    ResourceUnavailable,
    ResourceMetadataMismatch,
    InputLimitExceeded,
    Staging(StagingError),
}

impl fmt::Display for TaskInputPreparationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for TaskInputPreparationError {}

/// Resolves and stages only the exact immutable Resource revisions pinned by the
/// selected TaskSpec. The caller must authenticate the Workspace owner and load the
/// current Task view before calling. This returns `PreparedOnly`; it is not an
/// Environment, does not create domain state, and must not admit a process or Attempt.
pub fn stage_task_spec_inputs<R: ExactResourceReader>(
    reader: &R,
    task: &TaskView,
    expected_workspace_id: &str,
    expected_task_id: &str,
    expected_task_version: u64,
    expected_spec_revision: u64,
    staging_parent: &Path,
    limits: StagingLimits,
) -> Result<PreparedEnvironment, TaskInputPreparationError> {
    let task_spec = &task.current_spec_revision;
    if task.task.workspace_id != expected_workspace_id
        || task.task.task_id != expected_task_id
        || task.task.version != expected_task_version
        || task.task.current_spec_revision != expected_spec_revision
        || task_spec.workspace_id != expected_workspace_id
        || task_spec.task_id != expected_task_id
        || task_spec.revision != expected_spec_revision
        || expected_task_version == 0
        || expected_spec_revision == 0
    {
        return Err(TaskInputPreparationError::StaleTaskRevision);
    }
    if task_spec.input_refs.len() > limits.max_files {
        return Err(TaskInputPreparationError::InputLimitExceeded);
    }

    let mut pins = BTreeSet::new();
    let mut parsed = Vec::with_capacity(task_spec.input_refs.len());
    for input_ref in &task_spec.input_refs {
        let pin: PinnedResourceRef = serde_json::from_value(input_ref.clone())
            .map_err(|_| TaskInputPreparationError::InvalidTaskInput)?;
        if pin.workspace_id != expected_workspace_id
            || pin.resource_id.trim().is_empty()
            || pin.revision_id.trim().is_empty()
        {
            return Err(TaskInputPreparationError::InvalidTaskInput);
        }
        if !pins.insert((
            pin.workspace_id.clone(),
            pin.resource_id.clone(),
            pin.revision_id.clone(),
        )) {
            return Err(TaskInputPreparationError::DuplicatePin);
        }
        parsed.push(pin);
    }

    let mut total_bytes = 0_u64;
    let mut resolved = Vec::with_capacity(parsed.len());
    for pin in parsed {
        let remaining = limits
            .max_total_input_bytes
            .checked_sub(total_bytes)
            .ok_or(TaskInputPreparationError::InputLimitExceeded)?;
        let maximum_bytes = limits.max_file_bytes.min(remaining);
        if maximum_bytes == 0 {
            return Err(TaskInputPreparationError::InputLimitExceeded);
        }
        let stored = reader
            .read_exact(
                &pin.workspace_id,
                &pin.resource_id,
                &pin.revision_id,
                maximum_bytes,
            )
            .map_err(|_| TaskInputPreparationError::ResourceUnavailable)?
            .ok_or(TaskInputPreparationError::ResourceUnavailable)?;
        let summary = &stored.summary;
        if summary.workspace_id != pin.workspace_id
            || summary.resource_id != pin.resource_id
            || summary.resource_revision_id != pin.revision_id
            || summary.size_bytes != stored.content.len() as u64
            || summary.size_bytes > maximum_bytes
        {
            return Err(TaskInputPreparationError::ResourceMetadataMismatch);
        }
        total_bytes = total_bytes
            .checked_add(summary.size_bytes)
            .ok_or(TaskInputPreparationError::InputLimitExceeded)?;
        if total_bytes > limits.max_total_input_bytes {
            return Err(TaskInputPreparationError::InputLimitExceeded);
        }
        resolved.push((pin, stored));
    }

    let staged = resolved
        .iter()
        .map(|(pin, stored)| PinnedInput {
            workspace_id: pin.workspace_id.clone(),
            resource_id: pin.resource_id.clone(),
            revision_id: pin.revision_id.clone(),
            content_digest: stored.summary.content_digest.clone(),
            relative_path: stored.summary.display_name.clone(),
            bytes: stored.content.as_slice(),
        })
        .collect::<Vec<_>>();
    stage_inputs(staging_parent, expected_workspace_id, &staged, limits)
        .map_err(TaskInputPreparationError::Staging)
}

impl fmt::Display for StagingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for StagingError {}

#[derive(Debug)]
pub struct PreparedEnvironment {
    pub root: PathBuf,
    pub input_root: PathBuf,
    pub output_root: PathBuf,
    pub status: PreparationStatus,
    pub input_file_count: usize,
    pub input_bytes: u64,
}

impl PreparedEnvironment {
    /// Removes only this allocated staging root. It does not stop or fence processes.
    pub fn cleanup(self) -> Result<(), StagingError> {
        remove_private_tree(&self.root)
    }
}

/// Materializes exact-pinned bytes in a new private staging root.
///
/// `output/` is separate and writable by the owner. This helper does not enforce an
/// output byte limit, sandbox a process, or guarantee protection against another
/// same-user process racing with filesystem operations.
pub fn stage_inputs(
    staging_parent: &Path,
    expected_workspace_id: &str,
    inputs: &[PinnedInput<'_>],
    limits: StagingLimits,
) -> Result<PreparedEnvironment, StagingError> {
    validate_parent(staging_parent)?;
    if expected_workspace_id.trim().is_empty() {
        return Err(StagingError::InvalidWorkspace);
    }
    if inputs.len() > limits.max_files {
        return Err(StagingError::InputLimitExceeded);
    }

    let mut total_bytes = 0_u64;
    let mut pins = BTreeSet::new();
    let mut paths = BTreeSet::new();
    let mut files = Vec::with_capacity(inputs.len());
    for input in inputs {
        if input.workspace_id != expected_workspace_id
            || input.resource_id.trim().is_empty()
            || input.revision_id.trim().is_empty()
        {
            return Err(StagingError::InvalidPin);
        }
        if !pins.insert((
            input.workspace_id.as_str(),
            input.resource_id.as_str(),
            input.revision_id.as_str(),
        )) {
            return Err(StagingError::DuplicatePin);
        }
        let relative = validate_relative_path(&input.relative_path, limits.max_path_bytes)?;
        if !paths.insert(relative.key.clone()) {
            return Err(StagingError::DuplicatePath);
        }
        validate_digest(&input.content_digest)?;
        let size =
            u64::try_from(input.bytes.len()).map_err(|_| StagingError::InputLimitExceeded)?;
        if size > limits.max_file_bytes {
            return Err(StagingError::InputLimitExceeded);
        }
        total_bytes = total_bytes
            .checked_add(size)
            .ok_or(StagingError::InputLimitExceeded)?;
        if total_bytes > limits.max_total_input_bytes {
            return Err(StagingError::InputLimitExceeded);
        }
        files.push((relative, input));
    }
    reject_file_directory_collisions(&paths)?;

    let root = create_private_root(staging_parent)?;
    let mut guard = RootCleanupGuard::new(root.clone());
    let input_root = root.join("inputs");
    let output_root = root.join("outputs");
    create_private_dir(&input_root)?;
    create_private_dir(&output_root)?;

    for (relative, input) in files {
        if digest(input.bytes) != input.content_digest {
            return Err(StagingError::DigestMismatch);
        }
        let destination = input_root.join(relative.path);
        let parent = destination
            .parent()
            .ok_or(StagingError::InvalidRelativePath(
                input.relative_path.clone(),
            ))?;
        create_dir_tree_private(&input_root, parent)?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&destination)
            .map_err(io_error)?;
        file.write_all(input.bytes).map_err(io_error)?;
        file.sync_all().map_err(io_error)?;
        make_read_only(&destination)?;
    }
    make_tree_read_only(&input_root)?;

    guard.disarm();
    Ok(PreparedEnvironment {
        root,
        input_root,
        output_root,
        status: PreparationStatus::PreparedOnly,
        input_file_count: inputs.len(),
        input_bytes: total_bytes,
    })
}

#[derive(Debug)]
struct ValidatedPath {
    path: PathBuf,
    key: String,
}

fn validate_relative_path(value: &str, max_bytes: usize) -> Result<ValidatedPath, StagingError> {
    let invalid = || StagingError::InvalidRelativePath(value.to_owned());
    if value.is_empty() || value.len() > max_bytes || value.starts_with('/') || value.contains('\\')
    {
        return Err(invalid());
    }
    let path = Path::new(value);
    if path.is_absolute()
        || path
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(invalid());
    }

    let mut clean = Vec::new();
    let mut key_parts = Vec::new();
    for component in value.split('/') {
        if component.is_empty()
            || component == "."
            || component == ".."
            || component.ends_with(['.', ' '])
            || component
                .chars()
                .any(|ch| ch.is_control() || matches!(ch, '<' | '>' | ':' | '"' | '|' | '?' | '*'))
            || is_windows_reserved_name(component)
        {
            return Err(invalid());
        }
        clean.push(component);
        let normalized = component.nfc().collect::<String>();
        key_parts.push(normalized.case_fold().collect::<String>());
    }
    Ok(ValidatedPath {
        path: clean.iter().collect(),
        key: key_parts.join("/"),
    })
}

fn is_windows_reserved_name(component: &str) -> bool {
    let stem = component
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && matches!(stem.as_bytes()[3], b'1'..=b'9'))
}

fn reject_file_directory_collisions(paths: &BTreeSet<String>) -> Result<(), StagingError> {
    for path in paths {
        let mut prefix = String::new();
        let components = path.split('/').collect::<Vec<_>>();
        for component in components.iter().take(components.len().saturating_sub(1)) {
            if !prefix.is_empty() {
                prefix.push('/');
            }
            prefix.push_str(component);
            if paths.contains(&prefix) {
                return Err(StagingError::DuplicatePath);
            }
        }
    }
    Ok(())
}

fn validate_digest(value: &str) -> Result<(), StagingError> {
    let Some(encoded) = value.strip_prefix("sha256:") else {
        return Err(StagingError::InvalidDigest);
    };
    if encoded.len() != 64
        || !encoded
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(StagingError::InvalidDigest);
    }
    Ok(())
}

fn digest(bytes: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

fn validate_parent(path: &Path) -> Result<(), StagingError> {
    let metadata = fs::symlink_metadata(path).map_err(io_error)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(StagingError::InvalidParent);
    }
    Ok(())
}

fn create_private_root(parent: &Path) -> Result<PathBuf, StagingError> {
    for _ in 0..128 {
        let nonce = ROOT_NONCE.fetch_add(1, Ordering::Relaxed);
        let candidate = parent.join(format!("litecowork-stage-{}-{nonce}", std::process::id()));
        match create_dir_private(&candidate) {
            Ok(()) => return Ok(candidate),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(io_error(error)),
        }
    }
    Err(StagingError::Io(
        "could not allocate staging root".to_owned(),
    ))
}

fn create_private_dir(path: &Path) -> Result<(), StagingError> {
    create_dir_private(path).map_err(io_error)
}

#[cfg(unix)]
fn create_dir_private(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    let mut builder = fs::DirBuilder::new();
    builder.mode(0o700).create(path)
}

#[cfg(not(unix))]
fn create_dir_private(path: &Path) -> std::io::Result<()> {
    let _ = path;
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "owner-only staging directory permissions are not qualified on this platform",
    ))
}

fn create_dir_tree_private(root: &Path, parent: &Path) -> Result<(), StagingError> {
    let relative = parent
        .strip_prefix(root)
        .map_err(|_| StagingError::InvalidParent)?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(name) = component else {
            return Err(StagingError::InvalidParent);
        };
        current.push(name);
        match create_dir_private(&current) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                let metadata = fs::symlink_metadata(&current).map_err(io_error)?;
                if metadata.file_type().is_symlink() || !metadata.is_dir() {
                    return Err(StagingError::InvalidParent);
                }
            }
            Err(error) => return Err(io_error(error)),
        }
    }
    Ok(())
}

fn make_read_only(path: &Path) -> Result<(), StagingError> {
    let mut permissions = fs::metadata(path).map_err(io_error)?.permissions();
    permissions.set_readonly(true);
    fs::set_permissions(path, permissions).map_err(io_error)
}

fn make_tree_read_only(root: &Path) -> Result<(), StagingError> {
    let mut directories = Vec::new();
    collect_directories(root, &mut directories)?;
    for directory in directories.into_iter().rev() {
        make_read_only(&directory)?;
    }
    Ok(())
}

fn collect_directories(root: &Path, directories: &mut Vec<PathBuf>) -> Result<(), StagingError> {
    directories.push(root.to_path_buf());
    for entry in fs::read_dir(root).map_err(io_error)? {
        let path = entry.map_err(io_error)?.path();
        let metadata = fs::symlink_metadata(&path).map_err(io_error)?;
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_dir() {
            collect_directories(&path, directories)?;
        }
    }
    Ok(())
}

fn remove_private_tree(root: &Path) -> Result<(), StagingError> {
    let mut directories = Vec::new();
    match fs::symlink_metadata(root) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err(StagingError::InvalidParent);
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(io_error(error)),
    }
    collect_directories(root, &mut directories)?;
    clear_readonly_files(root)?;
    for directory in directories {
        let mut permissions = fs::metadata(&directory).map_err(io_error)?.permissions();
        permissions.set_readonly(false);
        fs::set_permissions(&directory, permissions).map_err(io_error)?;
    }
    fs::remove_dir_all(root).map_err(io_error)
}

fn clear_readonly_files(root: &Path) -> Result<(), StagingError> {
    for entry in fs::read_dir(root).map_err(io_error)? {
        let path = entry.map_err(io_error)?.path();
        let metadata = fs::symlink_metadata(&path).map_err(io_error)?;
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_dir() {
            clear_readonly_files(&path)?;
        } else if metadata.is_file() && metadata.permissions().readonly() {
            let mut permissions = metadata.permissions();
            permissions.set_readonly(false);
            fs::set_permissions(&path, permissions).map_err(io_error)?;
        }
    }
    Ok(())
}

fn io_error(error: std::io::Error) -> StagingError {
    if error.kind() == std::io::ErrorKind::Unsupported {
        StagingError::UnsupportedPlatform
    } else {
        StagingError::Io(error.to_string())
    }
}

struct RootCleanupGuard {
    path: PathBuf,
    armed: bool,
}

impl RootCleanupGuard {
    fn new(path: PathBuf) -> Self {
        Self { path, armed: true }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for RootCleanupGuard {
    fn drop(&mut self) {
        if self.armed {
            let _ = remove_private_tree(&self.path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    use std::fs;
    use tempfile::tempdir;

    fn digest(bytes: &[u8]) -> String {
        format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
    }

    fn input<'a>(relative_path: &str, bytes: &'a [u8]) -> PinnedInput<'a> {
        PinnedInput {
            workspace_id: "workspace-1".to_owned(),
            resource_id: "resource-1".to_owned(),
            revision_id: "revision-3".to_owned(),
            content_digest: digest(bytes),
            relative_path: relative_path.to_owned(),
            bytes,
        }
    }

    fn limits() -> StagingLimits {
        StagingLimits {
            max_files: 8,
            max_file_bytes: 128,
            max_total_input_bytes: 512,
            max_path_bytes: 240,
        }
    }

    fn task_view_fixture() -> storage_core::TaskView {
        storage_core::TaskView {
            task: storage_core::TaskRecord {
                task_id: "task-1".to_owned(),
                workspace_id: "workspace-1".to_owned(),
                conversation_id: None,
                current_spec_revision: 2,
                current_plan_revision: None,
                status: "READY".to_owned(),
                resume_status: None,
                routine_id: None,
                routine_revision: None,
                automation_id: None,
                automation_occurrence_id: None,
                origin_coworker_id: None,
                origin_coworker_revision: None,
                lead_agent_binding_id: "agent-1".to_owned(),
                blocking_conditions: Vec::new(),
                priority: "NORMAL".to_owned(),
                created_by: serde_json::Value::Null,
                created_at: "2026-10-09T00:00:00Z".to_owned(),
                updated_at: "2026-10-09T00:00:00Z".to_owned(),
                completed_at: None,
                version: 7,
            },
            current_spec_revision: storage_core::TaskSpecRevisionRecord {
                task_id: "task-1".to_owned(),
                workspace_id: "workspace-1".to_owned(),
                revision: 2,
                parent_revisions: vec![1],
                objective: "Prepare the pinned inputs".to_owned(),
                task_category: None,
                constraints: Vec::new(),
                non_goals: Vec::new(),
                input_refs: Vec::new(),
                workspace_instruction_revision: None,
                required_outputs: Vec::new(),
                acceptance_criteria: Vec::new(),
                approvals_required: Vec::new(),
                budget: None,
                delegation_budget_policy: None,
                lead_failover_policy: serde_json::Value::Null,
                deadline: None,
                source_message_refs: Vec::new(),
                placement_preference: serde_json::Value::Null,
                preferred_lead_agent_binding_id: None,
                authored_by: serde_json::Value::Null,
                created_at: "2026-10-09T00:00:00Z".to_owned(),
            },
        }
    }

    #[test]
    #[cfg(unix)]
    fn stages_pinned_files_into_read_only_inputs_and_separate_output_root() {
        let temp = tempdir().unwrap();
        let result = stage_inputs(
            temp.path(),
            "workspace-1",
            &[input("docs/README.md", b"hello pinned world")],
            limits(),
        )
        .unwrap();

        assert_eq!(result.status, PreparationStatus::PreparedOnly);
        assert_eq!(
            fs::read(result.input_root.join("docs/README.md")).unwrap(),
            b"hello pinned world"
        );
        assert!(result.output_root.is_dir());
        assert!(
            result
                .input_root
                .join("docs/README.md")
                .metadata()
                .unwrap()
                .permissions()
                .readonly()
        );
        assert_eq!(result.input_file_count, 1);
        assert_eq!(result.input_bytes, 18);
        let prepared_root = result.root.clone();
        result.cleanup().unwrap();
        assert!(!prepared_root.exists());
    }

    #[test]
    #[cfg(unix)]
    fn stages_multiple_files_under_the_same_nested_directory() {
        let temp = tempdir().unwrap();
        let first = input("docs/current/README.md", b"readme");
        let mut second = input("docs/current/CHANGELOG.md", b"changes");
        second.resource_id = "resource-2".to_owned();

        let prepared = stage_inputs(temp.path(), "workspace-1", &[first, second], limits())
            .expect("sibling resources should share their staged parent directory");

        assert_eq!(
            fs::read(prepared.input_root.join("docs/current/README.md")).unwrap(),
            b"readme"
        );
        assert_eq!(
            fs::read(prepared.input_root.join("docs/current/CHANGELOG.md")).unwrap(),
            b"changes"
        );
        prepared.cleanup().unwrap();
    }

    #[test]
    fn rejects_digest_mismatch_before_allocating_a_staging_root() {
        let temp = tempdir().unwrap();
        let first = input("docs/first.md", b"valid bytes");
        let mut second = input("docs/second.md", b"changed bytes");
        second.resource_id = "resource-2".to_owned();
        second.content_digest = digest(b"expected bytes");

        let error =
            stage_inputs(temp.path(), "workspace-1", &[first, second], limits()).unwrap_err();
        assert_eq!(error, StagingError::DigestMismatch);
        assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 0);
    }

    #[test]
    fn rejects_traversal_absolute_and_platform_ambiguous_paths() {
        let temp = tempdir().unwrap();
        for path in [
            "../escape",
            "/absolute",
            "C:\\escape",
            "CON.txt",
            "name.",
            "a//b",
        ] {
            let err = stage_inputs(temp.path(), "workspace-1", &[input(path, b"x")], limits())
                .unwrap_err();
            assert_eq!(
                err,
                StagingError::InvalidRelativePath(path.to_owned()),
                "{path}"
            );
        }
        assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 0);
    }

    #[test]
    fn rejects_duplicate_paths_even_when_case_differs() {
        let temp = tempdir().unwrap();
        let mut second = input("docs/README.md", b"b");
        second.resource_id = "resource-2".to_owned();
        let error = stage_inputs(
            temp.path(),
            "workspace-1",
            &[input("Docs/readme.md", b"a"), second],
            limits(),
        )
        .unwrap_err();
        assert_eq!(error, StagingError::DuplicatePath);
    }

    #[test]
    fn rejects_unicode_casefold_path_aliases_for_cross_platform_staging() {
        let temp = tempdir().unwrap();
        let first = input("σ.txt", b"first");
        let mut second = input("ς.txt", b"second");
        second.resource_id = "resource-2".to_owned();

        assert_eq!(
            stage_inputs(temp.path(), "workspace-1", &[first, second], limits()).unwrap_err(),
            StagingError::DuplicatePath
        );
        assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 0);
    }

    #[test]
    fn rejects_file_path_that_is_parent_of_another_input() {
        let temp = tempdir().unwrap();
        let first = input("node", b"file");
        let mut second = input("node/child.txt", b"child");
        second.resource_id = "resource-2".to_owned();
        assert_eq!(
            stage_inputs(temp.path(), "workspace-1", &[first, second], limits()).unwrap_err(),
            StagingError::DuplicatePath
        );
    }

    #[test]
    fn rejects_inputs_from_another_workspace_before_allocating_root() {
        let temp = tempdir().unwrap();
        let mut pinned = input("README.md", b"body");
        pinned.workspace_id = "foreign-workspace".to_owned();
        assert_eq!(
            stage_inputs(temp.path(), "workspace-1", &[pinned], limits()).unwrap_err(),
            StagingError::InvalidPin
        );
        assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 0);
    }

    #[test]
    fn rejects_duplicate_exact_resource_revision_pin() {
        let temp = tempdir().unwrap();
        let first = input("first.txt", b"one");
        let second = input("second.txt", b"two");
        assert_eq!(
            stage_inputs(temp.path(), "workspace-1", &[first, second], limits()).unwrap_err(),
            StagingError::DuplicatePin
        );
        assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 0);
    }

    #[test]
    fn enforces_file_and_total_input_limits_before_staging() {
        let temp = tempdir().unwrap();
        let small = StagingLimits {
            max_file_bytes: 4,
            ..limits()
        };
        assert_eq!(
            stage_inputs(
                temp.path(),
                "workspace-1",
                &[input("large", b"12345")],
                small
            )
            .unwrap_err(),
            StagingError::InputLimitExceeded
        );
        assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 0);

        let total = StagingLimits {
            max_total_input_bytes: 1,
            ..limits()
        };
        let mut second = input("second", b"x");
        second.resource_id = "resource-2".to_owned();
        assert_eq!(
            stage_inputs(
                temp.path(),
                "workspace-1",
                &[input("first", b"x"), second],
                total,
            )
            .unwrap_err(),
            StagingError::InputLimitExceeded
        );
    }

    #[test]
    #[cfg(unix)]
    fn preparation_result_contains_no_isolation_attestation() {
        let temp = tempdir().unwrap();
        let prepared = stage_inputs(
            temp.path(),
            "workspace-1",
            &[input("input", b"x")],
            limits(),
        )
        .unwrap();
        assert_eq!(prepared.status, PreparationStatus::PreparedOnly);
        prepared.cleanup().unwrap();
    }

    #[test]
    #[cfg(unix)]
    fn resolves_exact_task_pins_and_uses_store_display_names_for_staging_paths() {
        use serde_json::json;
        use std::sync::Mutex;
        use storage_core::{ResourceSummary, StoreError, StoredResourceContent};

        struct Reader {
            requested: Mutex<Vec<(String, String, String, u64)>>,
            content: StoredResourceContent,
        }

        impl ExactResourceReader for Reader {
            fn read_exact(
                &self,
                workspace_id: &str,
                resource_id: &str,
                revision_id: &str,
                maximum_bytes: u64,
            ) -> Result<Option<StoredResourceContent>, StoreError> {
                self.requested.lock().unwrap().push((
                    workspace_id.to_owned(),
                    resource_id.to_owned(),
                    revision_id.to_owned(),
                    maximum_bytes,
                ));
                Ok(Some(self.content.clone()))
            }
        }

        let bytes = b"pinned historical bytes";
        let reader = Reader {
            requested: Mutex::new(Vec::new()),
            content: StoredResourceContent {
                summary: ResourceSummary {
                    resource_id: "resource-1".to_owned(),
                    workspace_id: "workspace-1".to_owned(),
                    resource_revision_id: "revision-old".to_owned(),
                    display_name: "Research/notes.md".to_owned(),
                    media_type: "text/markdown".to_owned(),
                    content_digest: digest(bytes),
                    size_bytes: bytes.len() as u64,
                    created_at: "2026-10-09T00:00:00Z".to_owned(),
                },
                content: bytes.to_vec(),
            },
        };
        let mut task = task_view_fixture();
        task.current_spec_revision.input_refs = vec![json!({
            "workspace_id": "workspace-1",
            "resource_id": "resource-1",
            "revision_id": "revision-old"
        })];
        let temp = tempdir().unwrap();

        let prepared = stage_task_spec_inputs(
            &reader,
            &task,
            "workspace-1",
            "task-1",
            7,
            2,
            temp.path(),
            limits(),
        )
        .unwrap();

        assert_eq!(
            reader.requested.lock().unwrap().as_slice(),
            &[(
                "workspace-1".to_owned(),
                "resource-1".to_owned(),
                "revision-old".to_owned(),
                limits().max_file_bytes,
            )]
        );
        assert_eq!(
            fs::read(prepared.input_root.join("Research/notes.md")).unwrap(),
            bytes
        );
        assert_eq!(prepared.status, PreparationStatus::PreparedOnly);
        prepared.cleanup().unwrap();
    }

    #[test]
    fn rejects_unknown_task_pin_fields_before_resource_reads() {
        use serde_json::json;
        use std::sync::atomic::{AtomicUsize, Ordering};
        use storage_core::{StoreError, StoredResourceContent};

        struct Reader(AtomicUsize);
        impl ExactResourceReader for Reader {
            fn read_exact(
                &self,
                _workspace_id: &str,
                _resource_id: &str,
                _revision_id: &str,
                _maximum_bytes: u64,
            ) -> Result<Option<StoredResourceContent>, StoreError> {
                self.0.fetch_add(1, Ordering::SeqCst);
                Ok(None)
            }
        }

        let reader = Reader(AtomicUsize::new(0));
        let mut task = task_view_fixture();
        task.current_spec_revision.input_refs = vec![json!({
            "workspace_id": "workspace-1",
            "resource_id": "resource-1",
            "revision_id": "revision-1",
            "path": "../../outside"
        })];

        assert_eq!(
            stage_task_spec_inputs(
                &reader,
                &task,
                "workspace-1",
                "task-1",
                7,
                2,
                Path::new("/tmp"),
                limits(),
            )
            .unwrap_err(),
            TaskInputPreparationError::InvalidTaskInput
        );
        assert_eq!(reader.0.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn rejects_stale_task_version_before_resolving_resources() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use storage_core::{StoreError, StoredResourceContent};

        struct Reader(AtomicUsize);
        impl ExactResourceReader for Reader {
            fn read_exact(
                &self,
                _workspace_id: &str,
                _resource_id: &str,
                _revision_id: &str,
                _maximum_bytes: u64,
            ) -> Result<Option<StoredResourceContent>, StoreError> {
                self.0.fetch_add(1, Ordering::SeqCst);
                Ok(None)
            }
        }

        let reader = Reader(AtomicUsize::new(0));
        let task = task_view_fixture();
        assert_eq!(
            stage_task_spec_inputs(
                &reader,
                &task,
                "workspace-1",
                "task-1",
                6,
                2,
                Path::new("/tmp"),
                limits(),
            )
            .unwrap_err(),
            TaskInputPreparationError::StaleTaskRevision
        );
        assert_eq!(reader.0.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn rejects_store_metadata_that_does_not_match_the_pin_before_staging() {
        use serde_json::json;
        use storage_core::{ResourceSummary, StoreError, StoredResourceContent};

        struct Reader(StoredResourceContent);
        impl ExactResourceReader for Reader {
            fn read_exact(
                &self,
                _workspace_id: &str,
                _resource_id: &str,
                _revision_id: &str,
                _maximum_bytes: u64,
            ) -> Result<Option<StoredResourceContent>, StoreError> {
                Ok(Some(self.0.clone()))
            }
        }

        let bytes = b"some other revision";
        let reader = Reader(StoredResourceContent {
            summary: ResourceSummary {
                resource_id: "resource-1".to_owned(),
                workspace_id: "workspace-1".to_owned(),
                resource_revision_id: "revision-new".to_owned(),
                display_name: "notes.md".to_owned(),
                media_type: "text/markdown".to_owned(),
                content_digest: digest(bytes),
                size_bytes: bytes.len() as u64,
                created_at: "2026-10-09T00:00:00Z".to_owned(),
            },
            content: bytes.to_vec(),
        });
        let mut task = task_view_fixture();
        task.current_spec_revision.input_refs = vec![json!({
            "workspace_id": "workspace-1",
            "resource_id": "resource-1",
            "revision_id": "revision-old"
        })];
        let temp = tempdir().unwrap();

        assert_eq!(
            stage_task_spec_inputs(
                &reader,
                &task,
                "workspace-1",
                "task-1",
                7,
                2,
                temp.path(),
                limits(),
            )
            .unwrap_err(),
            TaskInputPreparationError::ResourceMetadataMismatch
        );
        assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 0);
    }

    #[test]
    fn cleanup_permission_helper_clears_readonly_file_attributes() {
        let temp = tempdir().unwrap();
        let file_path = temp.path().join("readonly.txt");
        fs::write(&file_path, b"test").unwrap();
        let mut permissions = fs::metadata(&file_path).unwrap().permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&file_path, permissions).unwrap();

        assert!(fs::metadata(&file_path).unwrap().permissions().readonly());
        clear_readonly_files(temp.path()).unwrap();
        assert!(!fs::metadata(&file_path).unwrap().permissions().readonly());
    }

    #[cfg(not(unix))]
    #[test]
    fn non_unix_staging_fails_closed_until_private_acl_is_qualified() {
        let temp = tempdir().unwrap();
        assert_eq!(
            stage_inputs(
                temp.path(),
                "workspace-1",
                &[input("README.md", b"body")],
                limits(),
            )
            .unwrap_err(),
            StagingError::UnsupportedPlatform
        );
        assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 0);
    }
}
