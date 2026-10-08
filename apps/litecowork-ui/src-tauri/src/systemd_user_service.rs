//! Linux-only user service manager for the local LiteCowork Runtime.
//!
//! This establishes daemon lifecycle ownership only. `Delegate=yes` is a required
//! service configuration, not proof that cgroup-v2 containment is usable or that
//! any Task may execute.

use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const SYSTEMCTL: &str = "/usr/bin/systemctl";
const UNIT_NAME: &str = "litecoworkd.service";
const UNIT_MARKER: &str = "# Managed by LiteCowork; schema=1";
const MAX_COMMAND_OUTPUT: usize = 16 * 1024;
// OperatorServer gives already-admitted requests up to 60 seconds to settle after
// admission stops. Keep the service-manager deadline longer so systemd does not send
// SIGKILL while the daemon is still performing its bounded drain and final lifecycle
// persistence. The remaining 30 seconds is a bounded settle margin, not a guarantee
// against a stalled filesystem or database.
const SYSTEMD_STOP_TIMEOUT_SECONDS: u8 = 90;

/// Refreshes the user unit to the currently installed daemon path and starts it.
/// It deliberately does not enable the unit at login or restart an active service.
pub(super) fn start(
    config_dir: &Path,
    daemon: &Path,
    state_dir: &Path,
    deadline: Instant,
) -> Result<(), String> {
    validate_systemctl()?;
    let daemon = absolute_regular_file(daemon)?;
    let state_dir = absolute_path_without_controls(state_dir)?;
    let unit_path = install_unit(config_dir, &daemon, &state_dir)?;

    run_systemctl(&["daemon-reload"], deadline, false)?;
    run_systemctl(&["start", UNIT_NAME], deadline, false)?;
    verify_active_delegated_service(&daemon, &state_dir, deadline)?;

    // Confirm both the unit text and the running process still match the paths
    // installed above. `systemctl start` is a no-op for an already-active unit;
    // checking only ActiveState would incorrectly accept its stale ExecStart.
    validate_managed_unit(&unit_path, &daemon, &state_dir)?;
    Ok(())
}

/// Re-checks service-manager ownership after the authenticated Runtime handshake.
pub(super) fn verify_active(
    config_dir: &Path,
    daemon: &Path,
    state_dir: &Path,
    deadline: Instant,
) -> Result<(), String> {
    let daemon = absolute_regular_file(daemon)?;
    let state_dir = absolute_path_without_controls(state_dir)?;
    let unit_path = config_dir.join("systemd").join("user").join(UNIT_NAME);
    validate_managed_unit(&unit_path, &daemon, &state_dir)?;
    verify_active_delegated_service(&daemon, &state_dir, deadline)
}

fn validate_systemctl() -> Result<(), String> {
    let metadata = fs::metadata(SYSTEMCTL).map_err(|_| unavailable())?;
    if !metadata.is_file() {
        return Err(unavailable());
    }
    Ok(())
}

fn unavailable() -> String {
    "Linux Runtime startup requires /usr/bin/systemctl and an available systemd user manager; LiteCowork did not start the daemon directly".to_owned()
}

fn absolute_regular_file(path: &Path) -> Result<PathBuf, String> {
    let canonical = path
        .canonicalize()
        .map_err(|_| "LiteCowork Runtime executable path is unavailable".to_owned())?;
    let metadata = fs::metadata(&canonical)
        .map_err(|_| "LiteCowork Runtime executable path is unavailable".to_owned())?;
    if !canonical.is_absolute() || !metadata.is_file() {
        return Err("LiteCowork Runtime executable path is not a regular absolute file".to_owned());
    }
    absolute_path_without_controls(&canonical)
}

fn absolute_path_without_controls(path: &Path) -> Result<PathBuf, String> {
    if !path.is_absolute() {
        return Err("LiteCowork systemd service requires absolute paths".to_owned());
    }
    let text = path
        .to_str()
        .ok_or_else(|| "LiteCowork paths must be valid UTF-8 for systemd".to_owned())?;
    if text.chars().any(char::is_control) {
        return Err("LiteCowork paths containing control characters cannot be used by systemd".to_owned());
    }
    Ok(path.to_path_buf())
}

fn install_unit(config_dir: &Path, daemon: &Path, state_dir: &Path) -> Result<PathBuf, String> {
    let config_dir = absolute_path_without_controls(config_dir)?;
    ensure_private_user_directories(&config_dir)?;
    let unit_path = config_dir.join("systemd").join("user").join(UNIT_NAME);
    let body = render_unit(daemon, state_dir)?;

    match fs::symlink_metadata(&unit_path) {
        Ok(metadata) => {
            use rustix::process::geteuid;
            if metadata.file_type().is_symlink()
                || !metadata.is_file()
                || metadata.uid() != geteuid().as_raw()
                || metadata.permissions().mode() & 0o022 != 0
            {
                return Err("LiteCowork systemd unit path is not a private user-owned regular file".to_owned());
            }
            let mut existing = String::new();
            File::open(&unit_path)
                .and_then(|mut file| file.read_to_string(&mut existing))
                .map_err(|_| "LiteCowork could not inspect the existing systemd unit".to_owned())?;
            if !existing.lines().next().is_some_and(|line| line == UNIT_MARKER) {
                return Err("A non-LiteCowork systemd unit already uses litecoworkd.service".to_owned());
            }
            fs::set_permissions(&unit_path, fs::Permissions::from_mode(0o600))
                .map_err(|_| "LiteCowork could not restrict systemd unit permissions".to_owned())?;
            if existing == body {
                return Ok(unit_path);
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return Err("LiteCowork could not inspect the systemd unit path".to_owned()),
    }

    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temporary = unit_path.with_file_name(format!(".{UNIT_NAME}.tmp-{}-{nonce}", std::process::id()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary)
        .map_err(|_| "LiteCowork could not create a private systemd unit".to_owned())?;
    let write_result = file.write_all(body.as_bytes()).and_then(|_| file.sync_all());
    if write_result.is_err() {
        let _ = fs::remove_file(&temporary);
        return Err("LiteCowork could not persist its systemd unit".to_owned());
    }
    drop(file);
    if fs::rename(&temporary, &unit_path).is_err() {
        let _ = fs::remove_file(&temporary);
        return Err("LiteCowork could not install its systemd unit".to_owned());
    }
    File::open(unit_path.parent().unwrap_or(&config_dir))
        .and_then(|directory| directory.sync_all())
        .map_err(|_| "LiteCowork could not persist its systemd unit directory".to_owned())?;
    Ok(unit_path)
}

fn ensure_private_user_directories(config_dir: &Path) -> Result<(), String> {
    use rustix::process::geteuid;

    ensure_user_directory(config_dir, false)?;
    let systemd = config_dir.join("systemd");
    ensure_user_directory(&systemd, true)?;
    let user = systemd.join("user");
    ensure_user_directory(&user, true)?;

    for path in [config_dir, systemd.as_path(), user.as_path()] {
        let metadata = fs::symlink_metadata(path)
            .map_err(|_| "LiteCowork systemd configuration directory is unavailable".to_owned())?;
        if metadata.file_type().is_symlink()
            || !metadata.is_dir()
            || metadata.uid() != geteuid().as_raw()
            || metadata.permissions().mode() & 0o022 != 0
        {
            return Err("LiteCowork systemd configuration directories must be user-owned and not group/world writable".to_owned());
        }
    }
    Ok(())
}

fn ensure_user_directory(path: &Path, private_mode: bool) -> Result<(), String> {
    if let Ok(metadata) = fs::symlink_metadata(path) {
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err("LiteCowork systemd configuration path is not a directory".to_owned());
        }
        if private_mode && metadata.permissions().mode() & 0o077 != 0 {
            fs::set_permissions(path, fs::Permissions::from_mode(0o700))
                .map_err(|_| "LiteCowork could not restrict systemd configuration directory permissions".to_owned())?;
        }
        return Ok(());
    }
    let mut builder = fs::DirBuilder::new();
    builder.mode(0o700);
    builder
        .create(path)
        .map_err(|_| "LiteCowork could not create a private systemd configuration directory".to_owned())
}

fn render_unit(daemon: &Path, state_dir: &Path) -> Result<String, String> {
    let daemon = systemd_argument(daemon)?;
    let state_dir = systemd_argument(state_dir)?;
    Ok(format!(
        "{UNIT_MARKER}\n\
         [Unit]\n\
         Description=LiteCowork local Runtime\n\
         StartLimitIntervalSec=60s\n\
         StartLimitBurst=3\n\
         \n\
         [Service]\n\
         Type=simple\n\
         ExecStart={daemon} run --data-dir {state_dir}\n\
         Restart=on-failure\n\
         RestartSec=2s\n\
         TimeoutStartSec=15s\n\
         TimeoutStopSec={SYSTEMD_STOP_TIMEOUT_SECONDS}s\n\
         KillMode=control-group\n\
         SendSIGKILL=yes\n\
         Delegate=yes\n\
         UMask=0077\n"
    ))
}

#[cfg(test)]
mod tests {
    use super::{render_unit, SYSTEMD_STOP_TIMEOUT_SECONDS};
    use std::path::Path;

    #[test]
    fn managed_unit_allows_operator_drain_before_systemd_kill_deadline() {
        let unit = render_unit(Path::new("/usr/bin/litecoworkd"), Path::new("/var/lib/litecowork"))
            .expect("absolute fixture paths should render");

        assert!(unit.contains("TimeoutStopSec=90s\n"));
        assert!(SYSTEMD_STOP_TIMEOUT_SECONDS > 60);
        assert!(unit.contains("KillMode=control-group\n"));
    }
}

/// systemd unit command-line quoting, with specifier and environment expansion
/// disabled for path content. Control characters are rejected before this point.
fn systemd_argument(path: &Path) -> Result<String, String> {
    let text = path
        .to_str()
        .ok_or_else(|| "LiteCowork paths must be valid UTF-8 for systemd".to_owned())?;
    if !path.is_absolute() || text.chars().any(char::is_control) {
        return Err("LiteCowork systemd arguments must be absolute and contain no control characters".to_owned());
    }
    let escaped = text
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('$', "\\$")
        .replace('`', "\\`")
        .replace('%', "%%");
    Ok(format!("\"{escaped}\""))
}

fn validate_managed_unit(path: &Path, daemon: &Path, state_dir: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|_| "LiteCowork systemd unit is unavailable".to_owned())?;
    use rustix::process::geteuid;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.uid() != geteuid().as_raw()
        || metadata.permissions().mode() & 0o077 != 0
    {
        return Err("LiteCowork systemd unit is no longer a private user-owned file".to_owned());
    }
    let contents = fs::read_to_string(path)
        .map_err(|_| "LiteCowork systemd unit could not be read".to_owned())?;
    let expected = render_unit(daemon, state_dir)?;
    if contents != expected {
        return Err("LiteCowork systemd unit does not match the installed Runtime configuration".to_owned());
    }
    Ok(())
}

fn verify_active_delegated_service(
    daemon: &Path,
    state_dir: &Path,
    deadline: Instant,
) -> Result<(), String> {
    let output = run_systemctl(
        &[
            "show",
            "--property=Id",
            "--property=ActiveState",
            "--property=Delegate",
            "--property=KillMode",
            "--property=MainPID",
            "--value",
            UNIT_NAME,
        ],
        deadline,
        true,
    )?;
    let text = std::str::from_utf8(&output)
        .map_err(|_| "systemd returned an invalid LiteCowork service status".to_owned())?;
    let mut lines = text.lines();
    let id = lines.next().unwrap_or_default();
    let active = lines.next().unwrap_or_default();
    let delegate = lines.next().unwrap_or_default();
    let kill_mode = lines.next().unwrap_or_default();
    let main_pid = lines.next().unwrap_or_default();
    let main_pid = main_pid.parse::<u32>().unwrap_or(0);
    if id != UNIT_NAME
        || active != "active"
        || delegate != "yes"
        || kill_mode != "control-group"
        || main_pid <= 1
    {
        return Err("LiteCowork systemd user service is not active with the required delegation and kill settings".to_owned());
    }
    verify_main_process_arguments(main_pid, daemon, state_dir)?;
    Ok(())
}

fn verify_main_process_arguments(main_pid: u32, daemon: &Path, state_dir: &Path) -> Result<(), String> {
    use std::os::unix::ffi::OsStrExt;

    let executable = fs::read_link(format!("/proc/{main_pid}/exe"))
        .map_err(|_| "LiteCowork systemd service executable identity is unavailable".to_owned())?;
    if executable != daemon {
        return Err("LiteCowork systemd service is not running the selected Runtime executable".to_owned());
    }

    let command_line = fs::read(format!("/proc/{main_pid}/cmdline"))
        .map_err(|_| "LiteCowork systemd service process command line is unavailable".to_owned())?;
    let actual = command_line
        .split(|byte| *byte == 0)
        .filter(|argument| !argument.is_empty())
        .collect::<Vec<_>>();
    let expected = [
        daemon.as_os_str().as_bytes(),
        b"run".as_slice(),
        b"--data-dir".as_slice(),
        state_dir.as_os_str().as_bytes(),
    ];
    if actual.len() != expected.len() || actual.iter().zip(expected).any(|(actual, expected)| *actual != expected) {
        return Err("LiteCowork systemd service is running a different Runtime command or data directory".to_owned());
    }
    Ok(())
}

fn run_systemctl(args: &[&str], deadline: Instant, capture_stdout: bool) -> Result<Vec<u8>, String> {
    if Instant::now() >= deadline {
        return Err("systemd user service operation exceeded the Runtime startup deadline".to_owned());
    }
    let mut command = Command::new(SYSTEMCTL);
    command.arg("--user").arg("--no-ask-password").arg("--no-pager");
    if args.first() != Some(&"show") {
        command.arg("--quiet");
    }
    command.args(args);
    command.stdin(Stdio::null()).stderr(Stdio::null());
    if capture_stdout {
        command.stdout(Stdio::piped());
    } else {
        command.stdout(Stdio::null());
    }
    command.env_remove("DBUS_SESSION_BUS_ADDRESS");
    command.env_remove("SYSTEMD_BUS_ADDRESS");
    command.env_remove("SYSTEMD_UNIT_PATH");

    let mut child = command.spawn().map_err(|_| unavailable())?;
    let output_reader = if capture_stdout {
        let stdout = match child.stdout.take() {
            Some(stdout) => stdout,
            None => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("systemd service status output is unavailable".to_owned());
            }
        };
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        thread::Builder::new()
            .name("litecowork-systemd-status".to_owned())
            .spawn(move || {
                let mut output = Vec::new();
                let mut reader = stdout;
                let mut chunk = [0_u8; 4096];
                let mut oversized = false;
                let result = loop {
                    match reader.read(&mut chunk) {
                        Ok(0) => break Ok((output, oversized)),
                        Ok(count) => {
                            let remaining = (MAX_COMMAND_OUTPUT + 1).saturating_sub(output.len());
                            let retained = count.min(remaining);
                            output.extend_from_slice(&chunk[..retained]);
                            oversized |= retained < count || output.len() > MAX_COMMAND_OUTPUT;
                        }
                        Err(_) => break Err("systemd service status could not be read".to_owned()),
                    }
                };
                let _ = sender.send(result);
            })
            .map_err(|_| {
                let _ = child.kill();
                let _ = child.wait();
                "systemd service status reader could not start".to_owned()
            })?;
        Some(receiver)
    } else {
        None
    };

    let status = loop {
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err("systemd user service operation exceeded the Runtime startup deadline".to_owned());
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => thread::sleep(Duration::from_millis(10).min(deadline.saturating_duration_since(Instant::now()))),
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("systemd user service process could not be observed".to_owned());
            }
        }
    };
    if !status.success() {
        return Err(if args.first() == Some(&"start") {
            "LiteCowork systemd user service failed to start; direct daemon startup is disabled on Linux".to_owned()
        } else {
            unavailable()
        });
    }
    let (output, oversized) = if let Some(receiver) = output_reader {
        receiver
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .map_err(|_| "systemd service status exceeded the Runtime startup deadline".to_owned())??
    } else {
        (Vec::new(), false)
    };
    if oversized || output.len() > MAX_COMMAND_OUTPUT {
        return Err("systemd service status exceeded its output limit".to_owned());
    }
    Ok(output)
}
