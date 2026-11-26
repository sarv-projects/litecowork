//! Private, Runtime-local identity and recovery for a LiteCowork-owned agent process.
//!
//! A PID is only a lookup key. The persisted identity also binds the process start,
//! executable and owner evidence. It is never projected into the Operator API or an
//! event. Recovery may signal only an exact Linux process group while a pidfd pins the
//! leader's identity; Windows relies on the existing kill-on-close Job Object and blocks
//! if an exact owned child unexpectedly survives it.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::AgentChild;

const PROCESS_IDENTITY_PREFIX: &str = "lcproc:v1:";
const LEGACY_PID_PREFIX: &str = "pid:";
const PROCESS_STOP_GRACE: std::time::Duration = std::time::Duration::from_secs(2);
const PROCESS_KILL_GRACE: std::time::Duration = std::time::Duration::from_secs(2);

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "platform", deny_unknown_fields, rename_all = "snake_case")]
enum ProcessIdentity {
    Linux {
        version: u8,
        pid: u32,
        boot_id: String,
        start_ticks: u64,
        effective_uid: u32,
        executable_device: u64,
        executable_inode: u64,
        process_group: u32,
        session_id: u32,
    },
    Windows {
        version: u8,
        pid: u32,
        creation_filetime: u64,
        owner_sid_hex: String,
        executable_path_sha256: String,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProcessRecoveryError {
    InvalidIdentity,
    UnsupportedPlatform,
    ObservationFailed,
    StopUnconfirmed,
}

/// Result of preparing recovery before the caller commits STOPPING.
pub(crate) enum PreparedProcessRecovery {
    /// The exact old process and its owned scope are confirmed absent, or the PID now
    /// belongs to a different process start. No process was signalled in this case.
    Gone,
    /// A verified LiteCowork-owned process scope. The caller must commit STOPPING before
    /// calling `stop`; on Linux this value retains a pidfd until cleanup is complete.
    Stop(OwnedProcessScope),
    /// Missing/legacy evidence, a cross-platform identity, or uncertain ownership. Keep
    /// the host nonterminal and surface a Runtime blocker; never guess from a PID.
    Blocked,
}

pub(crate) struct OwnedProcessScope {
    #[cfg(target_os = "linux")]
    pidfd: std::os::fd::OwnedFd,
    #[cfg(target_os = "linux")]
    process_group: u32,
    #[cfg(target_os = "linux")]
    session_id: u32,
    #[cfg(target_os = "linux")]
    effective_uid: u32,
}

impl OwnedProcessScope {
    pub(crate) fn stop(self) -> Result<(), ProcessRecoveryError> {
        #[cfg(target_os = "linux")]
        {
            // Keeping pidfd alive pins the original process identity and prevents a
            // reused numeric PID from becoming the target while the process group exits.
            let _pidfd = self.pidfd;
            signal_group(self.process_group, rustix::process::Signal::TERM)?;
            if wait_group_empty(
                self.process_group,
                self.session_id,
                self.effective_uid,
                PROCESS_STOP_GRACE,
            )? {
                return Ok(());
            }
            signal_group(self.process_group, rustix::process::Signal::KILL)?;
            if wait_group_empty(
                self.process_group,
                self.session_id,
                self.effective_uid,
                PROCESS_KILL_GRACE,
            )? {
                Ok(())
            } else {
                Err(ProcessRecoveryError::StopUnconfirmed)
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = self;
            Err(ProcessRecoveryError::UnsupportedPlatform)
        }
    }
}

/// Capture identity while the launched AgentChild still owns its native wait handle.
pub(crate) fn capture(child: &AgentChild) -> Result<String, ProcessRecoveryError> {
    #[cfg(target_os = "linux")]
    {
        let pid = child.id();
        let identity = capture_linux(pid)?;
        return encode(&identity);
    }
    #[cfg(windows)]
    {
        let details = child
            .windows_process_details()
            .map_err(|_| ProcessRecoveryError::ObservationFailed)?;
        let identity = ProcessIdentity::Windows {
            version: 1,
            pid: details.process_id,
            creation_filetime: details.creation_filetime,
            owner_sid_hex: hex::encode(details.owner_sid),
            executable_path_sha256: sha256(&details.executable_path.to_string_lossy()),
        };
        return encode(&identity);
    }
    #[cfg(not(any(target_os = "linux", windows)))]
    {
        let _ = child;
        Err(ProcessRecoveryError::UnsupportedPlatform)
    }
}

pub(crate) fn prepare_recovery(
    process_identity_ref: Option<&str>,
) -> Result<PreparedProcessRecovery, ProcessRecoveryError> {
    let Some(value) = process_identity_ref else {
        return Ok(PreparedProcessRecovery::Blocked);
    };
    if value.len() > 512 || value.chars().any(char::is_control) {
        return Ok(PreparedProcessRecovery::Blocked);
    }
    if let Some(identity_json) = value.strip_prefix(PROCESS_IDENTITY_PREFIX) {
        let identity: ProcessIdentity = serde_json::from_str(identity_json)
            .map_err(|_| ProcessRecoveryError::InvalidIdentity)?;
        return prepare_versioned_recovery(identity);
    }
    if let Some(pid_text) = value.strip_prefix(LEGACY_PID_PREFIX) {
        let pid = pid_text
            .parse::<u32>()
            .ok()
            .filter(|pid| *pid > 0)
            .ok_or(ProcessRecoveryError::InvalidIdentity)?;
        return prepare_legacy_recovery(pid);
    }
    Ok(PreparedProcessRecovery::Blocked)
}

fn encode(identity: &ProcessIdentity) -> Result<String, ProcessRecoveryError> {
    validate_identity(identity)?;
    let body =
        serde_json::to_string(identity).map_err(|_| ProcessRecoveryError::InvalidIdentity)?;
    let value = format!("{PROCESS_IDENTITY_PREFIX}{body}");
    if value.len() > 512 || value.chars().any(char::is_control) {
        return Err(ProcessRecoveryError::InvalidIdentity);
    }
    Ok(value)
}

fn validate_identity(identity: &ProcessIdentity) -> Result<(), ProcessRecoveryError> {
    let valid = match identity {
        ProcessIdentity::Linux {
            version,
            pid,
            boot_id,
            start_ticks,
            executable_inode,
            process_group,
            session_id,
            ..
        } => {
            *version == 1
                && *pid > 0
                && *process_group == *pid
                && *session_id > 0
                && *start_ticks > 0
                && *executable_inode > 0
                && boot_id.len() == 36
                && boot_id
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit() || byte == b'-')
        }
        ProcessIdentity::Windows {
            version,
            pid,
            creation_filetime,
            owner_sid_hex,
            executable_path_sha256,
        } => {
            *version == 1
                && *pid > 0
                && *creation_filetime > 0
                && !owner_sid_hex.is_empty()
                && owner_sid_hex.len() <= 160
                && owner_sid_hex.bytes().all(|byte| byte.is_ascii_hexdigit())
                && executable_path_sha256.len() == 64
                && executable_path_sha256
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit())
        }
    };
    valid
        .then_some(())
        .ok_or(ProcessRecoveryError::InvalidIdentity)
}

fn prepare_versioned_recovery(
    identity: ProcessIdentity,
) -> Result<PreparedProcessRecovery, ProcessRecoveryError> {
    validate_identity(&identity)?;
    #[cfg(target_os = "linux")]
    if let ProcessIdentity::Linux {
        pid,
        boot_id,
        start_ticks,
        effective_uid,
        executable_device,
        executable_inode,
        process_group,
        session_id,
        ..
    } = identity
    {
        return prepare_linux_recovery(
            pid,
            &boot_id,
            start_ticks,
            effective_uid,
            executable_device,
            executable_inode,
            process_group,
            session_id,
        );
    }
    #[cfg(windows)]
    if let ProcessIdentity::Windows {
        pid,
        creation_filetime,
        owner_sid_hex,
        executable_path_sha256,
        ..
    } = identity
    {
        return prepare_windows_recovery(
            pid,
            creation_filetime,
            &owner_sid_hex,
            &executable_path_sha256,
        );
    }
    let _ = identity;
    Ok(PreparedProcessRecovery::Blocked)
}

#[cfg(target_os = "linux")]
fn capture_linux(pid: u32) -> Result<ProcessIdentity, ProcessRecoveryError> {
    use rustix::process::{Pid, PidfdFlags, pidfd_open};
    use std::os::unix::fs::MetadataExt;

    let pid = Pid::from_raw(pid as rustix::process::RawPid)
        .ok_or(ProcessRecoveryError::InvalidIdentity)?;
    let _pidfd = pidfd_open(pid, PidfdFlags::empty())
        .map_err(|_| ProcessRecoveryError::ObservationFailed)?;
    let pid = pid.as_raw_nonzero().get() as u32;
    let process = read_linux_process(pid).map_err(|_| ProcessRecoveryError::ObservationFailed)?;
    if process.process_group != pid {
        return Err(ProcessRecoveryError::ObservationFailed);
    }
    let boot_id = std::fs::read_to_string("/proc/sys/kernel/random/boot_id")
        .map_err(|_| ProcessRecoveryError::ObservationFailed)?
        .trim()
        .to_ascii_lowercase();
    let executable = std::fs::metadata(format!("/proc/{pid}/exe"))
        .map_err(|_| ProcessRecoveryError::ObservationFailed)?;
    Ok(ProcessIdentity::Linux {
        version: 1,
        pid,
        boot_id,
        start_ticks: process.start_ticks,
        effective_uid: process.effective_uid,
        executable_device: executable.dev(),
        executable_inode: executable.ino(),
        process_group: process.process_group,
        session_id: process.session_id,
    })
}

#[cfg(target_os = "linux")]
fn prepare_linux_recovery(
    pid: u32,
    expected_boot_id: &str,
    expected_start_ticks: u64,
    expected_effective_uid: u32,
    expected_executable_device: u64,
    expected_executable_inode: u64,
    process_group: u32,
    session_id: u32,
) -> Result<PreparedProcessRecovery, ProcessRecoveryError> {
    use rustix::process::{Pid, PidfdFlags, pidfd_open};
    use std::{os::fd::OwnedFd, os::unix::fs::MetadataExt};

    let pid_value = Pid::from_raw(pid as rustix::process::RawPid)
        .ok_or(ProcessRecoveryError::InvalidIdentity)?;
    let pidfd: OwnedFd = match pidfd_open(pid_value, PidfdFlags::empty()) {
        Ok(pidfd) => pidfd,
        Err(error) if error == rustix::io::Errno::NOENT => {
            return if group_has_only_expected_owner(
                process_group,
                session_id,
                expected_effective_uid,
            )? {
                Ok(PreparedProcessRecovery::Blocked)
            } else {
                Ok(PreparedProcessRecovery::Gone)
            };
        }
        Err(_) => return Ok(PreparedProcessRecovery::Blocked),
    };
    let current = match read_linux_process(pid) {
        Ok(current) => current,
        Err(_) => return Ok(PreparedProcessRecovery::Blocked),
    };
    let current_boot_id = std::fs::read_to_string("/proc/sys/kernel/random/boot_id")
        .map_err(|_| ProcessRecoveryError::ObservationFailed)?
        .trim()
        .to_ascii_lowercase();
    let current_executable = std::fs::metadata(format!("/proc/{pid}/exe"))
        .map_err(|_| ProcessRecoveryError::ObservationFailed)?;
    let start_matches =
        current_boot_id == expected_boot_id && current.start_ticks == expected_start_ticks;
    if !start_matches {
        return if group_has_only_expected_owner(process_group, session_id, expected_effective_uid)?
        {
            Ok(PreparedProcessRecovery::Blocked)
        } else {
            Ok(PreparedProcessRecovery::Gone)
        };
    }
    if current.effective_uid != expected_effective_uid
        || current.process_group != process_group
        || current.session_id != session_id
        || current_executable.dev() != expected_executable_device
        || current_executable.ino() != expected_executable_inode
    {
        return Ok(PreparedProcessRecovery::Blocked);
    }
    Ok(PreparedProcessRecovery::Stop(OwnedProcessScope {
        pidfd,
        process_group,
        session_id,
        effective_uid: expected_effective_uid,
    }))
}

#[cfg(windows)]
fn prepare_windows_recovery(
    pid: u32,
    expected_creation_filetime: u64,
    expected_owner_sid_hex: &str,
    expected_executable_path_sha256: &str,
) -> Result<PreparedProcessRecovery, ProcessRecoveryError> {
    let details = match windows_local_security::agent_process_details(pid) {
        Ok(Some(details)) => details,
        Ok(None) => return Ok(PreparedProcessRecovery::Gone),
        Err(_) => return Ok(PreparedProcessRecovery::Blocked),
    };
    if details.creation_filetime != expected_creation_filetime {
        return Ok(PreparedProcessRecovery::Gone);
    }
    if hex::encode(details.owner_sid) != expected_owner_sid_hex
        || sha256(&details.executable_path.to_string_lossy()) != expected_executable_path_sha256
    {
        return Ok(PreparedProcessRecovery::Blocked);
    }
    // The original per-session Job Object has KILL_ON_JOB_CLOSE. A matching process
    // after a new Runtime starts is therefore outside the expected cleanup result; do
    // not weaken process-tree ownership by terminating only its leader.
    Ok(PreparedProcessRecovery::Blocked)
}

#[cfg(target_os = "linux")]
fn prepare_legacy_recovery(pid: u32) -> Result<PreparedProcessRecovery, ProcessRecoveryError> {
    // Legacy records do not identify a process start or scope. A reused PID may now
    // be in another process group, so checking only the old expected PGID could
    // incorrectly report the old host as stopped while the PID is still live.
    if process_exists(pid)? || group_has_any_member(pid)? {
        Ok(PreparedProcessRecovery::Blocked)
    } else {
        Ok(PreparedProcessRecovery::Gone)
    }
}

#[cfg(windows)]
fn prepare_legacy_recovery(pid: u32) -> Result<PreparedProcessRecovery, ProcessRecoveryError> {
    match windows_local_security::agent_process_details(pid) {
        Ok(None) => Ok(PreparedProcessRecovery::Gone),
        Ok(Some(_)) | Err(_) => Ok(PreparedProcessRecovery::Blocked),
    }
}

#[cfg(not(any(target_os = "linux", windows)))]
fn prepare_legacy_recovery(_pid: u32) -> Result<PreparedProcessRecovery, ProcessRecoveryError> {
    Ok(PreparedProcessRecovery::Blocked)
}

#[cfg(target_os = "linux")]
#[derive(Clone, Copy)]
struct LinuxProcessObservation {
    state: char,
    start_ticks: u64,
    effective_uid: u32,
    process_group: u32,
    session_id: u32,
}

#[cfg(target_os = "linux")]
fn read_linux_process(pid: u32) -> Result<LinuxProcessObservation, ()> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).map_err(|_| ())?;
    let close = stat.rfind(')').ok_or(())?;
    let fields = stat
        .get(close + 1..)
        .ok_or(())?
        .split_whitespace()
        .collect::<Vec<_>>();
    let state = fields
        .first()
        .and_then(|state| state.chars().next())
        .ok_or(())?;
    // Fields after comm start at field 3 (state): pgrp is field 5, session field 6,
    // and starttime field 22 (index 19 in this tail).
    let process_group = fields.get(2).ok_or(())?.parse().map_err(|_| ())?;
    let session_id = fields.get(3).ok_or(())?.parse().map_err(|_| ())?;
    let start_ticks = fields.get(19).ok_or(())?.parse().map_err(|_| ())?;
    let status = std::fs::read_to_string(format!("/proc/{pid}/status")).map_err(|_| ())?;
    let uid_line = status
        .lines()
        .find(|line| line.starts_with("Uid:"))
        .ok_or(())?;
    let effective_uid = uid_line
        .split_whitespace()
        .nth(2)
        .ok_or(())?
        .parse()
        .map_err(|_| ())?;
    Ok(LinuxProcessObservation {
        state,
        start_ticks,
        effective_uid,
        process_group,
        session_id,
    })
}

#[cfg(target_os = "linux")]
fn process_exists(pid: u32) -> Result<bool, ProcessRecoveryError> {
    match std::fs::read_to_string(format!("/proc/{pid}/stat")) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(_) => Err(ProcessRecoveryError::ObservationFailed),
    }
}

#[cfg(target_os = "linux")]
fn group_has_any_member(process_group: u32) -> Result<bool, ProcessRecoveryError> {
    group_members(process_group).map(|members| !members.is_empty())
}

#[cfg(target_os = "linux")]
fn group_has_only_expected_owner(
    process_group: u32,
    session_id: u32,
    effective_uid: u32,
) -> Result<bool, ProcessRecoveryError> {
    let members = group_members(process_group)?;
    if members.is_empty() {
        return Ok(false);
    }
    for pid in members {
        let process =
            read_linux_process(pid).map_err(|_| ProcessRecoveryError::ObservationFailed)?;
        if process.session_id != session_id || process.effective_uid != effective_uid {
            return Err(ProcessRecoveryError::ObservationFailed);
        }
    }
    Ok(true)
}

#[cfg(target_os = "linux")]
fn group_members(process_group: u32) -> Result<Vec<u32>, ProcessRecoveryError> {
    let entries =
        std::fs::read_dir("/proc").map_err(|_| ProcessRecoveryError::ObservationFailed)?;
    let mut members = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|_| ProcessRecoveryError::ObservationFailed)?;
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let Ok(pid) = name.parse::<u32>() else {
            continue;
        };
        match read_linux_process(pid) {
            Ok(process)
                if process.process_group == process_group
                    && !matches!(process.state, 'Z' | 'X' | 'x') =>
            {
                members.push(pid)
            }
            Ok(_) => {}
            Err(()) if !entry.path().exists() => {}
            Err(()) => return Err(ProcessRecoveryError::ObservationFailed),
        }
    }
    Ok(members)
}

#[cfg(target_os = "linux")]
fn wait_group_empty(
    process_group: u32,
    session_id: u32,
    effective_uid: u32,
    timeout: std::time::Duration,
) -> Result<bool, ProcessRecoveryError> {
    let deadline = std::time::Instant::now()
        .checked_add(timeout)
        .ok_or(ProcessRecoveryError::ObservationFailed)?;
    loop {
        let members = group_members(process_group)?;
        if members.is_empty() {
            return Ok(true);
        }
        for pid in members {
            let process =
                read_linux_process(pid).map_err(|_| ProcessRecoveryError::ObservationFailed)?;
            if process.session_id != session_id || process.effective_uid != effective_uid {
                return Err(ProcessRecoveryError::ObservationFailed);
            }
        }
        if std::time::Instant::now() >= deadline {
            return Ok(false);
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

#[cfg(target_os = "linux")]
fn signal_group(
    process_group: u32,
    signal: rustix::process::Signal,
) -> Result<(), ProcessRecoveryError> {
    let process_group = rustix::process::Pid::from_raw(process_group as rustix::process::RawPid)
        .ok_or(ProcessRecoveryError::InvalidIdentity)?;
    rustix::process::kill_process_group(process_group, signal)
        .map_err(|_| ProcessRecoveryError::StopUnconfirmed)
}

fn sha256(value: &str) -> String {
    hex::encode(Sha256::digest(value.to_lowercase().as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::{
        PROCESS_IDENTITY_PREFIX, PreparedProcessRecovery, ProcessIdentity, encode,
        prepare_recovery, validate_identity,
    };

    fn linux_identity() -> ProcessIdentity {
        ProcessIdentity::Linux {
            version: 1,
            pid: 321,
            boot_id: "01234567-89ab-cdef-0123-456789abcdef".to_owned(),
            start_ticks: 98_765,
            effective_uid: 1000,
            executable_device: 2,
            executable_inode: 3,
            process_group: 321,
            session_id: 20,
        }
    }

    #[test]
    fn process_identity_is_versioned_opaque_and_round_trips() {
        let identity = linux_identity();
        let encoded = encode(&identity).expect("valid process identity encodes");
        assert!(encoded.starts_with(PROCESS_IDENTITY_PREFIX));
        assert!(encoded.len() <= 512);
        let decoded: ProcessIdentity = serde_json::from_str(
            encoded
                .strip_prefix(PROCESS_IDENTITY_PREFIX)
                .expect("versioned prefix"),
        )
        .expect("versioned process identity parses");
        assert_eq!(decoded, identity);
    }

    #[test]
    fn pid_only_identity_is_never_accepted_as_process_authority() {
        let result = prepare_recovery(Some("pid:321")).expect("legacy identity is classified");
        #[cfg(target_os = "linux")]
        assert!(matches!(
            result,
            PreparedProcessRecovery::Gone | PreparedProcessRecovery::Blocked
        ));
        #[cfg(windows)]
        assert!(matches!(
            result,
            PreparedProcessRecovery::Gone | PreparedProcessRecovery::Blocked
        ));
        #[cfg(not(any(target_os = "linux", windows)))]
        assert!(matches!(result, PreparedProcessRecovery::Blocked));
    }

    #[test]
    fn process_identity_rejects_invalid_start_scope_and_unknown_fields() {
        let mut identity = linux_identity();
        if let ProcessIdentity::Linux { process_group, .. } = &mut identity {
            *process_group = 320;
        }
        assert!(validate_identity(&identity).is_err());
        let unknown = format!(
            "{PROCESS_IDENTITY_PREFIX}{{\"platform\":\"linux\",\"version\":1,\"pid\":321,\"boot_id\":\"01234567-89ab-cdef-0123-456789abcdef\",\"start_ticks\":98765,\"effective_uid\":1000,\"executable_device\":2,\"executable_inode\":3,\"process_group\":321,\"session_id\":20,\"native_handle\":\"secret\"}}"
        );
        assert!(prepare_recovery(Some(&unknown)).is_err());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn legacy_identity_with_a_live_reused_pid_fails_closed() {
        use super::process_exists;

        assert!(process_exists(std::process::id()).expect("current process is observable"));
        assert!(!process_exists(u32::MAX).expect("missing PID is observable"));
        assert!(matches!(
            prepare_recovery(Some(&format!("pid:{}", std::process::id())))
                .expect("legacy identity is classified"),
            PreparedProcessRecovery::Blocked
        ));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn exact_process_identity_stops_and_confirms_its_owned_group() {
        use super::{AgentChild, PreparedProcessRecovery, capture};
        use std::process::{Command, Stdio};

        let mut command = Command::new("/bin/sleep");
        command
            .arg("30")
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let mut child = AgentChild::spawn(command).expect("spawn an isolated process-group child");
        child.resume().expect("resume child");

        let identity = capture(&child).expect("capture the spawned child's identity");
        let recovery = prepare_recovery(Some(&identity)).expect("prepare exact recovery");
        let PreparedProcessRecovery::Stop(scope) = recovery else {
            panic!("exact live identity must produce a stoppable owned scope");
        };
        scope
            .stop()
            .expect("terminate and observe the exact process group");
        let status = child.wait().expect("reap the stopped child");
        assert!(!status.success(), "SIGTERM should not look like success");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn mismatched_process_start_never_signals_the_live_pid() {
        use super::{AgentChild, ProcessIdentity, capture};
        use std::process::{Command, Stdio};

        let mut command = Command::new("/bin/sleep");
        command
            .arg("30")
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let mut child = AgentChild::spawn(command).expect("spawn an isolated process-group child");
        child.resume().expect("resume child");

        let identity = capture(&child).expect("capture the spawned child's identity");
        let encoded = identity
            .strip_prefix(PROCESS_IDENTITY_PREFIX)
            .expect("versioned process identity");
        let mut identity: ProcessIdentity =
            serde_json::from_str(encoded).expect("decode captured identity");
        if let ProcessIdentity::Linux { start_ticks, .. } = &mut identity {
            *start_ticks = start_ticks.saturating_add(1);
        }
        let mismatched = encode(&identity).expect("encode a well-formed stale identity");

        assert!(matches!(
            prepare_recovery(Some(&mismatched)).expect("classify stale identity"),
            super::PreparedProcessRecovery::Blocked
        ));
        assert!(child.try_wait().expect("observe child").is_none());
        child.kill_tree().expect("clean up test child");
        let _ = child.wait().expect("reap test child");
    }
}
