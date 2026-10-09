#![cfg(target_os = "linux")]

use linux_process_scope::{AgentEnvironment, LaunchSpec, ResourceLimits, SystemdCommands, spawn};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::time::Duration;
use std::{ffi::OsString, process::Command};

fn systemd_commands() -> SystemdCommands {
    SystemdCommands {
        systemd_run: PathBuf::from("/usr/bin/systemd-run"),
        systemctl: PathBuf::from("/usr/bin/systemctl"),
        scope_gate: PathBuf::from(env!("CARGO_BIN_EXE_litecowork-scope-gate")),
        env: PathBuf::from("/usr/bin/env"),
    }
}

fn spec(working_directory: PathBuf, attempt_byte: u8, script: &str) -> LaunchSpec {
    LaunchSpec {
        attempt_id: [attempt_byte; 16],
        executable: PathBuf::from("/usr/bin/sh"),
        args: vec![OsString::from("-c"), OsString::from(script)],
        working_directory: working_directory.clone(),
        environment: AgentEnvironment {
            home: working_directory,
            search_path: vec![PathBuf::from("/usr/bin")],
            config_home: None,
            data_home: None,
            cache_home: None,
            temp_dir: None,
            locale: Some("C.UTF-8".to_owned()),
        },
        limits: ResourceLimits {
            memory_max_bytes: 256 * 1024 * 1024,
            cpu_quota_percent: 100,
            tasks_max: 32,
        },
    }
}

fn assert_attempt_slice_stopped(attempt_byte: u8) {
    let slice = format!(
        "litecowork-attempt-{}.slice",
        format!("{attempt_byte:02x}").repeat(16)
    );
    let output = Command::new("/usr/bin/systemctl")
        .args([
            "--user",
            "--no-pager",
            "show",
            "--value",
            "--property=ActiveState",
            &slice,
        ])
        .output()
        .expect("query the per-Attempt parent slice");
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "inactive",
        "the empty observer slice is stopped after positive quiescence"
    );
}

/// Runs only on an explicitly selected Linux host with a live user systemd manager,
/// writable delegated cgroup v2 controllers, and the scope-gate binary. This qualifies
/// the process/cgroup primitive; it does not qualify filesystem or network isolation.
#[test]
#[ignore = "requires a live Linux systemd user manager and delegated cgroup v2"]
fn systemd_scope_checks_limits_before_exec_and_preserves_native_stdio() {
    let temp = tempfile::tempdir().expect("temporary fixture directory");
    let commands = systemd_commands();
    let mut scope = spawn(
        &commands,
        spec(
            temp.path().to_path_buf(),
            0x71,
            "IFS= read -r line; printf 'child-out:%s\\n' \"$line\"; printf 'child-err\\n' >&2",
        ),
    )
    .expect("worker starts only after scope membership and cgroup limits are verified");

    assert!(scope.limits_are_enforced().expect("read cgroup limits"));
    let child = scope.child_mut().expect("managed child");
    let mut stdin = child.stdin.take().expect("stdin inherited through exec");
    stdin.write_all(b"hello\n").expect("write fixture input");
    drop(stdin);

    let mut stdout = String::new();
    child
        .stdout
        .as_mut()
        .expect("stdout inherited through exec")
        .read_to_string(&mut stdout)
        .expect("read fixture stdout");
    let mut stderr = String::new();
    child
        .stderr
        .as_mut()
        .expect("stderr inherited through exec")
        .read_to_string(&mut stderr)
        .expect("read fixture stderr");

    let status = scope
        .wait_for_quiescence(Duration::from_secs(10))
        .expect("recursive cgroup emptiness is observed");
    assert!(status.success());
    assert_eq!(stdout, "child-out:hello\n");
    assert_eq!(stderr, "child-err\n");
    assert!(!stdout.contains("LITECOWORK_SCOPE_"));
    assert!(!stderr.contains("LITECOWORK_SCOPE_"));
    assert_attempt_slice_stopped(0x71);
}

#[test]
#[ignore = "requires a live Linux systemd user manager and delegated cgroup v2"]
fn systemd_scope_kills_descendants_before_confirming_quiescence() {
    let temp = tempfile::tempdir().expect("temporary fixture directory");
    let mut scope = spawn(
        &systemd_commands(),
        spec(
            temp.path().to_path_buf(),
            0x72,
            "sleep 60 & child=$!; echo descendant:$child; wait",
        ),
    )
    .expect("fixture starts in a managed scope");

    let child = scope.child_mut().expect("managed child");
    let mut stdout = BufReader::new(
        child
            .stdout
            .as_mut()
            .expect("stdout inherited through exec"),
    );
    let mut line = String::new();
    stdout.read_line(&mut line).expect("read fixture marker");
    let descendant_pid = line
        .strip_prefix("descendant:")
        .expect("fixture reports its descendant PID")
        .trim()
        .parse::<u32>()
        .expect("fixture PID is numeric");
    drop(stdout);

    let status = scope
        .kill_and_wait(Duration::from_secs(10))
        .expect("scope kill waits for the entire cgroup subtree");
    assert!(!status.success(), "the fixture shell should be terminated");
    assert!(
        !PathBuf::from(format!("/proc/{descendant_pid}")).exists(),
        "the descendant process is gone before quiescence is returned"
    );
    assert_attempt_slice_stopped(0x72);
}
