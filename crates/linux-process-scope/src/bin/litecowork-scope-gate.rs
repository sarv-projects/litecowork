#[cfg(target_os = "linux")]
fn main() {
    use std::{
        env,
        ffi::OsStr,
        io::{self, Read, Write},
        os::unix::process::CommandExt,
        process::Command,
    };

    const READY: &[u8] = b"LITECOWORK_SCOPE_GATE_V1\n";
    const GO_CONSUMED: &[u8] = b"LITECOWORK_SCOPE_GO_CONSUMED_V1\n";
    const GO: &[u8] = b"GO\n";

    let mut arguments = env::args_os().skip(1);
    let Some(first) = arguments.next() else {
        eprintln!("LiteCowork scope gate: executable missing");
        std::process::exit(126);
    };

    // This mode is the Bubblewrap command itself. It intentionally does not use the
    // outer systemd scope gate handshake; the outer invocation still does.
    if first == OsStr::new("--attempt-runner") {
        std::process::exit(attempt_runner(arguments, false));
    }
    if first == OsStr::new("--held-attempt-runner") {
        std::process::exit(attempt_runner(arguments, true));
    }

    if !std::path::Path::new(&first).is_absolute() {
        eprintln!("LiteCowork scope gate: executable must be absolute");
        std::process::exit(126);
    }

    if io::stdout()
        .write_all(READY)
        .and_then(|_| io::stdout().flush())
        .is_err()
    {
        std::process::exit(126);
    }
    let mut go = [0; GO.len()];
    if io::stdin().read_exact(&mut go).is_err() || go != *GO {
        eprintln!("LiteCowork scope gate: GO handshake missing or invalid");
        std::process::exit(126);
    }
    // This marker proves only that the gate consumed exactly GO. It is emitted before
    // attempting exec; the parent waits for it before exposing stdin to the caller, so
    // agent protocol bytes cannot be prefetched by the gate's handshake read.
    if io::stdout()
        .write_all(GO_CONSUMED)
        .and_then(|_| io::stdout().flush())
        .is_err()
    {
        std::process::exit(126);
    }

    let error = Command::new(first).args(arguments).exec();
    eprintln!("LiteCowork scope gate: agent exec failed: {error}");
    std::process::exit(126);
}

#[cfg(target_os = "linux")]
fn attempt_runner(
    arguments: impl Iterator<Item = std::ffi::OsString>,
    wait_for_admission: bool,
) -> i32 {
    use linux_process_scope::output_channel::{CandidateRejection, MAX_CANDIDATE_BYTES};
    use rustix::process::{DumpableBehavior, Pid, set_child_subreaper, set_dumpable_behavior};
    use std::{
        ffi::OsStr,
        process::{Command, Stdio},
        thread,
    };

    let mut arguments = arguments;
    let Some(limit_arg) = arguments.next() else {
        return write_terminal_rejection(CandidateRejection::ReadFailure)
            .map(|()| 126)
            .unwrap_or(126);
    };
    let Some(limit_text) = limit_arg.to_str() else {
        return write_terminal_rejection(CandidateRejection::ReadFailure)
            .map(|()| 126)
            .unwrap_or(126);
    };
    if limit_text.is_empty() || !limit_text.bytes().all(|byte| byte.is_ascii_digit()) {
        return write_terminal_rejection(CandidateRejection::ReadFailure)
            .map(|()| 126)
            .unwrap_or(126);
    }
    let Ok(candidate_limit) = limit_text.parse::<usize>() else {
        return write_terminal_rejection(CandidateRejection::ReadFailure)
            .map(|()| 126)
            .unwrap_or(126);
    };
    if candidate_limit == 0 || candidate_limit > MAX_CANDIDATE_BYTES {
        return write_terminal_rejection(CandidateRejection::ReadFailure)
            .map(|()| 126)
            .unwrap_or(126);
    }
    let Some(workspace_limit_arg) = arguments.next() else {
        return write_terminal_rejection(CandidateRejection::ReadFailure)
            .map(|()| 126)
            .unwrap_or(126);
    };
    let Some(workspace_limit_text) = workspace_limit_arg.to_str() else {
        return write_terminal_rejection(CandidateRejection::ReadFailure)
            .map(|()| 126)
            .unwrap_or(126);
    };
    if workspace_limit_text.is_empty()
        || !workspace_limit_text
            .bytes()
            .all(|byte| byte.is_ascii_digit())
    {
        return write_terminal_rejection(CandidateRejection::ReadFailure)
            .map(|()| 126)
            .unwrap_or(126);
    }
    let Ok(workspace_limit_bytes) = workspace_limit_text.parse::<u64>() else {
        return write_terminal_rejection(CandidateRejection::ReadFailure)
            .map(|()| 126)
            .unwrap_or(126);
    };
    if workspace_limit_bytes == 0
        || workspace_limit_bytes > linux_process_scope::MAX_SCRATCH_BYTES
        || linux_process_scope::environment_probe::verify_workspace_tmpfs(workspace_limit_bytes)
            .is_err()
    {
        return write_terminal_rejection(CandidateRejection::ReadFailure)
            .map(|()| 126)
            .unwrap_or(126);
    }
    if arguments.next().as_deref() != Some(OsStr::new("--")) {
        return write_terminal_rejection(CandidateRejection::ReadFailure)
            .map(|()| 126)
            .unwrap_or(126);
    }
    let Some(executable) = arguments.next() else {
        return write_terminal_rejection(CandidateRejection::ReadFailure)
            .map(|()| 126)
            .unwrap_or(126);
    };
    if !std::path::Path::new(&executable).is_absolute() || std::process::id() != 1 {
        return write_terminal_rejection(CandidateRejection::ReadFailure)
            .map(|()| 126)
            .unwrap_or(126);
    }

    // The harness must not open PID 1's stderr descriptor to forge a terminal frame.
    // Keep this trusted process non-dumpable before creating any untrusted child.
    if set_dumpable_behavior(DumpableBehavior::NotDumpable).is_err()
        || set_child_subreaper(Some(Pid::INIT)).is_err()
    {
        return write_terminal_rejection(CandidateRejection::ReadFailure)
            .map(|()| 126)
            .unwrap_or(126);
    }

    if wait_for_admission
        && linux_process_scope::admission_protocol::wait_for_admission(
            &mut std::io::stdin().lock(),
            &mut std::io::stdout().lock(),
        )
        .is_err()
    {
        let _ = write_terminal_rejection(CandidateRejection::ReadFailure);
        return 126;
    }

    let mut command = Command::new(&executable);
    command
        .args(arguments)
        .stdin(Stdio::inherit())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(_) => {
            let _ = write_terminal_rejection(CandidateRejection::ReadFailure);
            return 126;
        }
    };
    let Some(child_stdout) = child.stdout.take() else {
        let _ = write_terminal_rejection(CandidateRejection::ReadFailure);
        return 126;
    };
    let Some(child_stderr) = child.stderr.take() else {
        let _ = write_terminal_rejection(CandidateRejection::ReadFailure);
        return 126;
    };

    // Preserve the native harness stdout protocol byte-for-byte. Log frames travel
    // separately on scope stderr, and the untrusted harness never owns that pipe.
    let stdout_task = thread::spawn(move || forward_stdout(child_stdout));
    let stderr_task = thread::spawn(move || frame_child_stderr(child_stderr));
    let status = match child.wait() {
        Ok(status) => status,
        Err(_) => {
            let _ = write_terminal_rejection(CandidateRejection::ReadFailure);
            return 126;
        }
    };

    // PID 1 and the explicit subreaper collect orphaned descendants. The outer Runtime
    // still requires cgroup populated=0 before it may expose candidate bytes.
    if reap_attempt_descendants().is_err() {
        let _ = write_terminal_rejection(CandidateRejection::ReadFailure);
        return 126;
    }
    let stdout_ok = stdout_task.join().is_ok_and(|result| result.is_ok());
    let stderr_ok = stderr_task.join().is_ok_and(|result| result.is_ok());
    if !stdout_ok || !stderr_ok {
        let _ = write_terminal_rejection(CandidateRejection::ReadFailure);
        return 126;
    }

    if !status.success() {
        return match write_terminal_no_candidate() {
            Ok(()) => exit_code(status),
            Err(_) => 126,
        };
    }

    match read_attempt_proposal(candidate_limit) {
        Ok(Some(bytes)) => match write_terminal_candidate(&bytes) {
            Ok(()) => 0,
            Err(_) => 126,
        },
        Ok(None) => match write_terminal_no_candidate() {
            Ok(()) => 0,
            Err(_) => 126,
        },
        Err(rejection) => {
            let _ = write_terminal_rejection(rejection);
            126
        }
    }
}

#[cfg(target_os = "linux")]
fn forward_stdout(mut source: impl std::io::Read) -> std::io::Result<()> {
    use std::io::{self, Write};

    let mut destination = io::stdout().lock();
    let mut buffer = [0u8; 8192];
    let mut write_error = None;
    loop {
        let count = match source.read(&mut buffer) {
            Ok(0) => break,
            Ok(count) => count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        };
        if write_error.is_none()
            && let Err(error) = destination.write_all(&buffer[..count])
        {
            write_error = Some(error);
        }
    }
    if let Some(error) = write_error {
        return Err(error);
    }
    destination.flush()
}

#[cfg(target_os = "linux")]
fn frame_child_stderr(mut source: impl std::io::Read) -> std::io::Result<()> {
    use linux_process_scope::output_channel::{
        MAX_LOG_FRAME_BYTES, MAX_TOTAL_LOG_BYTES, write_log_frame,
    };
    use std::io::{self, Write};

    let mut destination = io::stderr().lock();
    let mut buffer = [0u8; MAX_LOG_FRAME_BYTES];
    let mut remaining = MAX_TOTAL_LOG_BYTES;
    let mut write_error = None;
    loop {
        let count = match source.read(&mut buffer) {
            Ok(0) => break,
            Ok(count) => count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        };
        let forward = count.min(remaining);
        if forward > 0 {
            if write_error.is_none()
                && let Err(error) = write_log_frame(&mut destination, &buffer[..forward])
            {
                write_error = Some(error);
            }
            remaining -= forward;
        }
        // After the log ceiling, continue draining and discard the rest so the
        // native harness cannot block on a full stderr pipe.
    }
    if let Some(error) = write_error {
        return Err(error);
    }
    destination.flush()
}

#[cfg(target_os = "linux")]
fn reap_attempt_descendants() -> Result<(), rustix::io::Errno> {
    use rustix::io::Errno;
    use rustix::process::{WaitOptions, wait};

    loop {
        match wait(WaitOptions::empty()) {
            Ok(Some(_)) => {}
            Ok(None) => continue,
            Err(error) if error == Errno::INTR => continue,
            Err(error) if error == Errno::CHILD => return Ok(()),
            Err(error) => return Err(error),
        }
    }
}

#[cfg(target_os = "linux")]
fn read_attempt_proposal(
    candidate_limit: usize,
) -> Result<Option<Vec<u8>>, linux_process_scope::output_channel::CandidateRejection> {
    use linux_process_scope::output_channel::CandidateRejection;
    use rustix::fs::{Mode, OFlags, open, openat};
    use std::fs::File;

    let root = open(
        "/",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map(File::from)
    .map_err(|_| CandidateRejection::InvalidCandidate)?;
    let workspace = openat(
        &root,
        "workspace",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map(File::from)
    .map_err(|_| CandidateRejection::InvalidCandidate)?;
    read_attempt_proposal_from_workspace(&workspace, candidate_limit)
}

#[cfg(target_os = "linux")]
fn read_attempt_proposal_from_workspace(
    workspace: &std::fs::File,
    candidate_limit: usize,
) -> Result<Option<Vec<u8>>, linux_process_scope::output_channel::CandidateRejection> {
    use linux_process_scope::output_channel::CandidateRejection;
    use rustix::{
        fs::{Mode, OFlags, openat},
        io::Errno,
    };
    use std::{fs::File, io::Read, os::unix::fs::MetadataExt};

    let outputs = openat(
        &workspace,
        "outputs",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map(File::from)
    .map_err(|_| CandidateRejection::InvalidCandidate)?;
    let proposal = match openat(
        &outputs,
        "proposal",
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(file) => File::from(file),
        Err(error) if error == Errno::NOENT => return Ok(None),
        Err(_) => return Err(CandidateRejection::InvalidCandidate),
    };
    let metadata = proposal
        .metadata()
        .map_err(|_| CandidateRejection::ReadFailure)?;
    if !metadata.is_file() || metadata.nlink() != 1 {
        return Err(CandidateRejection::InvalidCandidate);
    }
    if metadata.len() > candidate_limit as u64 {
        return Err(CandidateRejection::TooLarge);
    }

    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    let mut bounded_reader = proposal.take(candidate_limit as u64 + 1);
    bounded_reader
        .read_to_end(&mut bytes)
        .map_err(|_| CandidateRejection::ReadFailure)?;
    if bytes.len() > candidate_limit {
        return Err(CandidateRejection::TooLarge);
    }
    Ok(Some(bytes))
}

#[cfg(target_os = "linux")]
fn write_terminal_candidate(bytes: &[u8]) -> std::io::Result<()> {
    use linux_process_scope::output_channel::write_candidate_frame;
    use std::io::{self, Write};

    let mut stderr = io::stderr().lock();
    write_candidate_frame(&mut stderr, bytes)?;
    stderr.flush()
}

#[cfg(target_os = "linux")]
fn write_terminal_no_candidate() -> std::io::Result<()> {
    use linux_process_scope::output_channel::write_no_candidate_frame;
    use std::io::{self, Write};

    let mut stderr = io::stderr().lock();
    write_no_candidate_frame(&mut stderr)?;
    stderr.flush()
}

#[cfg(target_os = "linux")]
fn write_terminal_rejection(
    rejection: linux_process_scope::output_channel::CandidateRejection,
) -> std::io::Result<()> {
    use linux_process_scope::output_channel::write_rejected_frame;
    use std::io::{self, Write};

    let mut stderr = io::stderr().lock();
    write_rejected_frame(&mut stderr, rejection)?;
    stderr.flush()
}

#[cfg(target_os = "linux")]
fn exit_code(status: std::process::ExitStatus) -> i32 {
    use std::os::unix::process::ExitStatusExt;
    status
        .code()
        .unwrap_or_else(|| 128 + status.signal().unwrap_or(0))
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use linux_process_scope::output_channel::CandidateRejection;
    use std::{fs, fs::File, os::unix::fs::symlink};

    fn workspace(root: &std::path::Path) -> File {
        let workspace = root.join("workspace");
        fs::create_dir(&workspace).unwrap();
        fs::create_dir(workspace.join("outputs")).unwrap();
        File::open(workspace).unwrap()
    }

    #[test]
    fn reads_only_the_bounded_regular_proposal_leaf() {
        let temp = tempfile::tempdir().unwrap();
        let workspace = workspace(temp.path());
        fs::write(
            temp.path().join("workspace/outputs/proposal"),
            b"proposed bytes",
        )
        .unwrap();

        assert_eq!(
            read_attempt_proposal_from_workspace(&workspace, 32).unwrap(),
            Some(b"proposed bytes".to_vec())
        );
        assert_eq!(
            read_attempt_proposal_from_workspace(&workspace, 4),
            Err(CandidateRejection::TooLarge)
        );
    }

    #[test]
    fn missing_proposal_is_explicitly_absent() {
        let temp = tempfile::tempdir().unwrap();
        let workspace = workspace(temp.path());
        assert_eq!(
            read_attempt_proposal_from_workspace(&workspace, 32).unwrap(),
            None
        );
    }

    #[test]
    fn rejects_symlinks_and_hard_links() {
        let temp = tempfile::tempdir().unwrap();
        let workspace = workspace(temp.path());
        let outside = temp.path().join("outside");
        fs::write(&outside, b"not a proposal").unwrap();
        symlink(&outside, temp.path().join("workspace/outputs/proposal")).unwrap();
        assert_eq!(
            read_attempt_proposal_from_workspace(&workspace, 32),
            Err(CandidateRejection::InvalidCandidate)
        );

        fs::remove_file(temp.path().join("workspace/outputs/proposal")).unwrap();
        fs::hard_link(&outside, temp.path().join("workspace/outputs/proposal")).unwrap();
        assert_eq!(
            read_attempt_proposal_from_workspace(&workspace, 32),
            Err(CandidateRejection::InvalidCandidate)
        );
    }

    #[test]
    fn rejects_a_symlinked_outputs_directory() {
        let temp = tempfile::tempdir().unwrap();
        let workspace_path = temp.path().join("workspace");
        fs::create_dir(&workspace_path).unwrap();
        let outside = temp.path().join("outside");
        fs::create_dir(&outside).unwrap();
        symlink(&outside, workspace_path.join("outputs")).unwrap();
        let workspace = File::open(workspace_path).unwrap();

        assert_eq!(
            read_attempt_proposal_from_workspace(&workspace, 32),
            Err(CandidateRejection::InvalidCandidate)
        );
    }
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("LiteCowork scope gate is unsupported on this platform");
    std::process::exit(126);
}
