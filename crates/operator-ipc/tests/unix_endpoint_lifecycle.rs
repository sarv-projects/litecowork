#![cfg(unix)]

use operator_ipc::unix::{TransportError, UnixEndpoint};
use std::{
    fs,
    os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt},
    os::unix::net::{UnixListener, UnixStream},
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

const CHILD_DATA_DIR_ENV: &str = "LITECOWORK_OPERATOR_IPC_ACTIVE_ENDPOINT_TEST_DIR";

struct PrivateTempDir(PathBuf);

impl PrivateTempDir {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is after the Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "litecowork-ipc-active-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("private test directory is created");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700))
            .expect("directory is owner-only");
        Self(path)
    }
}

impl Drop for PrivateTempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// This test is invoked explicitly in a child test process by the parent below.
/// During an ordinary test run it has no environment variable and is a no-op.
#[test]
fn duplicate_bind_subprocess_probe() {
    let Ok(data_dir) = std::env::var(CHILD_DATA_DIR_ENV) else {
        return;
    };
    let endpoint = UnixEndpoint::derive(data_dir).expect("the parent endpoint is valid");
    assert!(matches!(
        endpoint.bind(),
        Err(TransportError::EndpointAlreadyActive)
    ));
}

#[test]
fn separate_process_cannot_replace_an_active_endpoint() {
    let directory = PrivateTempDir::new();
    let endpoint = UnixEndpoint::derive(&directory.0).expect("endpoint derives");

    // Use a standard-library listener so the parent can keep the endpoint live
    // without introducing a second Tokio runtime into the integration test.
    let listener = UnixListener::bind(endpoint.socket_path()).expect("parent binds endpoint");
    fs::set_permissions(endpoint.socket_path(), fs::Permissions::from_mode(0o600))
        .expect("socket is owner-only");
    let before = fs::symlink_metadata(endpoint.socket_path()).expect("socket metadata");
    assert!(before.file_type().is_socket());
    let before_identity = (before.dev(), before.ino(), before.uid());

    let child = Command::new(std::env::current_exe().expect("integration test executable"))
        .arg("--exact")
        .arg("duplicate_bind_subprocess_probe")
        .arg("--nocapture")
        .env(CHILD_DATA_DIR_ENV, &directory.0)
        .output()
        .expect("child test process starts");
    assert!(
        child.status.success(),
        "child process must reject an active endpoint; stdout={} stderr={}",
        String::from_utf8_lossy(&child.stdout),
        String::from_utf8_lossy(&child.stderr)
    );

    let after = fs::symlink_metadata(endpoint.socket_path()).expect("active socket remains");
    assert_eq!((after.dev(), after.ino(), after.uid()), before_identity);
    assert_eq!(after.permissions().mode() & 0o777, 0o600);

    let client = UnixStream::connect(endpoint.socket_path())
        .expect("incumbent endpoint remains connectable after the failed bind");
    let (accepted, _) = listener
        .accept()
        .expect("incumbent still accepts connections");
    drop(accepted);
    drop(client);
}
