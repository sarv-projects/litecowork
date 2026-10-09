#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn identity() -> AttemptProcessScopeIdentity {
        AttemptProcessScopeIdentity::new(
            "workspace-1",
            "task-1",
            "step-1",
            "attempt-1",
            "environment-1",
            "runtime-1",
            "incarnation-1",
        )
        .expect("valid identity")
    }

    #[test]
    fn journal_refuses_a_record_with_missing_execution_identity() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let journal = AttemptProcessScopeJournal::open(directory.path()).expect("journal");
        let path = journal.record_path(&identity());
        fs::write(path, br#"{"state":"ACTIVE"}"#).expect("corrupt record");

        assert!(matches!(
            journal.incomplete_records(),
            Err(AttemptProcessScopeError::InvalidRecord(_))
        ));
    }

    #[test]
    fn unavailable_quiescence_keeps_the_process_identity_unresolved() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let journal = AttemptProcessScopeJournal::open(directory.path()).expect("journal");
        let identity = identity();
        journal
            .record_unresolved(&identity, "QUIESCENCE_UNOBSERVABLE")
            .expect("persist unresolved identity");

        let records = journal.incomplete_records().expect("load");
        assert_eq!(records, vec![identity.clone()]);
        assert!(matches!(
            journal.settle(&identity, None),
            Err(AttemptProcessScopeError::QuiescenceProofRequired)
        ));
        assert_eq!(
            journal.incomplete_records().expect("still blocked"),
            records
        );
    }

    #[test]
    fn parent_exit_does_not_create_a_quiescence_proof() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let journal = AttemptProcessScopeJournal::open(directory.path()).expect("journal");
        let identity = identity();
        journal.record_active(&identity).expect("record active");

        // A parent/launcher exit observation is not a RuntimeQuiescenceProof. Only
        // ManagedAttemptScope::wait_for_quiescence can construct one.
        assert!(matches!(
            journal.settle(&identity, None),
            Err(AttemptProcessScopeError::QuiescenceProofRequired)
        ));
        assert_eq!(
            journal.incomplete_records().expect("still active"),
            vec![identity]
        );
    }

    #[test]
    fn duplicate_create_cleans_temporary_file_and_allows_later_update() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let journal = AttemptProcessScopeJournal::open(directory.path()).expect("journal");
        let identity = identity();
        journal.record_active(&identity).expect("initial record");
        assert!(matches!(
            journal.record_active(&identity),
            Err(AttemptProcessScopeError::DuplicateIdentity)
        ));
        assert_eq!(
            fs::read_dir(directory.path())
                .expect("journal entries")
                .count(),
            1
        );
        journal
            .record_unresolved(&identity, "QUIESCENCE_UNPROVEN")
            .expect("update following duplicate must succeed");
        assert_eq!(
            fs::read_dir(directory.path())
                .expect("journal entries")
                .count(),
            1
        );
        assert_eq!(
            journal.incomplete_records().expect("read journal"),
            vec![identity]
        );
    }

    #[test]
    fn stale_old_pid_temporary_file_does_not_block_new_record() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let journal = AttemptProcessScopeJournal::open(directory.path()).expect("journal");
        let identity = identity();
        let stale =
            directory
                .path()
                .join(format!(".{}.{}.tmp", identity.digest(), std::process::id()));
        fs::write(&stale, b"stale incomplete write").expect("create stale path");
        journal
            .record_active(&identity)
            .expect("create despite stale temporary path");
        assert_eq!(
            journal.incomplete_records().expect("read journal"),
            vec![identity]
        );
        assert_eq!(
            fs::read(&stale).expect("stale file retained"),
            b"stale incomplete write"
        );
    }

    #[test]
    fn quiescence_proof_is_bound_to_the_exact_runtime_incarnation() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let journal = AttemptProcessScopeJournal::open(directory.path()).expect("journal");
        let identity = identity();
        journal.record_active(&identity).expect("record active");
        let other_incarnation = AttemptProcessScopeIdentity::new(
            "workspace-1",
            "task-1",
            "step-1",
            "attempt-1",
            "environment-1",
            "runtime-1",
            "incarnation-2",
        )
        .expect("valid other incarnation");
        let proof = RuntimeQuiescenceProof {
            identity_digest: other_incarnation.digest(),
        };

        assert!(matches!(
            journal.settle(&identity, Some(&proof)),
            Err(AttemptProcessScopeError::QuiescenceProofMismatch)
        ));
        assert_eq!(
            journal.incomplete_records().expect("still active"),
            vec![identity]
        );
    }
}
use linux_process_scope::{LaunchSpec, ManagedScope, ScopeError, SystemdCommands, spawn};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AttemptProcessScopeIdentity {
    workspace_id: String,
    task_id: String,
    step_id: String,
    attempt_id: String,
    environment_id: String,
    runtime_id: String,
    runtime_incarnation_id: String,
}

impl AttemptProcessScopeIdentity {
    pub(super) fn new(
        workspace_id: &str,
        task_id: &str,
        step_id: &str,
        attempt_id: &str,
        environment_id: &str,
        runtime_id: &str,
        runtime_incarnation_id: &str,
    ) -> Result<Self, AttemptProcessScopeError> {
        let values = [
            workspace_id,
            task_id,
            step_id,
            attempt_id,
            environment_id,
            runtime_id,
            runtime_incarnation_id,
        ];
        if values.iter().any(|value| {
            value.is_empty()
                || value.len() > 128
                || !value
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
                || *value == "."
                || *value == ".."
        }) {
            return Err(AttemptProcessScopeError::InvalidIdentity);
        }
        Ok(Self {
            workspace_id: workspace_id.to_owned(),
            task_id: task_id.to_owned(),
            step_id: step_id.to_owned(),
            attempt_id: attempt_id.to_owned(),
            environment_id: environment_id.to_owned(),
            runtime_id: runtime_id.to_owned(),
            runtime_incarnation_id: runtime_incarnation_id.to_owned(),
        })
    }

    fn digest(&self) -> String {
        let bytes = serde_json::to_vec(self).expect("identity serialization is infallible");
        hex::encode(Sha256::digest(bytes))
    }

    fn supervisor_attempt_id(&self) -> [u8; 16] {
        let digest = Sha256::digest(serde_json::to_vec(self).expect("identity serialization"));
        digest[..16].try_into().expect("fixed digest prefix")
    }
}

#[derive(Debug)]
pub(super) enum AttemptProcessScopeError {
    InvalidIdentity,
    Io(std::io::Error),
    InvalidRecord(String),
    DuplicateIdentity,
    QuiescenceProofRequired,
    QuiescenceProofMismatch,
}

impl From<std::io::Error> for AttemptProcessScopeError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum AttemptProcessScopeState {
    Active,
    Unresolved,
    Settled,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct AttemptProcessScopeRecord {
    schema_version: u32,
    identity: AttemptProcessScopeIdentity,
    identity_digest: String,
    state: AttemptProcessScopeState,
    reason: Option<String>,
}

#[derive(Clone, Debug)]
pub(super) struct AttemptProcessScopeJournal {
    directory: PathBuf,
}

impl AttemptProcessScopeJournal {
    pub(super) fn open(directory: &Path) -> Result<Self, AttemptProcessScopeError> {
        fs::create_dir_all(directory)?;
        let metadata = fs::symlink_metadata(directory)?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(AttemptProcessScopeError::InvalidRecord(
                "journal directory is not a real directory".into(),
            ));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::{MetadataExt, PermissionsExt};
            if metadata.uid() != rustix::process::geteuid().as_raw() {
                return Err(AttemptProcessScopeError::InvalidRecord(
                    "journal directory owner mismatch".into(),
                ));
            }
            fs::set_permissions(directory, fs::Permissions::from_mode(0o700))?;
        }
        Ok(Self {
            directory: directory.to_path_buf(),
        })
    }

    fn record_path(&self, identity: &AttemptProcessScopeIdentity) -> PathBuf {
        self.directory.join(format!("{}.json", identity.digest()))
    }

    pub(super) fn record_active(
        &self,
        identity: &AttemptProcessScopeIdentity,
    ) -> Result<(), AttemptProcessScopeError> {
        self.write(identity, AttemptProcessScopeState::Active, None, true)
    }

    pub(super) fn record_unresolved(
        &self,
        identity: &AttemptProcessScopeIdentity,
        reason: &str,
    ) -> Result<(), AttemptProcessScopeError> {
        let safe_reason = if reason
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
        {
            reason
        } else {
            "QUIESCENCE_UNKNOWN"
        };
        self.write(
            identity,
            AttemptProcessScopeState::Unresolved,
            Some(safe_reason),
            false,
        )
    }

    fn write(
        &self,
        identity: &AttemptProcessScopeIdentity,
        state: AttemptProcessScopeState,
        reason: Option<&str>,
        create_new: bool,
    ) -> Result<(), AttemptProcessScopeError> {
        let path = self.record_path(identity);
        if create_new && path.exists() {
            return Err(AttemptProcessScopeError::DuplicateIdentity);
        }
        let record = AttemptProcessScopeRecord {
            schema_version: 1,
            identity: identity.clone(),
            identity_digest: identity.digest(),
            state,
            reason: reason.map(str::to_owned),
        };
        let mut bytes = serde_json::to_vec(&record).map_err(|_| {
            AttemptProcessScopeError::InvalidRecord("record serialization failed".into())
        })?;
        bytes.push(b'\n');
        // A unique, privately owned temporary file is cleaned up on every failure.
        // In particular, a duplicate record must not strand a predictable PID-based
        // .tmp path and block later writes after a retry or daemon restart.
        let mut file = tempfile::NamedTempFile::new_in(&self.directory)?;
        file.write_all(&bytes)?;
        file.as_file().sync_all()?;
        if create_new {
            file.persist_noclobber(&path).map_err(|error| {
                if error.error.kind() == std::io::ErrorKind::AlreadyExists {
                    AttemptProcessScopeError::DuplicateIdentity
                } else {
                    AttemptProcessScopeError::Io(error.error)
                }
            })?;
        } else {
            file.persist(&path)
                .map_err(|error| AttemptProcessScopeError::Io(error.error))?;
        }
        File::open(&self.directory)?.sync_all()?;
        Ok(())
    }

    pub(super) fn incomplete_records(
        &self,
    ) -> Result<Vec<AttemptProcessScopeIdentity>, AttemptProcessScopeError> {
        let mut entries = fs::read_dir(&self.directory)?.collect::<Result<Vec<_>, _>>()?;
        entries.sort_by_key(|entry| entry.file_name());
        let mut incomplete = Vec::new();
        for entry in entries {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if !name.ends_with(".json") {
                continue;
            }
            let metadata = fs::symlink_metadata(entry.path())?;
            if !metadata.is_file() || metadata.file_type().is_symlink() {
                return Err(AttemptProcessScopeError::InvalidRecord(
                    "journal entry is not a regular file".into(),
                ));
            }
            let mut file = File::open(entry.path())?;
            let mut bytes = Vec::new();
            file.take(16 * 1024).read_to_end(&mut bytes)?;
            let record: AttemptProcessScopeRecord =
                serde_json::from_slice(&bytes).map_err(|_| {
                    AttemptProcessScopeError::InvalidRecord(
                        "journal entry is unreadable or missing required identity".into(),
                    )
                })?;
            if record.schema_version != 1
                || record.identity_digest != record.identity.digest()
                || entry.path() != self.record_path(&record.identity)
            {
                return Err(AttemptProcessScopeError::InvalidRecord(
                    "journal identity or schema mismatch".into(),
                ));
            }
            if record.state != AttemptProcessScopeState::Settled {
                incomplete.push(record.identity);
            }
        }
        Ok(incomplete)
    }

    fn settle(
        &self,
        identity: &AttemptProcessScopeIdentity,
        proof: Option<&RuntimeQuiescenceProof>,
    ) -> Result<(), AttemptProcessScopeError> {
        let proof = proof.ok_or(AttemptProcessScopeError::QuiescenceProofRequired)?;
        if proof.identity_digest != identity.digest() {
            return Err(AttemptProcessScopeError::QuiescenceProofMismatch);
        }
        self.write(identity, AttemptProcessScopeState::Settled, None, false)
    }
}

/// Private proof token. No Operator, Agent, provider response, or journal record can
/// construct it; the only constructor follows successful cgroup observation and reaping.
struct RuntimeQuiescenceProof {
    identity_digest: String,
}

pub(super) struct ManagedAttemptScope {
    scope: ManagedScope,
    identity: AttemptProcessScopeIdentity,
    journal: AttemptProcessScopeJournal,
}

impl ManagedAttemptScope {
    pub(super) fn unit_name(&self) -> &str {
        self.scope.unit_name()
    }

    pub(super) fn wait_for_quiescence(&mut self, timeout: Duration) -> Result<(), ScopeError> {
        match self.scope.wait_for_quiescence(timeout) {
            Ok(_direct_child_status) => {
                let proof = RuntimeQuiescenceProof {
                    identity_digest: self.identity.digest(),
                };
                self.journal.settle(&self.identity, Some(&proof)).map_err(|error| ScopeError::Control(format!("quiescence proven but local Attempt identity could not be settled: {error:?}")))
            }
            Err(error) => {
                let _ = self
                    .journal
                    .record_unresolved(&self.identity, "QUIESCENCE_UNPROVEN");
                Err(error)
            }
        }
    }

    pub(super) fn kill_and_wait(&mut self, timeout: Duration) -> Result<(), ScopeError> {
        match self.scope.kill_and_wait(timeout) {
            Ok(_direct_child_status) => {
                let proof = RuntimeQuiescenceProof {
                    identity_digest: self.identity.digest(),
                };
                self.journal.settle(&self.identity, Some(&proof)).map_err(|error| ScopeError::Control(format!("quiescence proven but local Attempt identity could not be settled: {error:?}")))
            }
            Err(error) => {
                let _ = self
                    .journal
                    .record_unresolved(&self.identity, "QUIESCENCE_UNPROVEN");
                Err(error)
            }
        }
    }
}

pub(super) fn launch(
    journal: &AttemptProcessScopeJournal,
    identity: AttemptProcessScopeIdentity,
    commands: &SystemdCommands,
    mut spec: LaunchSpec,
) -> Result<ManagedAttemptScope, ScopeError> {
    spec.attempt_id = identity.supervisor_attempt_id();
    journal.record_active(&identity).map_err(|error| {
        ScopeError::Control(format!(
            "could not durably record Attempt process identity: {error:?}"
        ))
    })?;
    match spawn(commands, spec) {
        Ok(scope) => Ok(ManagedAttemptScope {
            scope,
            identity,
            journal: journal.clone(),
        }),
        Err(error) => {
            let _ = journal.record_unresolved(&identity, "SCOPE_LAUNCH_OR_CLEANUP_UNPROVEN");
            Err(error)
        }
    }
}
