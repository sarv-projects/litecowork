//! Linux filesystem/network namespace launch layered over the cgroup process scope.
//!
//! This is a real OS-enforced worker boundary, but deliberately not a complete
//! Environment provider: it has no egress broker, aggregate output quota, Runtime-local
//! attestation, Task/Attempt admission, or Trust/Effect/lease reconciliation.

use crate::{AgentEnvironment, LaunchSpec, ManagedScope, ScopeError, SystemdCommands, spawn};
use std::{
    ffi::OsString,
    fs,
    os::unix::fs::MetadataExt,
    path::{Component, Path, PathBuf},
};

const INPUT_TARGET: &str = "/workspace/inputs";
const OUTPUT_TARGET: &str = "/workspace/outputs";
const WORKSPACE_TARGET: &str = "/workspace";
const TMP_TARGET: &str = "/tmp";
const MAX_RUNTIME_MOUNTS: usize = 32;
const SYSTEM_RUNTIME_ROOTS: [&str; 6] = ["/usr", "/bin", "/sbin", "/lib", "/lib64", "/opt"];

/// Trusted host runtime tree exposed read-only at one explicit sandbox path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReadOnlyRuntimeMount {
    pub source: PathBuf,
    pub target: PathBuf,
}

/// Input to the qualified Linux launch path. `process.executable` and
/// `process.environment` are host-side adapter values: the executable must reside under
/// one `runtime_mounts` source, while environment paths must name sandbox paths. The
/// process working directory is ignored and replaced with the fixed output directory.
#[derive(Clone, Debug)]
pub struct BubblewrapLaunchSpec {
    pub bubblewrap: PathBuf,
    pub process: LaunchSpec,
    pub input_root: PathBuf,
    pub output_root: PathBuf,
    pub runtime_mounts: Vec<ReadOnlyRuntimeMount>,
}

/// Build and launch Bubblewrap under the existing Attempt-scoped cgroup supervisor.
/// No agent starts unless Bubblewrap confirms its required user/PID/network namespaces.
pub fn spawn_bubblewrapped(
    commands: &SystemdCommands,
    spec: BubblewrapLaunchSpec,
) -> Result<ManagedScope, ScopeError> {
    let (bubblewrap, arguments, output_root, limits, attempt_id) =
        prepare_bubblewrap_command(&spec)?;

    let supervisor_spec = LaunchSpec {
        attempt_id,
        executable: bubblewrap,
        args: arguments,
        working_directory: output_root,
        environment: AgentEnvironment {
            // This environment is for the trusted Bubblewrap bootstrap only. It is
            // cleared inside the sandbox before the native agent is exec'd.
            home: PathBuf::from(TMP_TARGET),
            search_path: vec![PathBuf::from("/usr/bin")],
            config_home: None,
            data_home: None,
            cache_home: None,
            temp_dir: None,
            locale: Some("C".to_owned()),
        },
        limits,
    };
    spawn(commands, supervisor_spec)
}

/// Return the canonical Bubblewrap executable and a validated argument vector. This is
/// public for the desktop Runtime's startup qualification and for independent system
/// tests; it does not itself execute or attest a worker.
pub fn prepare_bubblewrap_command(
    spec: &BubblewrapLaunchSpec,
) -> Result<
    (
        PathBuf,
        Vec<OsString>,
        PathBuf,
        crate::ResourceLimits,
        [u8; 16],
    ),
    ScopeError,
> {
    let bubblewrap = trusted_bubblewrap(&spec.bubblewrap)?;
    let input_root = private_directory(&spec.input_root, "input root")?;
    let output_root = private_directory(&spec.output_root, "output root")?;
    if input_root == output_root
        || input_root.starts_with(&output_root)
        || output_root.starts_with(&input_root)
    {
        return Err(ScopeError::InvalidInput(
            "input and output roots must be separate directories",
        ));
    }
    if fs::read_dir(&output_root)
        .map_err(ScopeError::Io)?
        .next()
        .is_some()
    {
        return Err(ScopeError::InvalidInput(
            "Attempt output root must be new and empty",
        ));
    }
    validate_input_tree(&input_root)?;
    if spec.runtime_mounts.is_empty() || spec.runtime_mounts.len() > MAX_RUNTIME_MOUNTS {
        return Err(ScopeError::InvalidInput(
            "runtime mount count is empty or exceeds its bound",
        ));
    }

    let mut mounts = Vec::with_capacity(spec.runtime_mounts.len());
    let mut guest_targets = Vec::with_capacity(spec.runtime_mounts.len());
    for mount in &spec.runtime_mounts {
        let source = trusted_runtime_mount_source(&mount.source)?;
        validate_guest_mount_target(&mount.target)?;
        if source == input_root
            || source == output_root
            || source.starts_with(&input_root)
            || source.starts_with(&output_root)
            || input_root.starts_with(&source)
            || output_root.starts_with(&source)
        {
            return Err(ScopeError::InvalidInput(
                "runtime mount overlaps a Task input/output root",
            ));
        }
        if guest_targets.iter().any(|target: &PathBuf| {
            target == &mount.target
                || target.starts_with(&mount.target)
                || mount.target.starts_with(target)
        }) {
            return Err(ScopeError::InvalidInput(
                "runtime mount targets overlap or duplicate",
            ));
        }
        guest_targets.push(mount.target.clone());
        mounts.push((source, mount.target.clone()));
    }

    let executable = fs::canonicalize(&spec.process.executable).map_err(ScopeError::Io)?;
    let executable_metadata = fs::metadata(&executable).map_err(ScopeError::Io)?;
    if !executable_metadata.is_file() {
        return Err(ScopeError::InvalidInput("agent executable is not a file"));
    }
    let guest_executable = mounts
        .iter()
        .find_map(|(source, target)| {
            executable
                .strip_prefix(source)
                .ok()
                .map(|relative| target.join(relative))
        })
        .ok_or(ScopeError::InvalidInput(
            "agent executable is outside every read-only runtime mount",
        ))?;

    let mut arguments = vec![
        arg("--die-with-parent"),
        arg("--new-session"),
        arg("--unshare-user"),
        arg("--disable-userns"),
        arg("--assert-userns-disabled"),
        arg("--unshare-pid"),
        arg("--unshare-net"),
        arg("--unshare-ipc"),
        arg("--unshare-uts"),
        arg("--hostname"),
        arg("litecowork-worker"),
        arg("--tmpfs"),
        arg("/"),
        arg("--dir"),
        arg(TMP_TARGET),
        arg("--dev"),
        arg("/dev"),
        arg("--proc"),
        arg("/proc"),
        arg("--dir"),
        arg(WORKSPACE_TARGET),
        arg("--ro-bind"),
        input_root.as_os_str().to_os_string(),
        arg(INPUT_TARGET),
        arg("--bind"),
        output_root.as_os_str().to_os_string(),
        arg(OUTPUT_TARGET),
    ];
    for (source, target) in mounts {
        arguments.push(arg("--ro-bind"));
        arguments.push(source.into_os_string());
        arguments.push(target.into_os_string());
    }

    arguments.extend([
        arg("--dir"),
        arg(format!("{OUTPUT_TARGET}/home")),
        arg("--clearenv"),
    ]);
    let sandbox_environment = sandbox_environment(&spec.process.environment, &guest_targets)?;
    for (name, value) in sandbox_environment {
        arguments.extend([arg("--setenv"), OsString::from(name), OsString::from(value)]);
    }
    arguments.extend([arg("--setenv"), arg("PWD"), arg(OUTPUT_TARGET)]);
    arguments.extend([
        arg("--chdir"),
        arg(OUTPUT_TARGET),
        arg("--"),
        guest_executable.into_os_string(),
    ]);
    arguments.extend(spec.process.args.iter().cloned());

    Ok((
        bubblewrap,
        arguments,
        output_root,
        spec.process.limits,
        spec.process.attempt_id,
    ))
}

fn trusted_bubblewrap(path: &Path) -> Result<PathBuf, ScopeError> {
    let canonical = fs::canonicalize(path).map_err(ScopeError::Io)?;
    let metadata = fs::metadata(&canonical).map_err(ScopeError::Io)?;
    if !metadata.is_file() || metadata.uid() != 0 || metadata.mode() & 0o022 != 0 {
        return Err(ScopeError::Unsupported(
            "Bubblewrap must be a root-owned, non-group/world-writable executable".into(),
        ));
    }
    Ok(canonical)
}

fn trusted_runtime_mount_source(path: &Path) -> Result<PathBuf, ScopeError> {
    let canonical = fs::canonicalize(path).map_err(ScopeError::Io)?;
    let metadata = fs::metadata(&canonical).map_err(ScopeError::Io)?;
    let allowed = SYSTEM_RUNTIME_ROOTS.iter().any(|root| {
        fs::canonicalize(root)
            .map(|root| canonical.starts_with(root))
            .unwrap_or(false)
    });
    if !metadata.is_dir() || metadata.uid() != 0 || metadata.mode() & 0o022 != 0 || !allowed {
        return Err(ScopeError::Unsupported(
            "runtime mounts must be root-owned, non-writable system trees under /usr, /bin, /sbin, /lib, /lib64, or /opt".into(),
        ));
    }
    Ok(canonical)
}

fn private_directory(path: &Path, label: &'static str) -> Result<PathBuf, ScopeError> {
    let metadata = fs::symlink_metadata(path).map_err(ScopeError::Io)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() || metadata.mode() & 0o077 != 0 {
        return Err(ScopeError::InvalidInput(label));
    }
    fs::canonicalize(path).map_err(ScopeError::Io)
}

fn validate_input_tree(root: &Path) -> Result<(), ScopeError> {
    let mut pending = vec![root.to_path_buf()];
    let mut entries = 0_usize;
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory).map_err(ScopeError::Io)? {
            entries = entries
                .checked_add(1)
                .ok_or(ScopeError::InvalidInput("input entry count overflow"))?;
            if entries > 100_000 {
                return Err(ScopeError::InvalidInput(
                    "input root exceeds the entry-count bound",
                ));
            }
            let path = entry.map_err(ScopeError::Io)?.path();
            let metadata = fs::symlink_metadata(&path).map_err(ScopeError::Io)?;
            if metadata.file_type().is_symlink() || metadata.nlink() != 1 {
                return Err(ScopeError::InvalidInput(
                    "input tree contains a symlink or hard link",
                ));
            }
            if metadata.is_dir() {
                pending.push(path);
            } else if !metadata.is_file() {
                return Err(ScopeError::InvalidInput(
                    "input tree contains a non-regular entry",
                ));
            }
        }
    }
    Ok(())
}

fn validate_guest_mount_target(target: &Path) -> Result<(), ScopeError> {
    if !target.is_absolute()
        || target
            .components()
            .any(|component| !matches!(component, Component::RootDir | Component::Normal(_)))
        || target == Path::new("/")
        || target == Path::new(WORKSPACE_TARGET)
        || target.starts_with(WORKSPACE_TARGET)
        || target == Path::new(TMP_TARGET)
        || target.starts_with(TMP_TARGET)
        || ["/proc", "/dev", "/run", "/sys", "/home", "/root"]
            .iter()
            .any(|reserved| target == Path::new(reserved) || target.starts_with(reserved))
    {
        return Err(ScopeError::InvalidInput(
            "runtime mount target is non-normalized or reserved",
        ));
    }
    Ok(())
}

fn sandbox_environment(
    environment: &AgentEnvironment,
    runtime_targets: &[PathBuf],
) -> Result<Vec<(&'static str, String)>, ScopeError> {
    let safe = crate::linux::sandbox_environment_values(environment)?;
    for (name, value) in &safe {
        if *name == "PATH" {
            for path in value.split(':') {
                validate_guest_environment_path(Path::new(path), runtime_targets)?;
            }
        } else if [
            "HOME",
            "XDG_CONFIG_HOME",
            "XDG_DATA_HOME",
            "XDG_CACHE_HOME",
            "TMPDIR",
        ]
        .contains(name)
        {
            validate_guest_environment_path(Path::new(value), runtime_targets)?;
        }
    }
    Ok(safe)
}

fn validate_guest_environment_path(
    path: &Path,
    runtime_targets: &[PathBuf],
) -> Result<(), ScopeError> {
    if !path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::RootDir | Component::Normal(_)))
        || !(path.starts_with(OUTPUT_TARGET)
            || path.starts_with(TMP_TARGET)
            || runtime_targets.iter().any(|root| path.starts_with(root)))
    {
        return Err(ScopeError::InvalidInput(
            "agent environment path escapes explicit sandbox roots",
        ));
    }
    Ok(())
}

fn arg(value: impl Into<OsString>) -> OsString {
    value.into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ResourceLimits;
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;
    use tempfile::tempdir;

    fn test_spec(root: &Path) -> BubblewrapLaunchSpec {
        let input = root.join("inputs");
        let output = root.join("outputs");
        fs::create_dir(&input).unwrap();
        fs::create_dir(&output).unwrap();
        fs::set_permissions(&input, fs::Permissions::from_mode(0o700)).unwrap();
        fs::set_permissions(&output, fs::Permissions::from_mode(0o700)).unwrap();
        fs::write(input.join("pinned.txt"), "exact input\n").unwrap();
        fs::set_permissions(input.join("pinned.txt"), fs::Permissions::from_mode(0o400)).unwrap();
        BubblewrapLaunchSpec {
            bubblewrap: PathBuf::from("/usr/bin/bwrap"),
            process: LaunchSpec {
                attempt_id: [7; 16],
                executable: PathBuf::from("/usr/bin/dash"),
                args: vec![
                    OsString::from("-c"),
                    OsString::from("cat /workspace/inputs/pinned.txt"),
                ],
                working_directory: output.clone(),
                environment: AgentEnvironment {
                    home: PathBuf::from("/workspace/outputs/home"),
                    search_path: vec![PathBuf::from("/usr/bin")],
                    config_home: None,
                    data_home: None,
                    cache_home: None,
                    temp_dir: Some(PathBuf::from("/tmp")),
                    locale: Some("C".into()),
                },
                limits: ResourceLimits {
                    memory_max_bytes: 256 * 1024 * 1024,
                    cpu_quota_percent: 100,
                    tasks_max: 32,
                },
            },
            input_root: input,
            output_root: output,
            runtime_mounts: vec![
                ReadOnlyRuntimeMount {
                    source: PathBuf::from("/usr"),
                    target: PathBuf::from("/usr"),
                },
                ReadOnlyRuntimeMount {
                    source: PathBuf::from("/lib"),
                    target: PathBuf::from("/lib"),
                },
                ReadOnlyRuntimeMount {
                    source: PathBuf::from("/lib64"),
                    target: PathBuf::from("/lib64"),
                },
            ],
        }
    }

    #[test]
    fn builds_network_and_filesystem_isolation_before_agent_exec() {
        let temp = tempdir().unwrap();
        let spec = test_spec(temp.path());
        let (_, args, output, _, _) = prepare_bubblewrap_command(&spec).unwrap();
        let args = args
            .iter()
            .map(|arg| arg.to_string_lossy())
            .collect::<Vec<_>>();
        assert_eq!(output, spec.output_root.canonicalize().unwrap());
        for required in [
            "--unshare-user",
            "--disable-userns",
            "--assert-userns-disabled",
            "--unshare-pid",
            "--unshare-net",
            "--ro-bind",
            "--bind",
            "--tmpfs",
            "--clearenv",
        ] {
            assert!(args.iter().any(|arg| arg == required), "{required}");
        }
        let clear = args.iter().position(|arg| arg == "--clearenv").unwrap();
        let first_env = args.iter().position(|arg| arg == "--setenv").unwrap();
        assert!(clear < first_env);
        assert!(args.iter().any(|arg| arg == "/workspace/inputs"));
        assert!(args.iter().any(|arg| arg == "/workspace/outputs"));
    }

    #[test]
    fn rejects_symlinks_hardlinks_and_output_reuse() {
        let temp = tempdir().unwrap();
        let mut spec = test_spec(temp.path());
        std::os::unix::fs::symlink("/etc/passwd", spec.input_root.join("escape")).unwrap();
        assert!(matches!(
            prepare_bubblewrap_command(&spec),
            Err(ScopeError::InvalidInput(_))
        ));
        fs::remove_file(spec.input_root.join("escape")).unwrap();
        fs::hard_link(
            spec.input_root.join("pinned.txt"),
            spec.input_root.join("second.txt"),
        )
        .unwrap();
        assert!(matches!(
            prepare_bubblewrap_command(&spec),
            Err(ScopeError::InvalidInput(_))
        ));
        fs::remove_file(spec.input_root.join("second.txt")).unwrap();
        fs::write(spec.output_root.join("stale"), "not new").unwrap();
        assert!(matches!(
            prepare_bubblewrap_command(&spec),
            Err(ScopeError::InvalidInput(_))
        ));
        spec.output_root = spec.input_root.clone();
        assert!(matches!(
            prepare_bubblewrap_command(&spec),
            Err(ScopeError::InvalidInput(_))
        ));
    }

    #[test]
    fn rejects_reserved_mount_targets_and_host_executable_outside_mounts() {
        let temp = tempdir().unwrap();
        let mut spec = test_spec(temp.path());
        spec.runtime_mounts[0].target = PathBuf::from("/run/user/1000");
        assert!(matches!(
            prepare_bubblewrap_command(&spec),
            Err(ScopeError::InvalidInput(_))
        ));
        spec.runtime_mounts[0].target = PathBuf::from("/usr");
        spec.process.executable = PathBuf::from("/etc/passwd");
        assert!(matches!(
            prepare_bubblewrap_command(&spec),
            Err(ScopeError::InvalidInput(_))
        ));
    }

    #[test]
    fn rejects_user_controlled_host_paths_as_runtime_mount_sources() {
        let temp = tempdir().unwrap();
        let mut spec = test_spec(temp.path());
        spec.runtime_mounts[0].source = temp.path().to_path_buf();
        assert!(matches!(
            prepare_bubblewrap_command(&spec),
            Err(ScopeError::Unsupported(_))
        ));
    }

    #[test]
    fn rejects_guest_path_traversal_in_runtime_and_environment_bindings() {
        let temp = tempdir().unwrap();
        let mut spec = test_spec(temp.path());
        spec.runtime_mounts[0].target = PathBuf::from("/runtime/../../home");
        assert!(prepare_bubblewrap_command(&spec).is_err());
        spec.runtime_mounts[0].target = PathBuf::from("/usr");
        spec.process.environment.home = PathBuf::from("/home/user");
        assert!(prepare_bubblewrap_command(&spec).is_err());
    }

    #[test]
    #[ignore = "real user-namespace Bubblewrap qualification; run on each claimed Linux build"]
    fn live_bubblewrap_exposes_pinned_inputs_read_only_and_only_output_writable() {
        let temp = tempdir().unwrap();
        let mut spec = test_spec(temp.path());
        spec.process.args = vec![
            OsString::from("-c"),
            OsString::from(
                "test ! -e /home && test \"$(cat /workspace/inputs/pinned.txt)\" = 'exact input' && printf 'generated\\n' > /workspace/outputs/result.txt && printf 'blocked\\n' > /workspace/inputs/pinned.txt",
            ),
        ];
        let (program, args, _, _, _) = prepare_bubblewrap_command(&spec).unwrap();
        let output = Command::new(program)
            .args(args)
            .output()
            .expect("launch real Bubblewrap namespaces");
        assert!(!output.status.success(), "input mount must be read-only");
        assert_eq!(
            fs::read(spec.input_root.join("pinned.txt")).unwrap(),
            b"exact input\n"
        );
        assert_eq!(
            fs::read(spec.output_root.join("result.txt")).unwrap(),
            b"generated\n"
        );
    }

    #[test]
    #[ignore = "real network-namespace Bubblewrap qualification; run on each claimed Linux build"]
    fn live_bubblewrap_network_namespace_cannot_reach_host_loopback() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::time::Duration;

        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        listener
            .set_nonblocking(true)
            .expect("listener accepts nonblocking mode");
        let port = listener.local_addr().unwrap().port();
        let temp = tempdir().unwrap();
        let mut spec = test_spec(temp.path());
        spec.process.executable = PathBuf::from("/usr/bin/python3");
        spec.process.args = vec![
            OsString::from("-c"),
            OsString::from(format!(
                "import socket; s=socket.socket(); s.settimeout(0.5);\ntry: s.connect(('127.0.0.1',{port})); raise SystemExit(7)\nexcept OSError: print('isolated')"
            )),
        ];
        let (program, args, _, _, _) = prepare_bubblewrap_command(&spec).unwrap();
        let output = Command::new(program)
            .args(args)
            .output()
            .expect("launch real Bubblewrap network namespace");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "isolated");
        let mut accepted = false;
        match listener.accept() {
            Ok((mut stream, _)) => {
                accepted = true;
                let _ = stream.write_all(b"unexpected");
                let mut buffer = [0; 8];
                let _ = stream.read(&mut buffer);
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(error) => panic!("listener check failed: {error}"),
        }
        std::thread::sleep(Duration::from_millis(25));
        assert!(!accepted, "sandbox reached the host loopback listener");
    }
}

/// Runtime capability probe. The user-visible result must remain unavailable unless a
/// separate Environment integration also proves output limits, egress, authority, and
/// durable recovery.
pub fn bubblewrap_binary_is_trusted(path: &Path) -> bool {
    trusted_bubblewrap(path).is_ok()
}
