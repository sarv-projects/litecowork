#![cfg(target_os = "linux")]

use linux_process_scope::{
    AgentEnvironment, BubblewrapLaunchSpec, LaunchSpec, ReadOnlyRuntimeMount, ResourceLimits,
    SystemdCommands,
    output_channel::{CandidateFrame, read_candidate_channel},
    prepare_bubblewrap_command, spawn_bubblewrapped,
};
use std::{
    ffi::OsString,
    fs,
    io::{Cursor, Read, Write},
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

fn systemd_commands() -> SystemdCommands {
    SystemdCommands {
        systemd_run: PathBuf::from("/usr/bin/systemd-run"),
        systemctl: PathBuf::from("/usr/bin/systemctl"),
        scope_gate: PathBuf::from(env!("CARGO_BIN_EXE_litecowork-scope-gate")),
        env: PathBuf::from("/usr/bin/env"),
    }
}

fn bubblewrap_spec(root: &std::path::Path, attempt_byte: u8, script: &str) -> BubblewrapLaunchSpec {
    let input = root.join("inputs");
    let output = root.join("outputs");
    fs::create_dir(&input).expect("create exact input root");
    fs::create_dir(&output).expect("create private output root");
    fs::set_permissions(&input, fs::Permissions::from_mode(0o700)).expect("protect input root");
    fs::set_permissions(&output, fs::Permissions::from_mode(0o700)).expect("protect output root");
    fs::write(input.join("pinned.txt"), b"pinned input\n").expect("write input fixture");
    fs::set_permissions(input.join("pinned.txt"), fs::Permissions::from_mode(0o400))
        .expect("make input read-only");

    BubblewrapLaunchSpec {
        bubblewrap: PathBuf::from("/usr/bin/bwrap"),
        process: LaunchSpec {
            attempt_id: [attempt_byte; 16],
            executable: PathBuf::from("/usr/bin/dash"),
            args: vec![OsString::from("-c"), OsString::from(script)],
            working_directory: output.clone(),
            environment: AgentEnvironment {
                home: PathBuf::from("/workspace/outputs/home"),
                search_path: vec![PathBuf::from("/usr/bin")],
                config_home: None,
                data_home: None,
                cache_home: None,
                temp_dir: Some(PathBuf::from("/tmp")),
                locale: Some("C".to_owned()),
            },
            limits: ResourceLimits {
                memory_max_bytes: 256 * 1024 * 1024,
                cpu_quota_percent: 100,
                tasks_max: 32,
            },
        },
        input_root: input,
        output_root: output,
        scratch_limit_bytes: 64 * 1024 * 1024,
        candidate_capture: None,
        runtime_mounts: ["/usr", "/lib", "/lib64"]
            .into_iter()
            .map(|path| ReadOnlyRuntimeMount {
                source: PathBuf::from(path),
                target: PathBuf::from(path),
            })
            .collect(),
    }
}

#[test]
#[ignore = "requires live Bubblewrap user namespaces; does not exercise systemd/cgroup qualification"]
fn candidate_runner_as_pid_one_blocks_frame_forgery_and_waits_for_descendants() {
    let runner = env!("CARGO_BIN_EXE_litecowork-scope-gate");
    let agent = concat!(
        "if printf '\\114\\103\\117\\127\\001\\002\\000\\000\\000\\006forged' ",
        "> /proc/1/fd/2 2>/dev/null; then printf 'FORGE_OPENED\\n'; ",
        "else printf 'FORGE_BLOCKED\\n'; fi; ",
        "(/usr/bin/sleep 0.25; printf 'candidate-after-descendant' ",
        "> /workspace/outputs/proposal) & exit 0"
    );
    let started = Instant::now();
    let output = Command::new("/usr/bin/bwrap")
        .args([
            "--die-with-parent",
            "--new-session",
            "--unshare-user",
            "--disable-userns",
            "--assert-userns-disabled",
            "--unshare-pid",
            "--as-pid-1",
            "--unshare-net",
            "--unshare-ipc",
            "--unshare-uts",
            "--hostname",
            "litecowork-runner-check",
            "--cap-drop",
            "ALL",
            "--size",
            "67108864",
            "--tmpfs",
            "/",
            "--dev",
            "/dev",
            "--proc",
            "/proc",
            "--dir",
            "/runtime",
            "--ro-bind",
            runner,
            "/runtime/scope-gate",
            "--dir",
            "/workspace",
            "--size",
            "1048576",
            "--tmpfs",
            "/workspace",
            "--dir",
            "/workspace/outputs",
            "--ro-bind",
            "/usr",
            "/usr",
            "--ro-bind",
            "/lib",
            "/lib",
            "--ro-bind",
            "/lib64",
            "/lib64",
            "--symlink",
            "/usr/bin",
            "/bin",
            "--remount-ro",
            "/",
            "--chdir",
            "/workspace/outputs",
            "--",
            "/runtime/scope-gate",
            "--attempt-runner",
            "4096",
            "--",
            "/usr/bin/dash",
            "-c",
            agent,
        ])
        .output()
        .expect("launch the packaged scope-gate binary inside Bubblewrap");

    assert!(
        output.status.success(),
        "Bubblewrap runner failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"FORGE_BLOCKED\n");
    assert!(started.elapsed() >= Duration::from_millis(200));

    let mut candidate_channel = Cursor::new(output.stderr.as_slice());
    assert_eq!(
        read_candidate_channel(&mut candidate_channel, 4096)
            .expect("the trusted runner emits one well-formed terminal frame"),
        CandidateFrame::Candidate(b"candidate-after-descendant".to_vec())
    );
    assert_eq!(candidate_channel.position() as usize, output.stderr.len());
}

#[test]
#[ignore = "requires live Bubblewrap user namespaces; exercises the held runner, not systemd/cgroup qualification"]
fn held_runner_does_not_start_native_harness_until_admission_signal() {
    let runner = env!("CARGO_BIN_EXE_litecowork-scope-gate");
    let agent = "printf 'AGENT_STARTED\\n'; printf 'held-candidate' > /workspace/outputs/proposal";
    let mut child = Command::new("/usr/bin/bwrap")
        .args([
            "--die-with-parent",
            "--new-session",
            "--unshare-user",
            "--disable-userns",
            "--assert-userns-disabled",
            "--unshare-pid",
            "--as-pid-1",
            "--unshare-net",
            "--unshare-ipc",
            "--unshare-uts",
            "--hostname",
            "litecowork-held-runner-check",
            "--cap-drop",
            "ALL",
            "--size",
            "67108864",
            "--tmpfs",
            "/",
            "--dev",
            "/dev",
            "--proc",
            "/proc",
            "--dir",
            "/runtime",
            "--ro-bind",
            runner,
            "/runtime/scope-gate",
            "--dir",
            "/workspace",
            "--size",
            "1048576",
            "--tmpfs",
            "/workspace",
            "--dir",
            "/workspace/outputs",
            "--ro-bind",
            "/usr",
            "/usr",
            "--ro-bind",
            "/lib",
            "/lib",
            "--ro-bind",
            "/lib64",
            "/lib64",
            "--symlink",
            "/usr/bin",
            "/bin",
            "--remount-ro",
            "/",
            "--chdir",
            "/workspace/outputs",
            "--",
            "/runtime/scope-gate",
            "--held-attempt-runner",
            "4096",
            "--",
            "/usr/bin/dash",
            "-c",
            agent,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("launch the held runner inside Bubblewrap");
    let mut stdin = child.stdin.take().expect("held runner stdin is piped");
    let mut stdout = child.stdout.take().expect("held runner stdout is piped");
    let mut stderr = child.stderr.take().expect("candidate channel is piped");
    let mut ready = vec![0; linux_process_scope::admission_protocol::READY.len()];
    stdout
        .read_exact(&mut ready)
        .expect("trusted supervisor reports held readiness");
    assert_eq!(ready, linux_process_scope::admission_protocol::READY);

    std::thread::sleep(Duration::from_millis(100));
    assert!(
        child
            .try_wait()
            .expect("held supervisor remains observable")
            .is_none(),
        "the native harness must not finish before Runtime admission"
    );

    stdin
        .write_all(linux_process_scope::admission_protocol::START_AFTER_RECEIPT)
        .and_then(|()| stdin.flush())
        .expect("send the exact post-receipt signal");
    drop(stdin);
    let status = child.wait().expect("runner and harness are reaped");
    let mut agent_output = Vec::new();
    stdout
        .read_to_end(&mut agent_output)
        .expect("native harness output is readable");
    let mut candidate_output = Vec::new();
    stderr
        .read_to_end(&mut candidate_output)
        .expect("candidate frame is readable");

    assert!(status.success());
    assert_eq!(agent_output, b"AGENT_STARTED\n");
    assert_eq!(
        read_candidate_channel(&mut Cursor::new(candidate_output), 4096)
            .expect("one candidate follows successful execution"),
        CandidateFrame::Candidate(b"held-candidate".to_vec())
    );
}

#[test]
#[ignore = "requires live Bubblewrap user namespaces; exercises tmpfs only, not systemd/cgroup qualification"]
fn bubblewrap_command_builder_enforces_workspace_tmpfs_cap_and_observes_enospc() {
    const WORKSPACE_CAP_BYTES: u64 = 1024 * 1024;
    let script = concat!(
        "set -- $(/usr/bin/stat -f -c '%S %b' /workspace); ",
        "cap=$(($1 * $2)); printf 'CAP=%s\\n' \"$cap\"; ",
        "[ \"$cap\" -eq 1048576 ] || exit 3; ",
        "test -L /tmp && [ \"$(/usr/bin/readlink /tmp)\" = /workspace/tmp ] || exit 4; ",
        "/usr/bin/dd if=/dev/zero of=/workspace/outputs/output-fill bs=4096 count=64 status=none 2>/dev/null || exit 5; ",
        "if /usr/bin/dd if=/dev/zero of=/tmp/temp-fill bs=4096 count=256 status=none 2>/dev/null; then printf 'CAPACITY_NOT_ENFORCED\\n'; exit 6; fi; ",
        "output_filled=$(/usr/bin/stat -c '%s' /workspace/outputs/output-fill); ",
        "temp_filled=$(/usr/bin/stat -c '%s' /workspace/tmp/temp-fill); ",
        "printf 'OUTPUT_FILLED=%s\\nTEMP_FILLED=%s\\n' \"$output_filled\" \"$temp_filled\"; ",
        "[ \"$temp_filled\" -gt 0 ] && [ \"$((output_filled + temp_filled))\" -ge 900000 ] || exit 7; ",
        "printf 'ENOSPC\\n'"
    );
    let temp = tempfile::tempdir().expect("temporary Environment fixture");
    let mut spec = bubblewrap_spec(temp.path(), 0x74, script);
    spec.scratch_limit_bytes = WORKSPACE_CAP_BYTES;
    let (program, args, _, _, _) =
        prepare_bubblewrap_command(&spec).expect("build the production Bubblewrap command");
    let output = Command::new(program)
        .args(args)
        .output()
        .expect("run the production Bubblewrap command directly without systemd");

    assert!(
        output.status.success(),
        "Bubblewrap command-builder scratch probe failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("probe output is UTF-8");
    assert!(stdout.starts_with(&format!("CAP={WORKSPACE_CAP_BYTES}\n")));
    let output_filled = stdout
        .lines()
        .find_map(|line| line.strip_prefix("OUTPUT_FILLED="))
        .and_then(|value| value.parse::<u64>().ok())
        .expect("probe reports output bytes written");
    let temp_filled = stdout
        .lines()
        .find_map(|line| line.strip_prefix("TEMP_FILLED="))
        .and_then(|value| value.parse::<u64>().ok())
        .expect("probe reports temporary bytes written before ENOSPC");
    assert_eq!(output_filled, 256 * 1024);
    assert!(
        temp_filled > 0,
        "the /tmp symlink must reach the capped scratch"
    );
    assert!(output_filled + temp_filled >= 900_000);
    assert!(stdout.ends_with("ENOSPC\n"), "probe output: {stdout:?}");
}

#[test]
#[ignore = "requires live user systemd manager and delegated cgroup-v2 controllers"]
fn combined_attempt_scope_runs_agent_inside_bubblewrap_and_observes_quiescence() {
    let temp = tempfile::tempdir().expect("temporary Environment fixture");
    let spec = bubblewrap_spec(
        temp.path(),
        0x73,
        "test ! -e /home && cat /workspace/inputs/pinned.txt && printf 'generated\\n' > /workspace/outputs/result.txt && cat /workspace/outputs/result.txt",
    );
    let mut scope = spawn_bubblewrapped(&systemd_commands(), spec.clone())
        .expect("worker starts only after both OS boundaries are set up");

    assert!(scope.limits_are_enforced().expect("kernel applies limits"));
    let child = scope.child_mut().expect("owned scope child");
    let mut stdout = String::new();
    child
        .stdout
        .as_mut()
        .expect("worker stdout inherited")
        .read_to_string(&mut stdout)
        .expect("read bounded test output");
    let status = scope
        .wait_for_quiescence(Duration::from_secs(10))
        .expect("parent slice reports recursive quiescence");

    assert!(status.success());
    assert_eq!(stdout, "pinned input\ngenerated\n");
    assert_eq!(
        fs::read(spec.input_root.join("pinned.txt")).unwrap(),
        b"pinned input\n"
    );
    assert!(!spec.output_root.join("result.txt").exists());
}
