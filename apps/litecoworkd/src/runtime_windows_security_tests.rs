use super::{
    ensure_private_state_directory, open_runtime_lock_file, read_state, restrict_file_permissions,
};
use std::{fs, path::Path, process::Command};

fn path_dacl_is_protected(path: &Path) -> bool {
    let result = Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "$acl = Get-Acl -LiteralPath $env:LITECOWORK_RUNTIME_ACL_TEST_PATH; $allowed = @('S-1-3-4','S-1-5-18'); $rules = @($acl.Access); if (-not $acl.AreAccessRulesProtected) { [Console]::Out.Write('inherited'); exit 0 }; if ($rules.Count -ne 2) { exit 1 }; foreach ($rule in $rules) { $sid = $rule.IdentityReference.Translate([Security.Principal.SecurityIdentifier]).Value; if ($sid -notin $allowed -or $rule.IsInherited -or $rule.AccessControlType -ne 'Allow') { exit 2 } }; [Console]::Out.Write('protected'); exit 0",
        ])
        .env("LITECOWORK_RUNTIME_ACL_TEST_PATH", path)
        .output()
        .expect("inspect Windows Runtime path ACL");
    assert!(
        result.status.success(),
        "Runtime path ACL inspection must succeed; stderr: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    matches!(String::from_utf8_lossy(&result.stdout).trim(), "protected")
}

fn assert_path_dacl_is_protected(path: &Path) {
    assert!(
        path_dacl_is_protected(path),
        "Runtime path DACL must be protected and grant access only to OWNER RIGHTS and SYSTEM"
    );
}

#[test]
fn runtime_bootstrap_protects_state_directory_and_lifecycle_files() {
    let temporary = tempfile::tempdir().expect("temporary Runtime state parent");
    let state_directory = temporary.path().join("runtime");
    ensure_private_state_directory(&state_directory).expect("create private Runtime directory");
    assert_path_dacl_is_protected(&state_directory);

    let lock_path = state_directory.join("runtime.lock");
    let lock_file = open_runtime_lock_file(&lock_path).expect("open Runtime lock");
    restrict_file_permissions(&lock_path, &lock_file).expect("secure Runtime lock permissions");
    drop(lock_file);
    assert_path_dacl_is_protected(&lock_path);

    super::persist_state(
        &state_directory,
        &super::RuntimeLocalState {
            schema_version: 1,
            runtime_id: "rt_windows_acl_test".to_owned(),
            local_principal_id: None,
            os_principal_binding_established: false,
            local_incarnation_id: "rli_windows_acl_test".to_owned(),
            startup_policy: super::StartupPolicy::Manual,
            recovery_state: super::RuntimeState::Stopped,
            recovered_from_unclean_shutdown: false,
            last_shutdown_clean: true,
            blockers: Vec::new(),
            version: 1,
        },
    )
    .expect("persist Runtime lifecycle state");
    let state_path = state_directory.join("runtime-state.json");
    read_state(&state_path).expect("read persisted Runtime lifecycle state");
    assert_path_dacl_is_protected(&state_path);
}

#[test]
fn runtime_state_directory_rejects_a_junction_reparse_point() {
    let temporary = tempfile::tempdir().expect("temporary Runtime state parent");
    let target = temporary.path().join("actual-directory");
    let junction = temporary.path().join("state-junction");
    fs::create_dir(&target).expect("create junction target");
    let result = Command::new("cmd.exe")
        .args(["/C", "mklink", "/J"])
        .arg(&junction)
        .arg(&target)
        .output()
        .expect("create Runtime state junction fixture");
    assert!(result.status.success(), "junction setup failed");

    let rejected = super::ensure_private_state_directory(&junction).is_err();
    fs::remove_dir(&junction).expect("remove junction without traversing target");
    assert!(
        rejected,
        "Runtime bootstrap must reject a junction used as its state directory"
    );
}

#[test]
fn runtime_state_reader_rejects_file_symlink_without_changing_its_target_acl() {
    use std::os::windows::fs::symlink_file;

    let temporary = tempfile::tempdir().expect("temporary Runtime state parent");
    let target = temporary.path().join("valid-state.json");
    let state_path = temporary.path().join("runtime-state.json");
    fs::write(
        &target,
        br#"{"schema_version":1,"runtime_id":"rt","local_principal_id":null,"os_principal_binding_established":false,"local_incarnation_id":"rli","startup_policy":"MANUAL","recovery_state":"STOPPED","recovered_from_unclean_shutdown":false,"last_shutdown_clean":true,"blockers":[],"version":1}"#,
    )
    .expect("write valid lifecycle target");
    if let Err(error) = symlink_file(&target, &state_path) {
        if error.kind() == std::io::ErrorKind::PermissionDenied
            || error.raw_os_error() == Some(1314)
        {
            eprintln!(
                "SKIP: Windows file-symlink creation requires Developer Mode or SeCreateSymbolicLinkPrivilege"
            );
            return;
        }
        panic!("create file-symlink fixture failed: {error}");
    }

    let target_was_protected = path_dacl_is_protected(&target);
    assert!(
        read_state(&state_path).is_err(),
        "Runtime state reader must reject a reparse-point file instead of following it"
    );
    assert_eq!(
        path_dacl_is_protected(&target),
        target_was_protected,
        "rejecting the reparse path must not change its target's ACL"
    );
}
