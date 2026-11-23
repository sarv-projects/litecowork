//! Runtime-local isolation evidence for Task-scoped Environments.
//!
//! `docs/ENVIRONMENTS.md` requires that, before an AgentSession or Attempt can start,
//! the selected provider returns a Runtime-local, non-secret `IsolationAttestation`
//! bound to the Environment, Task, current Runtime incarnation, provider identity, OS
//! build, and normalized isolation policy digest. The Runtime then checks that every
//! required enforcement field is `ENFORCED`, that the attestation is current, and that
//! its policy/input digests match the admission request.
//!
//! Three rules shape this module:
//!
//! 1. The attestation is *operational evidence*, not a grant. It never authorizes a
//!    capability, never satisfies an ExecutionLease, and never appears in a domain
//!    event, aggregate snapshot, backup, Operator projection, Agent prompt, or Task
//!    packet. It lives only in the private incarnation-scoped store below.
//! 2. `NOT_ENFORCED` is an observed rejection and `UNKNOWN` preserves uncertainty.
//!    Neither is ever coerced into success.
//! 3. A provider capability *probe* is not enforcement evidence. The attestation is
//!    only produced from an observed launch; a probe alone yields `UNKNOWN` and the
//!    admission check therefore fails closed.

use linux_process_scope::LocalProviderProbe;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

/// Maximum lifetime of an isolation attestation.
///
/// An attestation that outlives its observation window would silently certify a
/// provider state the Runtime has not re-checked.
pub const MAX_ATTESTATION_TTL_MS: i64 = 15 * 60 * 1000;

use domain_environment::environment::EnforcementStatus;

/// The digests a prepared Environment root contributes to an attestation.
///
/// A provider adapter fills this from the materialized staging result. The Runtime
/// only ever sees digests, never a host filesystem path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedRootDigests {
    pub pinned_input_set_digest: String,
    pub readable_root_set_digest: String,
    pub writable_root_set_digest: String,
}

/// Identity a provider observation is bound to.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IsolationIdentity {
    pub environment_id: String,
    pub workspace_id: String,
    pub task_id: String,
    pub runtime_id: String,
    pub runtime_incarnation_id: String,
    /// Digest of the normalized isolation policy that was requested.
    pub normalized_policy_digest: String,
}

/// Provider facts observed while an Environment was actually prepared.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderIsolationFacts {
    pub provider_kind: String,
    pub provider_version: String,
    pub operating_system: String,
    pub operating_system_build: String,
    pub filesystem_enforcement: EnforcementStatus,
    pub process_tree_enforcement: EnforcementStatus,
    pub network_enforcement: EnforcementStatus,
    pub resource_limit_enforcement: EnforcementStatus,
}

/// Runtime-local isolation evidence.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IsolationAttestation {
    pub environment_id: String,
    pub workspace_id: String,
    pub task_id: String,
    pub runtime_id: String,
    pub runtime_incarnation_id: String,
    pub provider_kind: String,
    pub provider_version: String,
    pub operating_system: String,
    pub operating_system_build: String,
    pub normalized_policy_digest: String,
    pub pinned_input_set_digest: String,
    pub readable_root_set_digest: String,
    pub writable_root_set_digest: String,
    pub filesystem_enforcement: EnforcementStatus,
    pub process_tree_enforcement: EnforcementStatus,
    pub network_enforcement: EnforcementStatus,
    pub resource_limit_enforcement: EnforcementStatus,
    /// Unix epoch milliseconds when the provider observation was taken.
    pub observed_at_ms: i64,
    /// Unix epoch milliseconds after which the attestation is stale.
    pub expires_at_ms: i64,
}

/// Result of a provider quiescence observation.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum QuiescenceResult {
    /// The provider positively observed that no writer remains.
    Quiescent,
    /// The provider could not establish that no writer remains.
    Unknown,
}

/// Runtime-local observation that a provider execution identity has stopped writing.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct QuiescenceObservation {
    pub environment_id: String,
    pub runtime_id: String,
    pub runtime_incarnation_id: String,
    pub execution_identity_digest: String,
    pub process_scope_digest: String,
    pub result: QuiescenceResult,
    pub observed_at_ms: i64,
}

/// The admission request an attestation is checked against.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IsolationAdmissionRequest {
    pub environment_id: String,
    pub workspace_id: String,
    pub task_id: String,
    pub runtime_id: String,
    pub runtime_incarnation_id: String,
    pub normalized_policy_digest: String,
    pub pinned_input_set_digest: String,
    pub readable_root_set_digest: String,
    pub writable_root_set_digest: String,
    pub now_ms: i64,
}

/// Why an isolation admission was refused.
///
/// Every variant is a fail-closed outcome. None of them may be reported to an Agent as
/// an internal store string.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IsolationAdmissionError {
    AttestationMissing,
    EnvironmentMismatch,
    WorkspaceMismatch,
    TaskMismatch,
    RuntimeMismatch,
    RuntimeIncarnationMismatch,
    PolicyDigestMismatch,
    PinnedInputSetMismatch,
    ReadableRootSetMismatch,
    WritableRootSetMismatch,
    FilesystemEnforcementMissing,
    ProcessTreeEnforcementMissing,
    NetworkEnforcementMissing,
    ResourceLimitEnforcementMissing,
    AttestationExpired,
    ObservationFromTheFuture,
}

impl IsolationAdmissionError {
    /// Stable, non-sensitive reason code suitable for a blocker projection.
    pub fn reason_code(&self) -> &'static str {
        match self {
            Self::AttestationMissing => "ISOLATION_ATTESTATION_MISSING",
            Self::EnvironmentMismatch => "ISOLATION_ENVIRONMENT_MISMATCH",
            Self::WorkspaceMismatch => "ISOLATION_WORKSPACE_MISMATCH",
            Self::TaskMismatch => "ISOLATION_TASK_MISMATCH",
            Self::RuntimeMismatch => "ISOLATION_RUNTIME_MISMATCH",
            Self::RuntimeIncarnationMismatch => "ISOLATION_RUNTIME_INCARNATION_MISMATCH",
            Self::PolicyDigestMismatch => "ISOLATION_POLICY_DIGEST_MISMATCH",
            Self::PinnedInputSetMismatch => "ISOLATION_PINNED_INPUT_SET_MISMATCH",
            Self::ReadableRootSetMismatch => "ISOLATION_READABLE_ROOT_SET_MISMATCH",
            Self::WritableRootSetMismatch => "ISOLATION_WRITABLE_ROOT_SET_MISMATCH",
            Self::FilesystemEnforcementMissing => "ISOLATION_FILESYSTEM_NOT_ENFORCED",
            Self::ProcessTreeEnforcementMissing => "ISOLATION_PROCESS_TREE_NOT_ENFORCED",
            Self::NetworkEnforcementMissing => "ISOLATION_NETWORK_NOT_ENFORCED",
            Self::ResourceLimitEnforcementMissing => "ISOLATION_RESOURCE_LIMITS_NOT_ENFORCED",
            Self::AttestationExpired => "ISOLATION_ATTESTATION_EXPIRED",
            Self::ObservationFromTheFuture => "ISOLATION_OBSERVATION_FROM_THE_FUTURE",
        }
    }
}

/// Why a quiescence observation was refused.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum QuiescenceAdmissionError {
    ObservationMissing,
    EnvironmentMismatch,
    RuntimeMismatch,
    RuntimeIncarnationMismatch,
    ExecutionIdentityMismatch,
    ProcessScopeMismatch,
    ResultNotQuiescent,
    ObservationFromTheFuture,
}

impl QuiescenceAdmissionError {
    pub fn reason_code(&self) -> &'static str {
        match self {
            Self::ObservationMissing => "QUIESCENCE_OBSERVATION_MISSING",
            Self::EnvironmentMismatch => "QUIESCENCE_ENVIRONMENT_MISMATCH",
            Self::RuntimeMismatch => "QUIESCENCE_RUNTIME_MISMATCH",
            Self::RuntimeIncarnationMismatch => "QUIESCENCE_RUNTIME_INCARNATION_MISMATCH",
            Self::ExecutionIdentityMismatch => "QUIESCENCE_EXECUTION_IDENTITY_MISMATCH",
            Self::ProcessScopeMismatch => "QUIESCENCE_PROCESS_SCOPE_MISMATCH",
            Self::ResultNotQuiescent => "QUIESCENCE_NOT_CONFIRMED",
            Self::ObservationFromTheFuture => "QUIESCENCE_OBSERVATION_FROM_THE_FUTURE",
        }
    }
}

/// Errors raised while producing or storing isolation evidence.
#[derive(Debug)]
pub enum IsolationStoreError {
    InvalidIdentifier(String),
    Io(std::io::Error),
    InvalidRecord(String),
    AlreadyStored,
    StaleAttestation,
    IncarnationMismatch,
    JournalConflict,
    JournalTransition(&'static str),
}

impl From<std::io::Error> for IsolationStoreError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl std::fmt::Display for IsolationStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidIdentifier(detail) => {
                write!(f, "invalid isolation identifier: {detail}")
            }
            Self::Io(error) => write!(f, "isolation store I/O failed: {error}"),
            Self::InvalidRecord(detail) => write!(f, "invalid isolation record: {detail}"),
            Self::AlreadyStored => {
                write!(
                    f,
                    "an isolation attestation is already stored for this environment"
                )
            }
            Self::StaleAttestation => {
                write!(
                    f,
                    "the stored isolation attestation is newer than the candidate"
                )
            }
            Self::IncarnationMismatch => write!(
                f,
                "the isolation attestation belongs to a different runtime incarnation"
            ),
            Self::JournalConflict => {
                f.write_str("preparation journal identity conflicts with the existing record")
            }
            Self::JournalTransition(detail) => {
                write!(f, "invalid preparation journal transition: {detail}")
            }
        }
    }
}

impl std::error::Error for IsolationStoreError {}

/// Build the attestation for a prepared, observed Environment.
///
/// The digests come from the prepared staging result, so an attestation cannot claim a
/// root set or input set that was never materialized. `facts` must describe what the
/// provider actually observed; a caller that only probed the provider must pass
/// `EnforcementStatus::Unknown`, which fails admission.
pub fn build_attestation(
    identity: &IsolationIdentity,
    prepared: &PreparedRootDigests,
    facts: ProviderIsolationFacts,
    observed_at_ms: i64,
    ttl_ms: i64,
) -> Result<IsolationAttestation, IsolationStoreError> {
    if ttl_ms <= 0 || ttl_ms > MAX_ATTESTATION_TTL_MS {
        return Err(IsolationStoreError::InvalidRecord(
            "isolation attestation ttl is out of range".into(),
        ));
    }
    Ok(IsolationAttestation {
        environment_id: identity.environment_id.clone(),
        workspace_id: identity.workspace_id.clone(),
        task_id: identity.task_id.clone(),
        runtime_id: identity.runtime_id.clone(),
        runtime_incarnation_id: identity.runtime_incarnation_id.clone(),
        provider_kind: facts.provider_kind,
        provider_version: facts.provider_version,
        operating_system: facts.operating_system,
        operating_system_build: facts.operating_system_build,
        normalized_policy_digest: identity.normalized_policy_digest.clone(),
        pinned_input_set_digest: prepared.pinned_input_set_digest.clone(),
        readable_root_set_digest: prepared.readable_root_set_digest.clone(),
        writable_root_set_digest: prepared.writable_root_set_digest.clone(),
        filesystem_enforcement: facts.filesystem_enforcement,
        process_tree_enforcement: facts.process_tree_enforcement,
        network_enforcement: facts.network_enforcement,
        resource_limit_enforcement: facts.resource_limit_enforcement,
        observed_at_ms,
        expires_at_ms: observed_at_ms.saturating_add(ttl_ms),
    })
}

/// Check an attestation against the current admission request.
///
/// The check fails closed. Every required enforcement dimension must be positively
/// `ENFORCED`; `NOT_ENFORCED` and `UNKNOWN` are both refused.
pub fn evaluate_isolation_admission(
    attestation: Option<&IsolationAttestation>,
    request: &IsolationAdmissionRequest,
) -> Result<(), IsolationAdmissionError> {
    let attestation = attestation.ok_or(IsolationAdmissionError::AttestationMissing)?;
    if attestation.environment_id != request.environment_id {
        return Err(IsolationAdmissionError::EnvironmentMismatch);
    }
    if attestation.workspace_id != request.workspace_id {
        return Err(IsolationAdmissionError::WorkspaceMismatch);
    }
    if attestation.task_id != request.task_id {
        return Err(IsolationAdmissionError::TaskMismatch);
    }
    if attestation.runtime_id != request.runtime_id {
        return Err(IsolationAdmissionError::RuntimeMismatch);
    }
    if attestation.runtime_incarnation_id != request.runtime_incarnation_id {
        return Err(IsolationAdmissionError::RuntimeIncarnationMismatch);
    }
    if attestation.normalized_policy_digest != request.normalized_policy_digest {
        return Err(IsolationAdmissionError::PolicyDigestMismatch);
    }
    if attestation.pinned_input_set_digest != request.pinned_input_set_digest {
        return Err(IsolationAdmissionError::PinnedInputSetMismatch);
    }
    if attestation.readable_root_set_digest != request.readable_root_set_digest {
        return Err(IsolationAdmissionError::ReadableRootSetMismatch);
    }
    if attestation.writable_root_set_digest != request.writable_root_set_digest {
        return Err(IsolationAdmissionError::WritableRootSetMismatch);
    }
    if !attestation.filesystem_enforcement.is_enforced() {
        return Err(IsolationAdmissionError::FilesystemEnforcementMissing);
    }
    if !attestation.process_tree_enforcement.is_enforced() {
        return Err(IsolationAdmissionError::ProcessTreeEnforcementMissing);
    }
    if !attestation.network_enforcement.is_enforced() {
        return Err(IsolationAdmissionError::NetworkEnforcementMissing);
    }
    if !attestation.resource_limit_enforcement.is_enforced() {
        return Err(IsolationAdmissionError::ResourceLimitEnforcementMissing);
    }
    if request.now_ms < attestation.observed_at_ms {
        return Err(IsolationAdmissionError::ObservationFromTheFuture);
    }
    if request.now_ms >= attestation.expires_at_ms {
        return Err(IsolationAdmissionError::AttestationExpired);
    }
    Ok(())
}

/// Check a quiescence observation against the writer that is about to be released.
///
/// Only a positive `QUIESCENT` observation for the current Environment, Runtime
/// incarnation, and exact private execution identity releases a writer fence.
pub fn evaluate_quiescence(
    observation: Option<&QuiescenceObservation>,
    environment_id: &str,
    runtime_id: &str,
    runtime_incarnation_id: &str,
    execution_identity_digest: &str,
    process_scope_digest: &str,
    now_ms: i64,
) -> Result<(), QuiescenceAdmissionError> {
    let observation = observation.ok_or(QuiescenceAdmissionError::ObservationMissing)?;
    if observation.environment_id != environment_id {
        return Err(QuiescenceAdmissionError::EnvironmentMismatch);
    }
    if observation.runtime_id != runtime_id {
        return Err(QuiescenceAdmissionError::RuntimeMismatch);
    }
    if observation.runtime_incarnation_id != runtime_incarnation_id {
        return Err(QuiescenceAdmissionError::RuntimeIncarnationMismatch);
    }
    if observation.execution_identity_digest != execution_identity_digest {
        return Err(QuiescenceAdmissionError::ExecutionIdentityMismatch);
    }
    if observation.process_scope_digest != process_scope_digest {
        return Err(QuiescenceAdmissionError::ProcessScopeMismatch);
    }
    if observation.result != QuiescenceResult::Quiescent {
        return Err(QuiescenceAdmissionError::ResultNotQuiescent);
    }
    if now_ms < observation.observed_at_ms {
        return Err(QuiescenceAdmissionError::ObservationFromTheFuture);
    }
    Ok(())
}

/// Private, incarnation-scoped store for Runtime-local isolation evidence.
///
/// The store is deliberately *not* a domain store. Nothing here is an event source,
/// an aggregate snapshot, a backup, or an Operator projection, and the directory is
/// created with owner-only permissions. A Runtime restart creates a new incarnation
/// directory, so an attestation from a previous incarnation is never visible to the
/// new one.
#[derive(Clone, Debug)]
pub struct RuntimeLocalIsolationStore {
    directory: PathBuf,
    runtime_id: String,
    runtime_incarnation_id: String,
}

const PREPARATION_JOURNAL_VERSION: u8 = 1;
const PREPARATION_JOURNAL_LOCK_FILE: &str = ".journal.lock";
const MAX_PREPARATION_JOURNAL_ENTRY_BYTES: usize = 16 * 1024;
#[cfg(windows)]
const WINDOWS_FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;

/// Exact non-secret identity committed to the Runtime-private write-ahead journal before
/// a provider may create any external scope or filesystem state. The idempotency key is a
/// server-generated request identity, not an authorization credential.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PreparationJournalIntent {
    pub attempt_id: String,
    pub environment_id: String,
    pub workspace_id: String,
    pub task_id: String,
    pub step_id: String,
    pub runtime_id: String,
    pub runtime_incarnation_id: String,
    /// Deterministic provider scope token, known before the external scope is created.
    pub provider_scope_id: String,
    pub admission_idempotency_key: String,
    pub request_digest: String,
    pub normalized_policy_digest: String,
    pub pinned_input_set_digest: String,
    pub readable_root_set_digest: String,
    pub writable_root_set_digest: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HeldProviderScopeIdentity {
    pub unit_name: String,
    pub leader_pid: u32,
    pub leader_start_time_ticks: u64,
    pub process_scope_digest: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PreparationAdmissionReceipt {
    pub receipt_id: String,
    pub idempotency_key: String,
    pub request_digest: String,
    pub workspace_id: String,
    pub task_id: String,
    pub attempt_id: String,
    pub environment_id: String,
    pub runtime_id: String,
    pub runtime_incarnation_id: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PreparationJournalPhase {
    IntentWritten,
    ScopeHeld,
    AdmissionCommitted,
    LaunchHandoffAttempted,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PreparationJournalEntry {
    pub schema_version: u8,
    pub intent: PreparationJournalIntent,
    pub phase: PreparationJournalPhase,
    pub held_scope: Option<HeldProviderScopeIdentity>,
    pub admission_receipt: Option<PreparationAdmissionReceipt>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PreparationReceiptLookup {
    /// The storage owner could not establish whether the admission transaction committed.
    Unknown,
    /// The storage owner read by the exact journaled idempotency key and found no receipt.
    DefinitelyAbsent,
    /// The storage owner read by the exact key and returned this committed receipt.
    ExactCommit(PreparationAdmissionReceipt),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PreparationRecoveryAction {
    BlockUnknown,
    InspectExactProviderScope {
        provider_scope_id: String,
        held_scope: Option<HeldProviderScopeIdentity>,
    },
    RecoverCommittedAttempt {
        receipt: PreparationAdmissionReceipt,
        provider_scope_id: String,
        held_scope: HeldProviderScopeIdentity,
        launch_may_have_been_sent: bool,
    },
}

/// Durable Runtime-private recovery journal. Its directory is stable across Runtime
/// incarnations so startup can reconcile the previous incarnation's exact provider scope
/// and admission receipt. Entries contain no host paths, credentials or public authority.
#[derive(Clone, Debug)]
pub struct RuntimeLocalPreparationJournal {
    directory: PathBuf,
    runtime_id: String,
}

struct PreparationJournalFileLock {
    file: File,
}

impl Drop for PreparationJournalFileLock {
    fn drop(&mut self) {
        let _ = fs2::FileExt::unlock(&self.file);
    }
}

impl RuntimeLocalIsolationStore {
    /// Open (or create) the private store for one Runtime incarnation.
    pub fn open(
        root: &Path,
        runtime_id: &str,
        runtime_incarnation_id: &str,
    ) -> Result<Self, IsolationStoreError> {
        validate_identifier(runtime_id)?;
        validate_identifier(runtime_incarnation_id)?;
        let directory = root
            .join("isolation")
            .join(runtime_id)
            .join(runtime_incarnation_id);
        fs::create_dir_all(&directory)?;
        let metadata = fs::symlink_metadata(&directory)?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(IsolationStoreError::InvalidRecord(
                "isolation store directory is not a real directory".into(),
            ));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::{MetadataExt, PermissionsExt};
            if metadata.uid() != rustix::process::geteuid().as_raw() {
                return Err(IsolationStoreError::InvalidRecord(
                    "isolation store directory owner mismatch".into(),
                ));
            }
            fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))?;
        }
        #[cfg(windows)]
        windows_local_security::secure_current_user_directory(&directory)?;
        Ok(Self {
            directory,
            runtime_id: runtime_id.to_owned(),
            runtime_incarnation_id: runtime_incarnation_id.to_owned(),
        })
    }

    pub fn runtime_id(&self) -> &str {
        &self.runtime_id
    }

    pub fn runtime_incarnation_id(&self) -> &str {
        &self.runtime_incarnation_id
    }

    fn record_path(&self, environment_id: &str) -> Result<PathBuf, IsolationStoreError> {
        validate_identifier(environment_id)?;
        Ok(self
            .directory
            .join(format!("{}.json", sha256_hex(environment_id.as_bytes()))))
    }

    /// Persist an attestation for one Environment.
    ///
    /// The write is a compare-and-swap: an existing attestation may only be replaced
    /// by a strictly newer observation, and only for the same Environment.
    pub fn store(&self, attestation: &IsolationAttestation) -> Result<(), IsolationStoreError> {
        if attestation.runtime_id != self.runtime_id {
            return Err(IsolationStoreError::IncarnationMismatch);
        }
        if attestation.runtime_incarnation_id != self.runtime_incarnation_id {
            return Err(IsolationStoreError::IncarnationMismatch);
        }
        let path = self.record_path(&attestation.environment_id)?;
        if let Some(existing) = self.load(&attestation.environment_id)? {
            if existing.observed_at_ms > attestation.observed_at_ms {
                return Err(IsolationStoreError::StaleAttestation);
            }
            if existing.observed_at_ms == attestation.observed_at_ms && existing != *attestation {
                return Err(IsolationStoreError::AlreadyStored);
            }
        }
        let mut bytes = serde_json::to_vec(attestation).map_err(|_| {
            IsolationStoreError::InvalidRecord("attestation serialization failed".into())
        })?;
        bytes.push(b'\n');
        let mut file = tempfile::NamedTempFile::new_in(&self.directory)?;
        file.write_all(&bytes)?;
        file.as_file().sync_all()?;
        file.persist(&path)
            .map_err(|error| IsolationStoreError::Io(error.error))?;
        #[cfg(unix)]
        File::open(&self.directory)?.sync_all()?;
        Ok(())
    }

    /// Load the attestation stored for one Environment.
    pub fn load(
        &self,
        environment_id: &str,
    ) -> Result<Option<IsolationAttestation>, IsolationStoreError> {
        let path = self.record_path(environment_id)?;
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(IsolationStoreError::Io(error)),
        };
        let attestation: IsolationAttestation = serde_json::from_slice(&bytes).map_err(|_| {
            IsolationStoreError::InvalidRecord("attestation record is unreadable".into())
        })?;
        if attestation.environment_id != environment_id {
            return Err(IsolationStoreError::InvalidRecord(
                "attestation record does not match its environment".into(),
            ));
        }
        if attestation.runtime_incarnation_id != self.runtime_incarnation_id {
            return Err(IsolationStoreError::IncarnationMismatch);
        }
        Ok(Some(attestation))
    }

    /// Drop the stored attestation for one Environment.
    ///
    /// Callers use this after a positive quiescence observation, when the Environment
    /// is destroyed, or when the provider binding is revoked.
    pub fn remove(&self, environment_id: &str) -> Result<bool, IsolationStoreError> {
        let path = self.record_path(environment_id)?;
        match fs::remove_file(&path) {
            Ok(()) => {
                #[cfg(unix)]
                File::open(&self.directory)?.sync_all()?;
                Ok(true)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(IsolationStoreError::Io(error)),
        }
    }
}

impl RuntimeLocalPreparationJournal {
    pub fn open(root: &Path, runtime_id: &str) -> Result<Self, IsolationStoreError> {
        validate_identifier(runtime_id)?;
        let directory = root.join("isolation").join(runtime_id).join("preparations");
        fs::create_dir_all(&directory)?;
        let metadata = fs::symlink_metadata(&directory)?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(IsolationStoreError::InvalidRecord(
                "preparation journal directory is not a real directory".into(),
            ));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::{MetadataExt, PermissionsExt};
            if metadata.uid() != rustix::process::geteuid().as_raw() {
                return Err(IsolationStoreError::InvalidRecord(
                    "preparation journal directory owner mismatch".into(),
                ));
            }
            fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))?;
        }
        #[cfg(windows)]
        windows_local_security::secure_current_user_directory(&directory)?;
        Ok(Self {
            directory,
            runtime_id: runtime_id.to_owned(),
        })
    }

    fn record_path(&self, attempt_id: &str) -> Result<PathBuf, IsolationStoreError> {
        validate_identifier(attempt_id)?;
        Ok(self
            .directory
            .join(format!("{}.json", sha256_hex(attempt_id.as_bytes()))))
    }

    /// Persist the intent before creating a provider scope. Repeating the exact same
    /// begin request is idempotent; reusing an Attempt ID for different input conflicts.
    pub fn begin(
        &self,
        intent: PreparationJournalIntent,
    ) -> Result<PreparationJournalEntry, IsolationStoreError> {
        let _lock = self.acquire_exclusive_lock()?;
        validate_preparation_intent(&intent)?;
        if intent.runtime_id != self.runtime_id {
            return Err(IsolationStoreError::IncarnationMismatch);
        }
        let entry = PreparationJournalEntry {
            schema_version: PREPARATION_JOURNAL_VERSION,
            intent,
            phase: PreparationJournalPhase::IntentWritten,
            held_scope: None,
            admission_receipt: None,
        };
        let path = self.record_path(&entry.intent.attempt_id)?;
        if let Some(existing) = self.load_unlocked(&entry.intent.attempt_id)? {
            return if existing == entry {
                Ok(existing)
            } else {
                Err(IsolationStoreError::JournalConflict)
            };
        }
        let bytes = serialize_preparation_entry(&entry)?;
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&path)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        sync_private_directory(&self.directory)?;
        Ok(entry)
    }

    pub fn load(
        &self,
        attempt_id: &str,
    ) -> Result<Option<PreparationJournalEntry>, IsolationStoreError> {
        let _lock = self.acquire_exclusive_lock()?;
        self.load_unlocked(attempt_id)
    }

    fn load_unlocked(
        &self,
        attempt_id: &str,
    ) -> Result<Option<PreparationJournalEntry>, IsolationStoreError> {
        let path = self.record_path(attempt_id)?;
        match fs::symlink_metadata(&path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(IsolationStoreError::Io(error)),
            Ok(_) => {}
        }
        let entry = read_preparation_journal_entry(&path)?;
        if entry.intent.attempt_id != attempt_id || entry.intent.runtime_id != self.runtime_id {
            return Err(IsolationStoreError::InvalidRecord(
                "preparation journal identity does not match its key".into(),
            ));
        }
        let expected = self.record_path(&entry.intent.attempt_id)?;
        if path.file_name() != expected.file_name() {
            return Err(IsolationStoreError::InvalidRecord(
                "preparation journal filename does not match its key".into(),
            ));
        }
        Ok(Some(entry))
    }

    /// Enumerate durable intents left by any Runtime incarnation. Corrupt entries fail
    /// the whole scan so startup cannot silently overlook an unknown external scope.
    pub fn list_pending(&self) -> Result<Vec<PreparationJournalEntry>, IsolationStoreError> {
        let _lock = self.acquire_exclusive_lock()?;
        let mut entries = Vec::new();
        for item in fs::read_dir(&self.directory)? {
            let item = item?;
            let path = item.path();
            if path.extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            let entry = read_preparation_journal_entry(&path)?;
            let expected = self.record_path(&entry.intent.attempt_id)?;
            if path.file_name() != expected.file_name() {
                return Err(IsolationStoreError::InvalidRecord(
                    "preparation journal filename does not match its key".into(),
                ));
            }
            if entry.intent.runtime_id != self.runtime_id {
                return Err(IsolationStoreError::IncarnationMismatch);
            }
            entries.push(entry);
        }
        entries.sort_by(|left, right| left.intent.attempt_id.cmp(&right.intent.attempt_id));
        Ok(entries)
    }

    pub fn record_scope_held(
        &self,
        attempt_id: &str,
        held_scope: HeldProviderScopeIdentity,
    ) -> Result<PreparationJournalEntry, IsolationStoreError> {
        let _lock = self.acquire_exclusive_lock()?;
        validate_held_scope(&held_scope)?;
        let mut entry = self.required_entry_unlocked(attempt_id)?;
        let expected_unit = scope_unit_name(&entry.intent.provider_scope_id);
        if held_scope.unit_name != expected_unit {
            return Err(IsolationStoreError::JournalConflict);
        }
        match entry.phase {
            PreparationJournalPhase::IntentWritten => {
                entry.phase = PreparationJournalPhase::ScopeHeld;
                entry.held_scope = Some(held_scope);
                self.persist_entry(&entry)?;
                Ok(entry)
            }
            PreparationJournalPhase::ScopeHeld
                if entry.held_scope.as_ref() == Some(&held_scope) =>
            {
                Ok(entry)
            }
            _ => Err(IsolationStoreError::JournalTransition(
                "provider scope may be recorded only once".into(),
            )),
        }
    }

    pub fn record_admission_receipt(
        &self,
        attempt_id: &str,
        receipt: PreparationAdmissionReceipt,
    ) -> Result<PreparationJournalEntry, IsolationStoreError> {
        let _lock = self.acquire_exclusive_lock()?;
        let mut entry = self.required_entry_unlocked(attempt_id)?;
        validate_receipt_matches(&entry.intent, &receipt)?;
        match entry.phase {
            PreparationJournalPhase::ScopeHeld => {
                entry.phase = PreparationJournalPhase::AdmissionCommitted;
                entry.admission_receipt = Some(receipt);
                self.persist_entry(&entry)?;
                Ok(entry)
            }
            PreparationJournalPhase::AdmissionCommitted
            | PreparationJournalPhase::LaunchHandoffAttempted
                if entry.admission_receipt.as_ref() == Some(&receipt) =>
            {
                Ok(entry)
            }
            _ => Err(IsolationStoreError::JournalTransition(
                "admission receipt requires the exact held scope".into(),
            )),
        }
    }

    pub fn mark_launch_handoff_attempted(
        &self,
        attempt_id: &str,
    ) -> Result<PreparationJournalEntry, IsolationStoreError> {
        let _lock = self.acquire_exclusive_lock()?;
        let mut entry = self.required_entry_unlocked(attempt_id)?;
        match entry.phase {
            PreparationJournalPhase::AdmissionCommitted => {
                entry.phase = PreparationJournalPhase::LaunchHandoffAttempted;
                self.persist_entry(&entry)?;
                Ok(entry)
            }
            _ => Err(IsolationStoreError::JournalTransition(
                "launch handoff requires a committed admission receipt and is one-shot".into(),
            )),
        }
    }

    pub fn recovery_action(
        &self,
        attempt_id: &str,
        receipt: PreparationReceiptLookup,
    ) -> Result<PreparationRecoveryAction, IsolationStoreError> {
        let _lock = self.acquire_exclusive_lock()?;
        let entry = self.required_entry_unlocked(attempt_id)?;
        match receipt {
            PreparationReceiptLookup::Unknown => Ok(PreparationRecoveryAction::BlockUnknown),
            PreparationReceiptLookup::DefinitelyAbsent => {
                if entry.admission_receipt.is_some() {
                    return Err(IsolationStoreError::JournalConflict);
                }
                Ok(PreparationRecoveryAction::InspectExactProviderScope {
                    provider_scope_id: entry.intent.provider_scope_id,
                    held_scope: entry.held_scope,
                })
            }
            PreparationReceiptLookup::ExactCommit(receipt) => {
                validate_receipt_matches(&entry.intent, &receipt)?;
                let held_scope = entry
                    .held_scope
                    .clone()
                    .ok_or(IsolationStoreError::JournalConflict)?;
                if let Some(existing) = &entry.admission_receipt {
                    if existing != &receipt {
                        return Err(IsolationStoreError::JournalConflict);
                    }
                } else {
                    if entry.phase != PreparationJournalPhase::ScopeHeld {
                        return Err(IsolationStoreError::JournalConflict);
                    }
                    let mut committed = entry.clone();
                    committed.admission_receipt = Some(receipt.clone());
                    committed.phase = PreparationJournalPhase::AdmissionCommitted;
                    self.persist_entry(&committed)?;
                }
                Ok(PreparationRecoveryAction::RecoverCommittedAttempt {
                    receipt,
                    provider_scope_id: entry.intent.provider_scope_id,
                    held_scope,
                    launch_may_have_been_sent: entry.phase
                        == PreparationJournalPhase::LaunchHandoffAttempted,
                })
            }
        }
    }

    fn required_entry_unlocked(
        &self,
        attempt_id: &str,
    ) -> Result<PreparationJournalEntry, IsolationStoreError> {
        self.load_unlocked(attempt_id)?.ok_or_else(|| {
            IsolationStoreError::InvalidRecord("preparation journal entry is missing".into())
        })
    }

    fn persist_entry(&self, entry: &PreparationJournalEntry) -> Result<(), IsolationStoreError> {
        validate_preparation_entry(entry)?;
        let path = self.record_path(&entry.intent.attempt_id)?;
        let mut temporary = tempfile::NamedTempFile::new_in(&self.directory)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            temporary
                .as_file()
                .set_permissions(fs::Permissions::from_mode(0o600))?;
        }
        #[cfg(windows)]
        windows_local_security::secure_current_user_file(temporary.path())?;
        temporary.write_all(&serialize_preparation_entry(entry)?)?;
        temporary.as_file().sync_all()?;
        temporary
            .persist(&path)
            .map_err(|error| IsolationStoreError::Io(error.error))?;
        sync_private_directory(&self.directory)?;
        Ok(())
    }

    fn acquire_exclusive_lock(&self) -> Result<PreparationJournalFileLock, IsolationStoreError> {
        let path = self.directory.join(PREPARATION_JOURNAL_LOCK_FILE);
        let file = open_or_create_preparation_journal_lock(&path, &self.directory)?;
        fs2::FileExt::lock_exclusive(&file)?;
        Ok(PreparationJournalFileLock { file })
    }
}

fn open_or_create_preparation_journal_lock(
    path: &Path,
    directory: &Path,
) -> Result<File, IsolationStoreError> {
    loop {
        match fs::symlink_metadata(path) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() || !metadata.is_file() {
                    return Err(IsolationStoreError::InvalidRecord(
                        "preparation journal lock is not a regular file".into(),
                    ));
                }
                let file = open_existing_preparation_journal_file(path)?;
                let opened = file.metadata()?;
                if !opened.is_file() {
                    return Err(IsolationStoreError::InvalidRecord(
                        "preparation journal lock is not a regular file".into(),
                    ));
                }
                validate_private_journal_file(&opened)?;
                return Ok(file);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let mut options = OpenOptions::new();
                options.read(true).write(true).create_new(true);
                #[cfg(unix)]
                {
                    use std::os::unix::fs::OpenOptionsExt;
                    options.mode(0o600);
                }
                match options.open(path) {
                    Ok(file) => {
                        #[cfg(windows)]
                        windows_local_security::secure_current_user_file(path)?;
                        file.sync_all()?;
                        sync_private_directory(directory)?;
                        validate_private_journal_file(&file.metadata()?)?;
                        return Ok(file);
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                    Err(error) => return Err(IsolationStoreError::Io(error)),
                }
            }
            Err(error) => return Err(IsolationStoreError::Io(error)),
        }
    }
}

fn read_preparation_journal_entry(
    path: &Path,
) -> Result<PreparationJournalEntry, IsolationStoreError> {
    let file = open_existing_preparation_journal_file(path)?;
    let opened = file.metadata()?;
    if !opened.is_file() {
        return Err(IsolationStoreError::InvalidRecord(
            "preparation journal entry is not a regular file".into(),
        ));
    }
    validate_private_journal_file(&opened)?;
    if opened.len() > MAX_PREPARATION_JOURNAL_ENTRY_BYTES as u64 {
        return Err(IsolationStoreError::InvalidRecord(
            "preparation journal entry exceeds its size limit".into(),
        ));
    }
    let mut bytes = Vec::with_capacity(opened.len() as usize);
    file.take(MAX_PREPARATION_JOURNAL_ENTRY_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_PREPARATION_JOURNAL_ENTRY_BYTES {
        return Err(IsolationStoreError::InvalidRecord(
            "preparation journal entry exceeds its size limit".into(),
        ));
    }
    let entry: PreparationJournalEntry = serde_json::from_slice(&bytes).map_err(|_| {
        IsolationStoreError::InvalidRecord("preparation journal entry is unreadable".into())
    })?;
    validate_preparation_entry(&entry)?;
    Ok(entry)
}

fn open_existing_preparation_journal_file(path: &Path) -> Result<File, IsolationStoreError> {
    #[cfg(unix)]
    {
        let descriptor = rustix::fs::open(
            path,
            rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )?;
        Ok(File::from(descriptor))
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        let mut options = OpenOptions::new();
        options
            .read(true)
            .write(true)
            .custom_flags(WINDOWS_FILE_FLAG_OPEN_REPARSE_POINT);
        let file = options.open(path)?;
        let metadata = file.metadata()?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(IsolationStoreError::InvalidRecord(
                "preparation journal file is not a regular file".into(),
            ));
        }
        Ok(file)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = path;
        Err(IsolationStoreError::InvalidRecord(
            "preparation journal file access is unsupported on this platform".into(),
        ))
    }
}

fn validate_private_journal_file(metadata: &fs::Metadata) -> Result<(), IsolationStoreError> {
    if !metadata.is_file() {
        return Err(IsolationStoreError::InvalidRecord(
            "preparation journal file is not regular".into(),
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        if metadata.uid() != rustix::process::geteuid().as_raw()
            || metadata.permissions().mode() & 0o077 != 0
        {
            return Err(IsolationStoreError::InvalidRecord(
                "preparation journal file permissions or owner are unsafe".into(),
            ));
        }
    }
    Ok(())
}

fn serialize_preparation_entry(
    entry: &PreparationJournalEntry,
) -> Result<Vec<u8>, IsolationStoreError> {
    let mut bytes = serde_json::to_vec(entry).map_err(|_| {
        IsolationStoreError::InvalidRecord("preparation journal serialization failed".into())
    })?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn validate_preparation_intent(
    intent: &PreparationJournalIntent,
) -> Result<(), IsolationStoreError> {
    for value in [
        &intent.attempt_id,
        &intent.environment_id,
        &intent.workspace_id,
        &intent.task_id,
        &intent.step_id,
        &intent.runtime_id,
        &intent.runtime_incarnation_id,
    ] {
        validate_identifier(value)?;
    }
    if intent.provider_scope_id.len() != 32
        || !intent
            .provider_scope_id
            .bytes()
            .all(|value| value.is_ascii_hexdigit() && !value.is_ascii_uppercase())
    {
        return Err(IsolationStoreError::InvalidRecord(
            "provider scope identity is not a lowercase 128-bit token".into(),
        ));
    }
    if intent.admission_idempotency_key.is_empty()
        || intent.admission_idempotency_key.len() > 200
        || !intent.admission_idempotency_key.bytes().all(|value| {
            value.is_ascii_alphanumeric() || matches!(value, b'-' | b'_' | b'.' | b':')
        })
    {
        return Err(IsolationStoreError::InvalidRecord(
            "admission idempotency key is malformed".into(),
        ));
    }
    for digest in [
        &intent.request_digest,
        &intent.normalized_policy_digest,
        &intent.pinned_input_set_digest,
        &intent.readable_root_set_digest,
        &intent.writable_root_set_digest,
    ] {
        if !valid_sha256_digest(digest) {
            return Err(IsolationStoreError::InvalidRecord(
                "preparation journal contains a malformed digest".into(),
            ));
        }
    }
    Ok(())
}

fn validate_held_scope(scope: &HeldProviderScopeIdentity) -> Result<(), IsolationStoreError> {
    if scope.leader_pid == 0
        || scope.leader_start_time_ticks == 0
        || !valid_sha256_digest(&scope.process_scope_digest)
    {
        return Err(IsolationStoreError::InvalidRecord(
            "held provider scope identity is incomplete".into(),
        ));
    }
    if !scope.unit_name.starts_with("litecowork-attempt-") || !scope.unit_name.ends_with(".scope") {
        return Err(IsolationStoreError::InvalidRecord(
            "held provider scope unit name is malformed".into(),
        ));
    }
    Ok(())
}

fn validate_receipt_matches(
    intent: &PreparationJournalIntent,
    receipt: &PreparationAdmissionReceipt,
) -> Result<(), IsolationStoreError> {
    if receipt.receipt_id.is_empty()
        || receipt.receipt_id.len() > 128
        || !receipt
            .receipt_id
            .bytes()
            .all(|value| value.is_ascii_alphanumeric() || matches!(value, b'-' | b'_' | b'.'))
        || receipt.idempotency_key != intent.admission_idempotency_key
        || receipt.request_digest != intent.request_digest
        || receipt.workspace_id != intent.workspace_id
        || receipt.task_id != intent.task_id
        || receipt.attempt_id != intent.attempt_id
        || receipt.environment_id != intent.environment_id
        || receipt.runtime_id != intent.runtime_id
        || receipt.runtime_incarnation_id != intent.runtime_incarnation_id
    {
        return Err(IsolationStoreError::JournalConflict);
    }
    Ok(())
}

fn validate_preparation_entry(entry: &PreparationJournalEntry) -> Result<(), IsolationStoreError> {
    if entry.schema_version != PREPARATION_JOURNAL_VERSION {
        return Err(IsolationStoreError::InvalidRecord(
            "unsupported preparation journal version".into(),
        ));
    }
    validate_preparation_intent(&entry.intent)?;
    match (entry.phase, &entry.held_scope, &entry.admission_receipt) {
        (PreparationJournalPhase::IntentWritten, None, None) => {}
        (PreparationJournalPhase::ScopeHeld, Some(scope), None) => {
            validate_held_scope(scope)?;
            if scope.unit_name != scope_unit_name(&entry.intent.provider_scope_id) {
                return Err(IsolationStoreError::JournalConflict);
            }
        }
        (
            PreparationJournalPhase::AdmissionCommitted
            | PreparationJournalPhase::LaunchHandoffAttempted,
            Some(scope),
            Some(receipt),
        ) => {
            validate_held_scope(scope)?;
            if scope.unit_name != scope_unit_name(&entry.intent.provider_scope_id) {
                return Err(IsolationStoreError::JournalConflict);
            }
            validate_receipt_matches(&entry.intent, receipt)?;
        }
        _ => {
            return Err(IsolationStoreError::InvalidRecord(
                "preparation journal phase has inconsistent evidence".into(),
            ));
        }
    }
    Ok(())
}

fn valid_sha256_digest(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    })
}

fn scope_unit_name(provider_scope_id: &str) -> String {
    format!("litecowork-attempt-{provider_scope_id}.scope")
}

fn sync_private_directory(path: &Path) -> Result<(), IsolationStoreError> {
    #[cfg(unix)]
    File::open(path)?.sync_all()?;
    Ok(())
}

fn validate_identifier(value: &str) -> Result<(), IsolationStoreError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        || value == "."
        || value == ".."
    {
        return Err(IsolationStoreError::InvalidIdentifier(value.to_owned()));
    }
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// Probe the local provider on this host.
///
/// The probe is deliberately conservative. It reports what the host exposes, and it
/// never claims that an enforcement dimension is active: only an observed launch can
/// produce an attestation.
pub fn probe_local_provider() -> LocalProviderProbe {
    linux_process_scope::probe_local_provider()
}

/// Convert a provider probe into observed enforcement facts.
///
/// A probe cannot prove that a restriction was enforced at run time, so every
/// dimension is reported as `Unknown`. The resulting attestation is refused by
/// [`evaluate_isolation_admission`], which is the intended fail-closed behaviour.
pub fn facts_from_probe(capabilities: &LocalProviderProbe) -> ProviderIsolationFacts {
    ProviderIsolationFacts {
        provider_kind: "local-bubblewrap".to_owned(),
        provider_version: "1".to_owned(),
        operating_system: capabilities.operating_system.clone(),
        operating_system_build: capabilities.operating_system_build.clone(),
        filesystem_enforcement: EnforcementStatus::Unknown,
        process_tree_enforcement: EnforcementStatus::Unknown,
        network_enforcement: EnforcementStatus::Unknown,
        resource_limit_enforcement: EnforcementStatus::Unknown,
    }
}

/// Digest of a normalized isolation policy.
///
/// The policy is expressed as an ordered list of normalized labels so that the
/// Runtime and the provider derive the same digest for the same logical policy.
pub fn normalized_policy_digest(labels: &[String]) -> String {
    let mut unique = std::collections::BTreeSet::new();
    for label in labels {
        unique.insert(label.trim().to_owned());
    }
    let mut hasher = Sha256::new();
    for label in unique {
        hasher.update(label.as_bytes());
        hasher.update(b"\n");
    }
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

/// Digest of a structured normalized policy.
pub fn normalized_policy_digest_from_json(value: &serde_json::Value) -> String {
    let mut hasher = Sha256::new();
    hash_canonical(value, &mut hasher);
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

fn hash_canonical(value: &serde_json::Value, hasher: &mut Sha256) {
    match value {
        serde_json::Value::Null => hasher.update(b"null"),
        serde_json::Value::Bool(true) => hasher.update(b"true"),
        serde_json::Value::Bool(false) => hasher.update(b"false"),
        serde_json::Value::Number(number) => {
            hasher.update(b"n");
            hasher.update(number.to_string().as_bytes());
        }
        serde_json::Value::String(text) => {
            hasher.update(b"s");
            hasher.update(text.as_bytes());
        }
        serde_json::Value::Array(items) => {
            hasher.update(b"[");
            for item in items {
                hash_canonical(item, hasher);
                hasher.update(b",");
            }
            hasher.update(b"]");
        }
        serde_json::Value::Object(entries) => {
            hasher.update(b"{");
            let mut keys = entries.keys().collect::<Vec<_>>();
            keys.sort();
            for key in keys {
                hasher.update(b"k");
                hasher.update(key.as_bytes());
                hasher.update(b"v");
                hash_canonical(&entries[key], hasher);
                hasher.update(b",");
            }
            hasher.update(b"}");
        }
    }
}

/// Map a normalized policy document into its stable label list.
pub fn policy_labels(value: &serde_json::Value) -> Vec<String> {
    let mut labels = Vec::new();
    if let serde_json::Value::Object(entries) = value {
        for (key, entry) in entries {
            labels.push(format!("{key}={}", canonical_label(entry)));
        }
    }
    labels.sort();
    labels
}

fn canonical_label(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "null".to_owned(),
        serde_json::Value::Bool(true) => "true".to_owned(),
        serde_json::Value::Bool(false) => "false".to_owned(),
        serde_json::Value::Number(number) => number.to_string(),
        serde_json::Value::String(text) => text.clone(),
        serde_json::Value::Array(items) => {
            let inner = items.iter().map(canonical_label).collect::<Vec<_>>();
            format!("[{}]", inner.join(","))
        }
        serde_json::Value::Object(entries) => {
            let mut keys = entries.keys().collect::<Vec<_>>();
            keys.sort();
            let inner = keys
                .iter()
                .map(|key| format!("{key}={}", canonical_label(&entries[*key])))
                .collect::<Vec<_>>();
            format!("{{{}}}", inner.join(","))
        }
    }
}

/// The current wall-clock time in Unix epoch milliseconds.
pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or_default()
}

/// Build the digest of a private execution identity.
///
/// The execution identity is Runtime-local and never published; only its digest is
/// used to bind a quiescence observation.
pub fn execution_identity_digest(parts: &[&str]) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part.as_bytes());
        hasher.update([0x1f]);
    }
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

/// Sorted label map helper used by provider adapters that must record several
/// observations without depending on iteration order.
pub fn sorted_observations(observations: &[(&str, &str)]) -> BTreeMap<String, String> {
    observations
        .iter()
        .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn digest(bytes: &[u8]) -> String {
        format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
    }

    fn prepared() -> PreparedRootDigests {
        PreparedRootDigests {
            pinned_input_set_digest: digest(b"pinned input set"),
            readable_root_set_digest: digest(b"inputs"),
            writable_root_set_digest: digest(b"outputs"),
        }
    }

    fn identity() -> IsolationIdentity {
        IsolationIdentity {
            environment_id: "env-1".to_owned(),
            workspace_id: "workspace-1".to_owned(),
            task_id: "task-1".to_owned(),
            runtime_id: "runtime-1".to_owned(),
            runtime_incarnation_id: "incarnation-1".to_owned(),
            normalized_policy_digest: normalized_policy_digest(&[
                "filesystem=PRIVATE_COPY".to_owned(),
                "network=NONE".to_owned(),
                "process=NAMESPACE".to_owned(),
            ]),
        }
    }

    fn observed_facts() -> ProviderIsolationFacts {
        ProviderIsolationFacts {
            provider_kind: "local-bubblewrap".to_owned(),
            provider_version: "1".to_owned(),
            operating_system: "linux".to_owned(),
            operating_system_build: "6.8.0".to_owned(),
            filesystem_enforcement: EnforcementStatus::Enforced,
            process_tree_enforcement: EnforcementStatus::Enforced,
            network_enforcement: EnforcementStatus::Enforced,
            resource_limit_enforcement: EnforcementStatus::Enforced,
        }
    }

    fn attestation() -> IsolationAttestation {
        build_attestation(&identity(), &prepared(), observed_facts(), 1_000, 60_000)
            .expect("attestation")
    }

    fn request(attestation: &IsolationAttestation) -> IsolationAdmissionRequest {
        IsolationAdmissionRequest {
            environment_id: attestation.environment_id.clone(),
            workspace_id: attestation.workspace_id.clone(),
            task_id: attestation.task_id.clone(),
            runtime_id: attestation.runtime_id.clone(),
            runtime_incarnation_id: attestation.runtime_incarnation_id.clone(),
            normalized_policy_digest: attestation.normalized_policy_digest.clone(),
            pinned_input_set_digest: attestation.pinned_input_set_digest.clone(),
            readable_root_set_digest: attestation.readable_root_set_digest.clone(),
            writable_root_set_digest: attestation.writable_root_set_digest.clone(),
            now_ms: 2_000,
        }
    }

    #[test]
    fn a_fully_observed_attestation_admits_the_matching_request() {
        let attestation = attestation();
        assert_eq!(
            evaluate_isolation_admission(Some(&attestation), &request(&attestation)),
            Ok(())
        );
    }

    #[test]
    fn a_missing_attestation_fails_closed() {
        let attestation = attestation();
        assert_eq!(
            evaluate_isolation_admission(None, &request(&attestation)),
            Err(IsolationAdmissionError::AttestationMissing)
        );
    }

    /// One case of `every_identity_and_digest_mismatch_fails_closed`: the mutated
    /// field's name, the mutation, and the refusal it must produce.
    type MutationCase = (
        &'static str,
        fn(&mut IsolationAdmissionRequest),
        IsolationAdmissionError,
    );

    #[test]
    fn every_identity_and_digest_mismatch_fails_closed() {
        let attestation = attestation();
        let mutations: Vec<MutationCase> = vec![
            (
                "environment",
                |request| request.environment_id = "env-other".to_owned(),
                IsolationAdmissionError::EnvironmentMismatch,
            ),
            (
                "workspace",
                |request| request.workspace_id = "workspace-other".to_owned(),
                IsolationAdmissionError::WorkspaceMismatch,
            ),
            (
                "task",
                |request| request.task_id = "task-other".to_owned(),
                IsolationAdmissionError::TaskMismatch,
            ),
            (
                "runtime",
                |request| request.runtime_id = "runtime-other".to_owned(),
                IsolationAdmissionError::RuntimeMismatch,
            ),
            (
                "incarnation",
                |request| request.runtime_incarnation_id = "incarnation-2".to_owned(),
                IsolationAdmissionError::RuntimeIncarnationMismatch,
            ),
            (
                "policy",
                |request| request.normalized_policy_digest = "sha256:00".to_owned(),
                IsolationAdmissionError::PolicyDigestMismatch,
            ),
            (
                "inputs",
                |request| request.pinned_input_set_digest = "sha256:00".to_owned(),
                IsolationAdmissionError::PinnedInputSetMismatch,
            ),
            (
                "readable",
                |request| request.readable_root_set_digest = "sha256:00".to_owned(),
                IsolationAdmissionError::ReadableRootSetMismatch,
            ),
            (
                "writable",
                |request| request.writable_root_set_digest = "sha256:00".to_owned(),
                IsolationAdmissionError::WritableRootSetMismatch,
            ),
        ];
        for (name, mutate, expected) in mutations {
            let mut request = request(&attestation);
            mutate(&mut request);
            assert_eq!(
                evaluate_isolation_admission(Some(&attestation), &request),
                Err(expected),
                "{name}"
            );
        }
    }

    #[test]
    fn unknown_or_not_enforced_dimensions_fail_closed() {
        for status in [EnforcementStatus::Unknown, EnforcementStatus::NotEnforced] {
            let mut facts = observed_facts();
            facts.filesystem_enforcement = status;
            facts.process_tree_enforcement = status;
            facts.network_enforcement = status;
            facts.resource_limit_enforcement = status;
            let attestation = build_attestation(&identity(), &prepared(), facts, 1_000, 60_000)
                .expect("attestation");
            assert_eq!(
                evaluate_isolation_admission(Some(&attestation), &request(&attestation)),
                Err(IsolationAdmissionError::FilesystemEnforcementMissing)
            );
        }
    }

    #[test]
    fn each_missing_enforcement_dimension_is_reported_individually() {
        let cases = [
            (
                IsolationAdmissionError::FilesystemEnforcementMissing,
                0usize,
            ),
            (IsolationAdmissionError::ProcessTreeEnforcementMissing, 1),
            (IsolationAdmissionError::NetworkEnforcementMissing, 2),
            (IsolationAdmissionError::ResourceLimitEnforcementMissing, 3),
        ];
        for (expected, index) in cases {
            let mut facts = observed_facts();
            match index {
                0 => facts.filesystem_enforcement = EnforcementStatus::Unknown,
                1 => facts.process_tree_enforcement = EnforcementStatus::Unknown,
                2 => facts.network_enforcement = EnforcementStatus::Unknown,
                _ => facts.resource_limit_enforcement = EnforcementStatus::Unknown,
            }
            let attestation = build_attestation(&identity(), &prepared(), facts, 1_000, 60_000)
                .expect("attestation");
            assert_eq!(
                evaluate_isolation_admission(Some(&attestation), &request(&attestation)),
                Err(expected)
            );
        }
    }

    #[test]
    fn an_expired_attestation_fails_closed() {
        let attestation = attestation();
        let mut request = request(&attestation);
        request.now_ms = attestation.expires_at_ms;
        assert_eq!(
            evaluate_isolation_admission(Some(&attestation), &request),
            Err(IsolationAdmissionError::AttestationExpired)
        );
    }

    #[test]
    fn an_observation_from_the_future_fails_closed() {
        let attestation = attestation();
        let mut request = request(&attestation);
        request.now_ms = attestation.observed_at_ms - 1;
        assert_eq!(
            evaluate_isolation_admission(Some(&attestation), &request),
            Err(IsolationAdmissionError::ObservationFromTheFuture)
        );
    }

    #[test]
    fn a_provider_probe_cannot_produce_an_admitting_attestation() {
        let capabilities = probe_local_provider();
        let facts = facts_from_probe(&capabilities);
        let attestation =
            build_attestation(&identity(), &prepared(), facts, 1_000, 60_000).expect("attestation");
        assert_eq!(
            evaluate_isolation_admission(Some(&attestation), &request(&attestation)),
            Err(IsolationAdmissionError::FilesystemEnforcementMissing)
        );
    }

    #[test]
    fn the_store_keeps_evidence_private_to_one_incarnation() {
        let temp = tempdir().unwrap();
        let attestation = attestation();
        let store = RuntimeLocalIsolationStore::open(temp.path(), "runtime-1", "incarnation-1")
            .expect("store");
        store.store(&attestation).expect("stored");

        let loaded = store.load("env-1").expect("load").expect("present");
        assert_eq!(loaded, attestation);

        // A new incarnation has its own directory and cannot see the old evidence.
        let restarted = RuntimeLocalIsolationStore::open(temp.path(), "runtime-1", "incarnation-2")
            .expect("store");
        assert_eq!(restarted.load("env-1").expect("load"), None);
    }

    #[test]
    fn the_store_refuses_evidence_from_another_incarnation() {
        let temp = tempdir().unwrap();
        let attestation = attestation();
        let store = RuntimeLocalIsolationStore::open(temp.path(), "runtime-1", "incarnation-1")
            .expect("store");
        let mut foreign = attestation.clone();
        foreign.runtime_incarnation_id = "incarnation-2".to_owned();
        assert!(matches!(
            store.store(&foreign),
            Err(IsolationStoreError::IncarnationMismatch)
        ));
    }

    #[test]
    fn the_store_never_replaces_newer_evidence_with_older_evidence() {
        let temp = tempdir().unwrap();
        let store = RuntimeLocalIsolationStore::open(temp.path(), "runtime-1", "incarnation-1")
            .expect("store");
        let mut newer = attestation();
        newer.observed_at_ms = 5_000;
        newer.expires_at_ms = 65_000;
        store.store(&newer).expect("stored");

        let mut older = newer.clone();
        older.observed_at_ms = 4_000;
        assert!(matches!(
            store.store(&older),
            Err(IsolationStoreError::StaleAttestation)
        ));
        assert_eq!(
            store
                .load("env-1")
                .expect("load")
                .expect("present")
                .observed_at_ms,
            5_000
        );
    }

    #[test]
    fn removing_evidence_drops_it_for_the_next_admission() {
        let temp = tempdir().unwrap();
        let attestation = attestation();
        let store = RuntimeLocalIsolationStore::open(temp.path(), "runtime-1", "incarnation-1")
            .expect("store");
        store.store(&attestation).expect("stored");
        assert!(store.remove("env-1").expect("removed"));
        assert!(!store.remove("env-1").expect("already absent"));
        assert_eq!(store.load("env-1").expect("load"), None);
    }

    #[test]
    fn stored_evidence_contains_no_host_path() {
        let temp = tempdir().unwrap();
        let attestation = attestation();
        let store = RuntimeLocalIsolationStore::open(temp.path(), "runtime-1", "incarnation-1")
            .expect("store");
        store.store(&attestation).expect("stored");
        let path = store.record_path("env-1").expect("path");
        let rendered = fs::read_to_string(&path).expect("record");
        assert!(!rendered.contains(temp.path().to_string_lossy().as_ref()));
        assert!(rendered.contains("sha256:"));
    }

    #[test]
    fn the_store_rejects_unsafe_identifiers() {
        let temp = tempdir().unwrap();
        assert!(matches!(
            RuntimeLocalIsolationStore::open(temp.path(), "../escape", "incarnation-1"),
            Err(IsolationStoreError::InvalidIdentifier(_))
        ));
        assert!(matches!(
            RuntimeLocalIsolationStore::open(temp.path(), "runtime-1", "incarnation/1"),
            Err(IsolationStoreError::InvalidIdentifier(_))
        ));
    }

    fn preparation_intent() -> PreparationJournalIntent {
        PreparationJournalIntent {
            attempt_id: "attempt-1".to_owned(),
            environment_id: "env-1".to_owned(),
            workspace_id: "workspace-1".to_owned(),
            task_id: "task-1".to_owned(),
            step_id: "step-1".to_owned(),
            runtime_id: "runtime-1".to_owned(),
            runtime_incarnation_id: "incarnation-1".to_owned(),
            provider_scope_id: "0123456789abcdef0123456789abcdef".to_owned(),
            admission_idempotency_key: "attempt-admission-1".to_owned(),
            request_digest: digest(b"admission request"),
            normalized_policy_digest: digest(b"policy"),
            pinned_input_set_digest: digest(b"inputs"),
            readable_root_set_digest: digest(b"readable roots"),
            writable_root_set_digest: digest(b"writable roots"),
        }
    }

    fn held_provider_scope() -> HeldProviderScopeIdentity {
        HeldProviderScopeIdentity {
            unit_name: "litecowork-attempt-0123456789abcdef0123456789abcdef.scope".to_owned(),
            leader_pid: 456,
            leader_start_time_ticks: 789,
            process_scope_digest: digest(b"exact cgroup scope"),
        }
    }

    fn preparation_receipt() -> PreparationAdmissionReceipt {
        PreparationAdmissionReceipt {
            receipt_id: "receipt-1".to_owned(),
            idempotency_key: "attempt-admission-1".to_owned(),
            request_digest: digest(b"admission request"),
            workspace_id: "workspace-1".to_owned(),
            task_id: "task-1".to_owned(),
            attempt_id: "attempt-1".to_owned(),
            environment_id: "env-1".to_owned(),
            runtime_id: "runtime-1".to_owned(),
            runtime_incarnation_id: "incarnation-1".to_owned(),
        }
    }

    #[test]
    fn preparation_journal_is_durable_private_and_visible_to_a_new_incarnation() {
        let temp = tempdir().unwrap();
        let journal = RuntimeLocalPreparationJournal::open(temp.path(), "runtime-1")
            .expect("private preparation journal");
        let intent = preparation_intent();
        journal
            .begin(intent.clone())
            .expect("write ahead of provider creation");

        let restarted = RuntimeLocalPreparationJournal::open(temp.path(), "runtime-1")
            .expect("same Runtime journal after restart");
        let loaded = restarted
            .load(&intent.attempt_id)
            .expect("read journal")
            .expect("durable intent remains visible");
        assert_eq!(loaded.intent, intent);
        assert_eq!(loaded.phase, PreparationJournalPhase::IntentWritten);
        let rendered =
            fs::read_to_string(restarted.record_path(&loaded.intent.attempt_id).unwrap())
                .expect("journal file");
        assert!(!rendered.contains(temp.path().to_string_lossy().as_ref()));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(restarted.record_path(&loaded.intent.attempt_id).unwrap())
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
    }

    #[test]
    fn preparation_journal_pending_scan_rejects_noncanonical_entry_names() {
        let temp = tempdir().unwrap();
        let journal = RuntimeLocalPreparationJournal::open(temp.path(), "runtime-1")
            .expect("private preparation journal");
        let intent = preparation_intent();
        journal.begin(intent.clone()).expect("begin");
        let canonical = journal.record_path(&intent.attempt_id).unwrap();
        let noncanonical = journal.directory.join("other-attempt.json");
        fs::rename(canonical, noncanonical).expect("rename to a noncanonical journal key");

        assert!(matches!(
            journal.list_pending(),
            Err(IsolationStoreError::InvalidRecord(_))
        ));
    }

    #[test]
    fn preparation_journal_reads_reject_oversized_entries_before_parsing() {
        let temp = tempdir().unwrap();
        let journal = RuntimeLocalPreparationJournal::open(temp.path(), "runtime-1")
            .expect("private preparation journal");
        let intent = preparation_intent();
        journal.begin(intent.clone()).expect("begin");
        let path = journal.record_path(&intent.attempt_id).unwrap();
        fs::write(&path, vec![b' '; MAX_PREPARATION_JOURNAL_ENTRY_BYTES + 1])
            .expect("replace with oversized journal record");

        for result in [
            journal.load(&intent.attempt_id).map(|_| ()),
            journal.list_pending().map(|_| ()),
        ] {
            assert!(matches!(
                result,
                Err(IsolationStoreError::InvalidRecord(reason)) if reason.contains("size limit")
            ));
        }
    }

    #[cfg(unix)]
    #[test]
    fn preparation_journal_pending_scan_rejects_symlink_entries() {
        use std::os::unix::fs::symlink;

        let temp = tempdir().unwrap();
        let journal = RuntimeLocalPreparationJournal::open(temp.path(), "runtime-1")
            .expect("private preparation journal");
        let intent = preparation_intent();
        journal.begin(intent.clone()).expect("begin");
        let canonical = journal.record_path(&intent.attempt_id).unwrap();
        let saved = journal.directory.join("saved.json");
        fs::rename(&canonical, &saved).expect("move original entry");
        symlink(&saved, &canonical).expect("replace canonical entry with symlink");

        assert!(matches!(
            journal.list_pending(),
            Err(IsolationStoreError::InvalidRecord(_))
        ));
        assert!(matches!(
            journal.load(&intent.attempt_id),
            Err(IsolationStoreError::InvalidRecord(_))
        ));
    }

    #[test]
    fn preparation_journal_serializes_mutations_across_processes() {
        use std::{process::Command, thread, time::Duration};

        let temp = tempdir().unwrap();
        let journal =
            RuntimeLocalPreparationJournal::open(temp.path(), "runtime-1").expect("journal handle");
        let held_lock = journal.acquire_exclusive_lock().expect("hold journal lock");
        let started = temp.path().join("child-started");
        let done = temp.path().join("child-done");
        let mut child = Command::new(std::env::current_exe().expect("test executable"))
            .arg("--exact")
            .arg("environment_isolation::tests::preparation_journal_lock_child")
            .arg("--nocapture")
            .env("LITECOWORK_PREPARATION_JOURNAL_TEST_ROOT", temp.path())
            .spawn()
            .expect("start another process against the same journal");

        let started_at = std::time::Instant::now();
        while !started.exists() {
            if let Some(status) = child.try_wait().expect("inspect child test") {
                panic!("child test exited before reaching the journal: {status}");
            }
            assert!(started_at.elapsed() < Duration::from_secs(5));
            thread::sleep(Duration::from_millis(10));
        }
        thread::sleep(Duration::from_millis(100));
        assert!(
            !done.exists(),
            "child mutation must wait for the journal lock"
        );

        drop(held_lock);
        let status = child.wait().expect("wait for journal child");
        assert!(status.success(), "child journal operation must succeed");
        assert!(done.exists(), "child mutation completes after lock release");
    }

    #[test]
    fn preparation_journal_lock_child() {
        let Some(root) = std::env::var_os("LITECOWORK_PREPARATION_JOURNAL_TEST_ROOT") else {
            return;
        };
        let root = PathBuf::from(root);
        let journal = RuntimeLocalPreparationJournal::open(&root, "runtime-1")
            .expect("open journal in child process");
        fs::write(root.join("child-started"), b"started").expect("signal before begin");
        journal
            .begin(preparation_intent())
            .expect("begin after acquiring cross-process lock");
        fs::write(root.join("child-done"), b"done").expect("signal completed begin");
    }

    #[test]
    fn preparation_journal_requires_exact_scope_receipt_and_one_shot_handoff_order() {
        let temp = tempdir().unwrap();
        let journal = RuntimeLocalPreparationJournal::open(temp.path(), "runtime-1")
            .expect("private preparation journal");
        let intent = preparation_intent();
        journal.begin(intent.clone()).expect("begin");
        assert!(matches!(
            journal.mark_launch_handoff_attempted(&intent.attempt_id),
            Err(IsolationStoreError::JournalTransition(_))
        ));
        journal
            .record_scope_held(&intent.attempt_id, held_provider_scope())
            .expect("bind exact held scope");

        let mut mismatched = preparation_receipt();
        mismatched.request_digest = digest(b"different request");
        assert!(matches!(
            journal.record_admission_receipt(&intent.attempt_id, mismatched),
            Err(IsolationStoreError::JournalConflict)
        ));
        journal
            .record_admission_receipt(&intent.attempt_id, preparation_receipt())
            .expect("record exact committed receipt");
        journal
            .mark_launch_handoff_attempted(&intent.attempt_id)
            .expect("persist one-shot intent before signaling runner");
        assert!(matches!(
            journal.mark_launch_handoff_attempted(&intent.attempt_id),
            Err(IsolationStoreError::JournalTransition(_))
        ));
    }

    #[test]
    fn preparation_recovery_never_guesses_when_receipt_lookup_is_unknown() {
        let temp = tempdir().unwrap();
        let journal = RuntimeLocalPreparationJournal::open(temp.path(), "runtime-1")
            .expect("private preparation journal");
        let intent = preparation_intent();
        journal.begin(intent.clone()).expect("begin");
        assert_eq!(
            journal
                .recovery_action(&intent.attempt_id, PreparationReceiptLookup::Unknown)
                .expect("recovery decision"),
            PreparationRecoveryAction::BlockUnknown
        );
        assert!(matches!(
            journal.recovery_action(
                &intent.attempt_id,
                PreparationReceiptLookup::DefinitelyAbsent
            ),
            Ok(PreparationRecoveryAction::InspectExactProviderScope { .. })
        ));
    }

    #[test]
    fn a_quiescence_observation_only_releases_the_exact_execution_identity() {
        let observation = QuiescenceObservation {
            environment_id: "env-1".to_owned(),
            runtime_id: "runtime-1".to_owned(),
            runtime_incarnation_id: "incarnation-1".to_owned(),
            execution_identity_digest: "sha256:aa".to_owned(),
            process_scope_digest: "sha256:bb".to_owned(),
            result: QuiescenceResult::Quiescent,
            observed_at_ms: 1_000,
        };
        assert_eq!(
            evaluate_quiescence(
                Some(&observation),
                "env-1",
                "runtime-1",
                "incarnation-1",
                "sha256:aa",
                "sha256:bb",
                2_000
            ),
            Ok(())
        );
        assert_eq!(
            evaluate_quiescence(
                Some(&observation),
                "env-1",
                "runtime-1",
                "incarnation-2",
                "sha256:aa",
                "sha256:bb",
                2_000
            ),
            Err(QuiescenceAdmissionError::RuntimeIncarnationMismatch)
        );
        assert_eq!(
            evaluate_quiescence(
                Some(&observation),
                "env-1",
                "runtime-1",
                "incarnation-1",
                "sha256:cc",
                "sha256:bb",
                2_000
            ),
            Err(QuiescenceAdmissionError::ExecutionIdentityMismatch)
        );
        assert_eq!(
            evaluate_quiescence(
                None,
                "env-1",
                "runtime-1",
                "incarnation-1",
                "sha256:aa",
                "sha256:bb",
                2_000
            ),
            Err(QuiescenceAdmissionError::ObservationMissing)
        );
    }

    #[test]
    fn an_unknown_quiescence_result_never_releases_a_writer() {
        let observation = QuiescenceObservation {
            environment_id: "env-1".to_owned(),
            runtime_id: "runtime-1".to_owned(),
            runtime_incarnation_id: "incarnation-1".to_owned(),
            execution_identity_digest: "sha256:aa".to_owned(),
            process_scope_digest: "sha256:bb".to_owned(),
            result: QuiescenceResult::Unknown,
            observed_at_ms: 1_000,
        };
        assert_eq!(
            evaluate_quiescence(
                Some(&observation),
                "env-1",
                "runtime-1",
                "incarnation-1",
                "sha256:aa",
                "sha256:bb",
                2_000
            ),
            Err(QuiescenceAdmissionError::ResultNotQuiescent)
        );
    }

    #[test]
    fn a_normalized_policy_digest_is_order_independent() {
        let left = normalized_policy_digest(&[
            "network=NONE".to_owned(),
            "filesystem=PRIVATE_COPY".to_owned(),
        ]);
        let right = normalized_policy_digest(&[
            "filesystem=PRIVATE_COPY".to_owned(),
            "network=NONE".to_owned(),
        ]);
        assert_eq!(left, right);
        assert_ne!(
            left,
            normalized_policy_digest(&["network=DEFAULT".to_owned()])
        );
    }

    #[test]
    fn a_structured_policy_digest_ignores_object_key_order() {
        let left = serde_json::json!({"filesystem": "PRIVATE_COPY", "network": "NONE"});
        let right = serde_json::json!({"network": "NONE", "filesystem": "PRIVATE_COPY"});
        assert_eq!(
            normalized_policy_digest_from_json(&left),
            normalized_policy_digest_from_json(&right)
        );
        assert_ne!(
            normalized_policy_digest_from_json(&left),
            normalized_policy_digest_from_json(&serde_json::json!({"network": "DEFAULT"}))
        );
    }

    #[test]
    fn the_local_provider_probe_is_truthful_about_what_it_cannot_observe() {
        let capabilities = probe_local_provider();
        assert!(!capabilities.operating_system.is_empty());
        let facts = facts_from_probe(&capabilities);
        assert_eq!(facts.filesystem_enforcement, EnforcementStatus::Unknown);
        assert_eq!(facts.process_tree_enforcement, EnforcementStatus::Unknown);
        assert_eq!(facts.network_enforcement, EnforcementStatus::Unknown);
        assert_eq!(facts.resource_limit_enforcement, EnforcementStatus::Unknown);
    }
}
