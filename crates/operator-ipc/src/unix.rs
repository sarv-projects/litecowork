//! Owner-only Unix-domain socket transport primitives.
//!
//! This module deliberately does not decide which UID is trusted. The accept and
//! connect methods capture OS peer credentials and require the caller to approve
//! them before returning a stream capable of reading or writing frames.

use std::{
    io,
    os::unix::{
        fs::{FileTypeExt, MetadataExt, PermissionsExt},
        net::UnixListener as StdUnixListener,
    },
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use sha2::{Digest, Sha256};
use tokio::{
    net::{UnixListener, UnixStream},
    sync::{OwnedSemaphorePermit, Semaphore},
    time::timeout,
};

use crate::{InFlightBodyBudget, ProtocolError, RequestFrame, ResponseFrame, async_io};

pub const MAX_CONCURRENT_CONNECTIONS: usize = 8;
pub const CONNECT_DEADLINE: Duration = Duration::from_secs(3);
pub const HEADER_DEADLINE: Duration = Duration::from_secs(3);
pub const BODY_DEADLINE: Duration = Duration::from_secs(30);
pub const WRITE_DEADLINE: Duration = Duration::from_secs(30);

const ENDPOINT_DOMAIN: &[u8] = b"litecowork.local-operator-ipc.endpoint.v1\0";
// 103 is accepted by Linux and macOS sockaddr_un implementations.
const MAX_UNIX_SOCKET_PATH_BYTES: usize = 103;

#[derive(Debug)]
pub enum TransportError {
    Io(io::Error),
    Protocol(ProtocolError),
    InvalidPrivateDirectory,
    UnsafeEndpoint,
    EndpointAlreadyActive,
    PeerCredentialsUnavailable,
    PeerRejected,
    CapacityExceeded,
    DeadlineExceeded,
    InvalidExchangeState,
}

impl std::fmt::Display for TransportError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(_) => formatter.write_str("local Operator IPC failed"),
            Self::Protocol(error) => std::fmt::Display::fmt(error, formatter),
            Self::InvalidPrivateDirectory => {
                formatter.write_str("local Operator IPC data directory is not private")
            }
            Self::UnsafeEndpoint => formatter.write_str("local Operator IPC endpoint is unsafe"),
            Self::EndpointAlreadyActive => {
                formatter.write_str("local Operator IPC endpoint is already active")
            }
            Self::PeerCredentialsUnavailable => {
                formatter.write_str("local Operator IPC peer credentials are unavailable")
            }
            Self::PeerRejected => formatter.write_str("local Operator IPC peer was rejected"),
            Self::CapacityExceeded => formatter.write_str("local Operator IPC is at capacity"),
            Self::DeadlineExceeded => formatter.write_str("local Operator IPC deadline elapsed"),
            Self::InvalidExchangeState => {
                formatter.write_str("local Operator IPC exchange is no longer usable")
            }
        }
    }
}

impl std::error::Error for TransportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Protocol(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for TransportError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<ProtocolError> for TransportError {
    fn from(error: ProtocolError) -> Self {
        match error {
            ProtocolError::Io(error) if error.kind() == io::ErrorKind::TimedOut => {
                Self::DeadlineExceeded
            }
            other => Self::Protocol(other),
        }
    }
}

/// OS identity observed from a connected Unix socket.
///
/// The transport exposes this identity but does not decide which UID is trusted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PeerCredentials {
    pub uid: u32,
    pub gid: u32,
    pub pid: Option<u32>,
}

/// Deterministic endpoint derived from a canonical, owner-only data directory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnixEndpoint {
    private_data_dir: PathBuf,
    socket_path: PathBuf,
}

impl UnixEndpoint {
    /// Derives the shared daemon/Tauri endpoint from the canonical private data dir.
    ///
    /// The directory must already exist, be owned by the current effective UID,
    /// and have no group/other permission bits. The full path is never included in
    /// the socket filename.
    pub fn derive(private_data_dir: impl AsRef<Path>) -> Result<Self, TransportError> {
        let canonical = private_data_dir
            .as_ref()
            .canonicalize()
            .map_err(TransportError::Io)?;
        let metadata = std::fs::symlink_metadata(&canonical).map_err(TransportError::Io)?;
        if !metadata.file_type().is_dir()
            || metadata.uid() != effective_uid()
            || metadata.mode() & 0o777 != 0o700
        {
            return Err(TransportError::InvalidPrivateDirectory);
        }

        let mut digest = Sha256::new();
        digest.update(ENDPOINT_DOMAIN);
        digest.update(canonical.as_os_str().as_encoded_bytes());
        let endpoint_name = format!("operator-{}.sock", hex::encode(&digest.finalize()[..16]));
        let socket_path = canonical.join(endpoint_name);
        if socket_path.as_os_str().as_encoded_bytes().len() > MAX_UNIX_SOCKET_PATH_BYTES {
            return Err(TransportError::InvalidPrivateDirectory);
        }

        Ok(Self {
            private_data_dir: canonical,
            socket_path,
        })
    }

    /// Returns a filesystem path for native diagnostics only; never expose it to WebView.
    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    pub fn private_data_dir(&self) -> &Path {
        &self.private_data_dir
    }

    /// Binds the endpoint without replacing an active listener or unsafe stale path.
    pub fn bind(&self) -> Result<UnixIpcListener, TransportError> {
        validate_private_dir(&self.private_data_dir)?;
        prepare_stale_endpoint(&self.socket_path)?;

        let listener = StdUnixListener::bind(&self.socket_path).map_err(|error| {
            if error.kind() == io::ErrorKind::AddrInUse {
                TransportError::EndpointAlreadyActive
            } else {
                TransportError::Io(error)
            }
        })?;

        let created_identity = std::fs::symlink_metadata(&self.socket_path)
            .map(|metadata| SocketIdentity::from_metadata(&metadata));
        let result = (|| {
            let created_identity = created_identity.as_ref().map_err(|error| {
                io::Error::new(error.kind(), "could not inspect newly bound socket")
            })?;
            let metadata = std::fs::symlink_metadata(&self.socket_path)?;
            if !metadata.file_type().is_socket()
                || metadata.uid() != effective_uid()
                || !created_identity.matches(&metadata)
            {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "socket changed",
                ));
            }
            std::fs::set_permissions(&self.socket_path, std::fs::Permissions::from_mode(0o600))?;
            listener.set_nonblocking(true)?;
            let metadata = std::fs::symlink_metadata(&self.socket_path)?;
            if !metadata.file_type().is_socket()
                || metadata.uid() != effective_uid()
                || metadata.mode() & 0o777 != 0o600
            {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "unsafe socket",
                ));
            }
            Ok(SocketIdentity::from_metadata(&metadata))
        })();

        let identity = match result {
            Ok(identity) => identity,
            Err(error) => {
                drop(listener);
                if let Ok(identity) = created_identity {
                    let _ = remove_owned_socket(&self.socket_path, Some(identity));
                }
                return Err(TransportError::Io(error));
            }
        };

        let listener = match UnixListener::from_std(listener) {
            Ok(listener) => listener,
            Err(error) => {
                let _ = remove_owned_socket(&self.socket_path, Some(identity));
                return Err(TransportError::Io(error));
            }
        };
        Ok(UnixIpcListener {
            listener: Some(listener),
            socket_path: self.socket_path.clone(),
            identity,
            admission: Arc::new(Semaphore::new(MAX_CONCURRENT_CONNECTIONS)),
        })
    }

    /// Connects and validates the server's OS identity before returning a usable stream.
    pub async fn connect<F>(
        &self,
        authorize_peer: F,
    ) -> Result<AuthenticatedUnixConnection, TransportError>
    where
        F: FnOnce(PeerCredentials) -> bool,
    {
        validate_private_dir(&self.private_data_dir)?;
        validate_existing_endpoint(&self.socket_path)?;
        let stream = timeout(CONNECT_DEADLINE, UnixStream::connect(&self.socket_path))
            .await
            .map_err(|_| TransportError::DeadlineExceeded)??;
        let credentials = peer_credentials(&stream)?;
        if !authorize_peer(credentials) {
            return Err(TransportError::PeerRejected);
        }
        Ok(AuthenticatedUnixConnection::new(stream, credentials, None))
    }
}

/// Listener whose accepted streams cannot read frames until the caller approves the peer.
pub struct UnixIpcListener {
    listener: Option<UnixListener>,
    socket_path: PathBuf,
    identity: SocketIdentity,
    admission: Arc<Semaphore>,
}

impl UnixIpcListener {
    /// Accepts one authorized peer. Connections beyond the eight-slot limit are dropped.
    /// The callback runs after peer credentials are captured and before any frame read.
    pub async fn accept<F>(
        &self,
        authorize_peer: F,
    ) -> Result<AuthenticatedUnixConnection, TransportError>
    where
        F: FnOnce(PeerCredentials) -> bool,
    {
        let listener = self
            .listener
            .as_ref()
            .ok_or(TransportError::InvalidExchangeState)?;
        loop {
            let (stream, _) = listener.accept().await?;
            let permit = match Arc::clone(&self.admission).try_acquire_owned() {
                Ok(permit) => permit,
                Err(_) => {
                    drop(stream);
                    return Err(TransportError::CapacityExceeded);
                }
            };
            let credentials = match peer_credentials(&stream) {
                Ok(credentials) => credentials,
                Err(error) => {
                    drop(stream);
                    drop(permit);
                    return Err(error);
                }
            };
            if !authorize_peer(credentials) {
                drop(stream);
                drop(permit);
                return Err(TransportError::PeerRejected);
            }
            return Ok(AuthenticatedUnixConnection::new(
                stream,
                credentials,
                Some(permit),
            ));
        }
    }
}

impl Drop for UnixIpcListener {
    fn drop(&mut self) {
        // Close first, then remove only the exact socket inode created by this listener.
        drop(self.listener.take());
        let _ = remove_owned_socket(&self.socket_path, Some(self.identity));
    }
}

/// A single authenticated one-request/one-response Unix IPC exchange.
pub struct AuthenticatedUnixConnection {
    stream: UnixStream,
    peer: PeerCredentials,
    _permit: Option<OwnedSemaphorePermit>,
    state: ExchangeState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ExchangeState {
    New,
    RequestWritten(String),
    RequestRead(String),
    Complete,
    Failed,
}

impl AuthenticatedUnixConnection {
    fn new(
        stream: UnixStream,
        peer: PeerCredentials,
        permit: Option<OwnedSemaphorePermit>,
    ) -> Self {
        Self {
            stream,
            peer,
            _permit: permit,
            state: ExchangeState::New,
        }
    }

    pub fn peer_credentials(&self) -> PeerCredentials {
        self.peer
    }

    /// Reads the sole request on a server-side exchange.
    pub async fn read_request(
        &mut self,
        budget: &Arc<InFlightBodyBudget>,
    ) -> Result<RequestFrame, TransportError> {
        if self.state != ExchangeState::New {
            return Err(TransportError::InvalidExchangeState);
        }
        match async_io::read_request_with_deadlines(
            &mut self.stream,
            budget,
            HEADER_DEADLINE,
            BODY_DEADLINE,
        )
        .await
        {
            Ok(frame) => {
                self.state = ExchangeState::RequestRead(frame.header.request_id.clone());
                Ok(frame)
            }
            Err(error) => {
                self.state = ExchangeState::Failed;
                Err(error.into())
            }
        }
    }

    /// Writes the sole response, requiring it to correlate to the accepted request.
    pub async fn write_response(&mut self, frame: &ResponseFrame) -> Result<(), TransportError> {
        let ExchangeState::RequestRead(request_id) = &self.state else {
            return Err(TransportError::InvalidExchangeState);
        };
        if frame.header.request_id != *request_id {
            self.state = ExchangeState::Failed;
            return Err(TransportError::Protocol(ProtocolError::InvalidFrame));
        }
        match timeout(
            WRITE_DEADLINE,
            async_io::write_response(&mut self.stream, frame),
        )
        .await
        {
            Ok(Ok(())) => {
                self.state = ExchangeState::Complete;
                Ok(())
            }
            Ok(Err(error)) => {
                self.state = ExchangeState::Failed;
                Err(error.into())
            }
            Err(_) => {
                self.state = ExchangeState::Failed;
                Err(TransportError::DeadlineExceeded)
            }
        }
    }

    /// Writes the sole request on a client-side exchange.
    pub async fn write_request(&mut self, frame: &RequestFrame) -> Result<(), TransportError> {
        if self.state != ExchangeState::New {
            return Err(TransportError::InvalidExchangeState);
        }
        match timeout(
            WRITE_DEADLINE,
            async_io::write_request(&mut self.stream, frame),
        )
        .await
        {
            Ok(Ok(())) => {
                self.state = ExchangeState::RequestWritten(frame.header.request_id.clone());
                Ok(())
            }
            Ok(Err(error)) => {
                self.state = ExchangeState::Failed;
                Err(error.into())
            }
            Err(_) => {
                self.state = ExchangeState::Failed;
                Err(TransportError::DeadlineExceeded)
            }
        }
    }

    /// Reads the correlated response after the client has written its request.
    pub async fn read_response(
        &mut self,
        budget: &Arc<InFlightBodyBudget>,
    ) -> Result<ResponseFrame, TransportError> {
        let ExchangeState::RequestWritten(request_id) = &self.state else {
            return Err(TransportError::InvalidExchangeState);
        };
        match async_io::read_response_with_deadlines(
            &mut self.stream,
            request_id,
            budget,
            Duration::from_secs(60),
            BODY_DEADLINE,
        )
        .await
        {
            Ok(frame) => {
                self.state = ExchangeState::Complete;
                Ok(frame)
            }
            Err(error) => {
                self.state = ExchangeState::Failed;
                Err(error.into())
            }
        }
    }
}

fn peer_credentials(stream: &UnixStream) -> Result<PeerCredentials, TransportError> {
    let credentials = stream
        .peer_cred()
        .map_err(|_| TransportError::PeerCredentialsUnavailable)?;
    Ok(PeerCredentials {
        uid: credentials.uid(),
        gid: credentials.gid(),
        // rustix exposes peer PIDs as signed because some platforms use -1 for
        // unavailable. The public transport type uses an unsigned optional PID.
        pid: normalize_peer_pid(credentials.pid()),
    })
}

fn normalize_peer_pid(pid: Option<i32>) -> Option<u32> {
    pid.and_then(|pid| u32::try_from(pid).ok())
}

fn effective_uid() -> u32 {
    rustix::process::geteuid().as_raw()
}

fn validate_private_dir(path: &Path) -> Result<(), TransportError> {
    let metadata = std::fs::symlink_metadata(path).map_err(TransportError::Io)?;
    if !metadata.file_type().is_dir()
        || metadata.uid() != effective_uid()
        || metadata.mode() & 0o777 != 0o700
    {
        return Err(TransportError::InvalidPrivateDirectory);
    }
    Ok(())
}

fn prepare_stale_endpoint(path: &Path) -> Result<(), TransportError> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) => {
            if !metadata.file_type().is_socket()
                || metadata.uid() != effective_uid()
                || metadata.mode() & 0o777 != 0o600
            {
                return Err(TransportError::UnsafeEndpoint);
            }
            if endpoint_is_active_or_uncertain(path) {
                return Err(TransportError::EndpointAlreadyActive);
            }
            remove_owned_socket(path, Some(SocketIdentity::from_metadata(&metadata)))
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(TransportError::Io(error)),
    }
}

fn validate_existing_endpoint(path: &Path) -> Result<(), TransportError> {
    let metadata = std::fs::symlink_metadata(path).map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            TransportError::UnsafeEndpoint
        } else {
            TransportError::Io(error)
        }
    })?;
    if !metadata.file_type().is_socket()
        || metadata.uid() != effective_uid()
        || metadata.mode() & 0o777 != 0o600
    {
        return Err(TransportError::UnsafeEndpoint);
    }
    Ok(())
}

fn endpoint_is_active_or_uncertain(path: &Path) -> bool {
    use std::os::unix::net::UnixStream as StdUnixStream;
    match StdUnixStream::connect(path) {
        Ok(_) => true,
        Err(error) => !matches!(
            error.kind(),
            io::ErrorKind::ConnectionRefused | io::ErrorKind::NotFound
        ),
    }
}

fn remove_owned_socket(
    path: &Path,
    expected: Option<SocketIdentity>,
) -> Result<(), TransportError> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(TransportError::Io(error)),
    };
    if !metadata.file_type().is_socket()
        || metadata.uid() != effective_uid()
        || expected.is_some_and(|identity| !identity.matches(&metadata))
    {
        return Err(TransportError::UnsafeEndpoint);
    }
    std::fs::remove_file(path).map_err(TransportError::Io)
}

#[cfg(test)]
mod peer_credential_tests {
    #[test]
    fn signed_peer_pid_is_exposed_only_when_nonnegative() {
        assert_eq!(super::normalize_peer_pid(Some(17)), Some(17));
        assert_eq!(super::normalize_peer_pid(Some(-1)), None);
        assert_eq!(super::normalize_peer_pid(None), None);
    }
}

#[derive(Clone, Copy)]
struct SocketIdentity {
    device: u64,
    inode: u64,
    uid: u32,
}

impl SocketIdentity {
    fn from_metadata(metadata: &std::fs::Metadata) -> Self {
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            uid: metadata.uid(),
        }
    }

    fn matches(self, metadata: &std::fs::Metadata) -> bool {
        self.device == metadata.dev() && self.inode == metadata.ino() && self.uid == metadata.uid()
    }
}
