#![cfg(unix)]

use operator_ipc::{
    InFlightBodyBudget, LogicalHeader, PROTOCOL_VERSION, RequestFrame, RequestHeader,
    ResponseFrame, ResponseHeader,
    unix::{TransportError, UnixEndpoint},
};
use std::{
    fs,
    io::{Read, Write},
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

#[test]
fn rejected_peer_is_dropped_before_frame_admission_and_listener_remains_usable() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("test runtime builds");
    runtime.block_on(async {
        let directory = PrivateTempDir::new();
        let endpoint = UnixEndpoint::derive(&directory.0).expect("endpoint derives");
        let listener = endpoint.bind().expect("endpoint binds");

        let mut rejected_peer = UnixStream::connect(endpoint.socket_path())
            .expect("untrusted local process reaches the admission boundary");
        rejected_peer
            .set_read_timeout(Some(std::time::Duration::from_secs(2)))
            .expect("rejection observation is bounded");
        rejected_peer
            .write_all(&[0xff; 8])
            .expect("untrusted peer sends bytes before server admission");

        let (credentials_sender, credentials_receiver) = std::sync::mpsc::channel();
        let rejected = listener
            .accept(move |credentials| {
                credentials_sender
                    .send(credentials)
                    .expect("record observed peer credentials");
                false
            })
            .await;
        assert!(matches!(rejected, Err(TransportError::PeerRejected)));
        let observed = credentials_receiver
            .recv()
            .expect("peer credentials are captured before rejection");

        let mut rejected_bytes = [0; 1];
        let rejected_read = rejected_peer.read(&mut rejected_bytes);
        match rejected_read {
            Ok(0) => {}
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::BrokenPipe
                ) => {}
            result => panic!("rejected peer must observe socket closure, got {result:?}"),
        }

        let authorized_client = endpoint
            .connect(|_| true)
            .await
            .expect("a later authorized local peer still connects");
        let authorized_server = listener
            .accept(|_| true)
            .await
            .expect("rejected peer did not consume admission permanently");
        assert_eq!(observed, authorized_server.peer_credentials());
        drop((authorized_client, authorized_server));
    });
}

#[test]
fn authenticated_stream_session_has_correlated_handshake_messages_and_clean_eof() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("test runtime builds");
    runtime.block_on(async {
        let directory = PrivateTempDir::new();
        let endpoint = UnixEndpoint::derive(&directory.0).expect("endpoint derives");
        let listener = endpoint.bind().expect("endpoint binds");
        let budget = InFlightBodyBudget::new(2 * 1024 * 1024).expect("body budget");

        let mut client = endpoint.connect(|_| true).await.expect("client connects");
        let mut server = listener
            .accept(|_| true)
            .await
            .expect("authenticated test peer is accepted");
        let request = RequestFrame::new(
            RequestHeader {
                protocol_version: PROTOCOL_VERSION,
                request_id: "stream-request-1".to_owned(),
                method: "POST".to_owned(),
                path_and_query: "/v1/stream".to_owned(),
                headers: vec![
                    LogicalHeader {
                        name: "Content-Type".to_owned(),
                        value: "application/json".to_owned(),
                    },
                    LogicalHeader {
                        name: "X-Workspace-ID".to_owned(),
                        value: "workspace-a".to_owned(),
                    },
                ],
                body_length: 2,
            },
            b"{}".to_vec(),
            &budget,
        )
        .expect("subscription request is valid");

        client
            .write_request(&request)
            .await
            .expect("request writes");
        let received = server.read_request(&budget).await.expect("request reads");
        assert_eq!(received.header.request_id, "stream-request-1");

        let handshake = ResponseFrame::new(
            ResponseHeader {
                protocol_version: PROTOCOL_VERSION,
                request_id: "stream-request-1".to_owned(),
                status: 200,
                headers: vec![LogicalHeader {
                    name: "Content-Type".to_owned(),
                    value: "application/vnd.litecowork.operator-stream+json".to_owned(),
                }],
                body_length: 0,
            },
            Vec::new(),
            &budget,
        )
        .expect("stream handshake is valid");
        server
            .write_stream_response(&handshake)
            .await
            .expect("server accepts stream handshake");
        let received_handshake = client
            .read_stream_response(&budget)
            .await
            .expect("client accepts correlated stream handshake");
        assert_eq!(received_handshake.header.status, 200);

        server
            .write_stream_message(br#"{"type":"stream.ready"}"#, &budget)
            .await
            .expect("server writes stream message");
        let message = client
            .read_stream_message(&budget)
            .await
            .expect("client reads stream message")
            .expect("stream message precedes EOF");
        assert_eq!(message.body(), br#"{"type":"stream.ready"}"#);

        drop(server);
        assert!(
            client
                .read_stream_message(&budget)
                .await
                .expect("clean disconnect ends the stream")
                .is_none()
        );
        assert!(matches!(
            client.read_response(&budget).await,
            Err(operator_ipc::unix::TransportError::InvalidExchangeState)
        ));
    });
}
