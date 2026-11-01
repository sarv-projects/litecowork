#![cfg(target_os = "linux")]

use linux_process_scope::{
    AgentEnvironment, BubblewrapLaunchSpec, LaunchSpec, ReadOnlyRuntimeMount, ResourceLimits,
    SystemdCommands, spawn_bubblewrapped,
};
use std::{
    ffi::OsString, fs, io::Read, os::unix::fs::PermissionsExt, path::PathBuf, time::Duration,
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
#[ignore = "requires live user systemd manager and delegated cgroup-v2 controllers"]
fn combined_attempt_scope_runs_agent_inside_bubblewrap_and_observes_quiescence() {
    let temp = tempfile::tempdir().expect("temporary Environment fixture");
    let spec = bubblewrap_spec(
        temp.path(),
        0x73,
        "test ! -e /home && cat /workspace/inputs/pinned.txt && printf 'generated\\n' > /workspace/outputs/result.txt",
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
    assert_eq!(stdout, "pinned input\n");
    assert_eq!(
        fs::read(spec.input_root.join("pinned.txt")).unwrap(),
        b"pinned input\n"
    );
    assert_eq!(
        fs::read(spec.output_root.join("result.txt")).unwrap(),
        b"generated\n"
    );
}
