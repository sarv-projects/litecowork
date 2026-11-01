//! Owner-only Windows named-pipe transport primitives.
//!
//! The pipe's protected DACL and remote-client rejection are installed by the
//! narrowly scoped `windows-local-security` crate. A peer is authenticated by
//! the kernel's logon-SID ACL before `accept` returns a stream that can parse a
//! frame; process IDs are deliberately not used as an authorization signal.

#[cfg(any(windows, test))]
use std::io;
#[cfg(windows)]
use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

use sha2::{Digest, Sha256};
#[cfg(windows)]
use tokio::{
    net::windows::named_pipe::{NamedPipeClient, NamedPipeServer},
    sync::{OwnedSemaphorePermit, Semaphore},
    time::timeout,
};

#[cfg(windows)]
use crate::{
    InFlightBodyBudget, ProtocolError, RequestFrame, ResponseFrame, STREAM_WRITE_DEADLINE,
    StreamMessage, async_io,
};

#[cfg(windows)]
pub const MAX_CONCURRENT_CONNECTIONS: usize = 8;
#[cfg(windows)]
pub const CONNECT_DEADLINE: Duration = Duration::from_secs(3);
#[cfg(windows)]
pub const HEADER_DEADLINE: Duration = Duration::from_secs(3);
#[cfg(windows)]
pub const BODY_DEADLINE: Duration = Duration::from_secs(30);
#[cfg(windows)]
pub const WRITE_DEADLINE: Duration = Duration::from_secs(3);

const ENDPOINT_DOMAIN: &[u8] = b"litecowork.local-operator-ipc.windows-endpoint.v1\0";
const INSTANCE_CAPACITY: usize = 8;

#[derive(Debug)]
#[cfg(windows)]
pub enum TransportError {
    Io(io::Error),
    Protocol(ProtocolError),
    InvalidPrivateDirectory,
    EndpointAlreadyActive,
    PeerRejected,
    CapacityExceeded,
    DeadlineExceeded,
    InvalidExchangeState,
}

#[cfg(windows)]
impl std::fmt::Display for TransportError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(_) => formatter.write_str("local Operator IPC failed"),
            Self::Protocol(error) => std::fmt::Display::fmt(error, formatter),
            Self::InvalidPrivateDirectory => {
                formatter.write_str("local Operator IPC data directory is invalid")
            }
            Self::EndpointAlreadyActive => {
                formatter.write_str("local Operator IPC endpoint is already active")
            }
            Self::PeerRejected => {
                formatter.write_str("local Operator IPC peer disconnected during admission")
            }
            Self::CapacityExceeded => formatter.write_str("local Operator IPC is at capacity"),
            Self::DeadlineExceeded => formatter.write_str("local Operator IPC deadline elapsed"),
            Self::InvalidExchangeState => {
                formatter.write_str("local Operator IPC exchange is no longer usable")
            }
        }
    }
}

#[cfg(windows)]
impl std::error::Error for TransportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Protocol(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(windows)]
impl From<io::Error> for TransportError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

#[cfg(windows)]
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

/// Deterministic named-pipe endpoint derived from a canonical data directory.
///
/// Only the digest-derived pipe leaf is retained. Raw path material is not
/// included in the pipe name or transport error messages.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg(windows)]
pub struct WindowsEndpoint {
    private_data_dir: PathBuf,
    pipe_name: String,
}

#[cfg(windows)]
impl WindowsEndpoint {
    pub fn derive(private_data_dir: impl AsRef<Path>) -> Result<Self, TransportError> {
        let canonical = private_data_dir
            .as_ref()
            .canonicalize()
            .map_err(TransportError::Io)?;
        validate_data_directory(&canonical)?;
        use std::os::windows::ffi::OsStrExt;
        let material = canonical.as_os_str().encode_wide().collect::<Vec<_>>();
        Ok(Self {
            private_data_dir: canonical,
            pipe_name: endpoint_name_from_utf16(&material),
        })
    }

    /// Creates the first named-pipe instance. A first-instance collision fails
    /// closed; it is never replaced or treated as a stale endpoint.
    pub fn bind(&self) -> Result<WindowsIpcListener, TransportError> {
        validate_canonical_data_directory(&self.private_data_dir)?;

        let mut pending = VecDeque::with_capacity(MAX_CONCURRENT_CONNECTIONS);
        for index in 0..MAX_CONCURRENT_CONNECTIONS {
            let server = windows_local_security::create_operator_pipe(&self.pipe_name, index == 0)
                .map_err(|error| {
                    if index == 0 && error.kind() == io::ErrorKind::PermissionDenied {
                        TransportError::EndpointAlreadyActive
                    } else {
                        TransportError::Io(error)
                    }
                })?;
            pending.push_back(server);
        }

        Ok(WindowsIpcListener {
            pipe_name: self.pipe_name.clone(),
            pending: Mutex::new(pending),
            admission: Arc::new(Semaphore::new(MAX_CONCURRENT_CONNECTIONS)),
        })
    }

    /// Opens a fresh authenticated local pipe connection for one exchange.
    pub async fn connect(&self) -> Result<AuthenticatedWindowsConnection, TransportError> {
        validate_canonical_data_directory(&self.private_data_dir)?;
        let pipe_name = self.pipe_name.clone();
        let open = tokio::task::spawn_blocking(move || {
            windows_local_security::connect_operator_pipe(&pipe_name)
        });
        let client = timeout(CONNECT_DEADLINE, open)
            .await
            .map_err(|_| TransportError::DeadlineExceeded)?
            .map_err(|error| TransportError::Io(io::Error::other(error)))??;
        Ok(AuthenticatedWindowsConnection::client(client))
    }
}

/// Listener that accepts only clients admitted by the pipe's current-logon-SID
/// DACL. Each call accepts at most one connection and reserves one of eight slots.
#[cfg(windows)]
pub struct WindowsIpcListener {
    pipe_name: String,
    pending: Mutex<VecDeque<NamedPipeServer>>,
    admission: Arc<Semaphore>,
}

#[cfg(windows)]
impl WindowsIpcListener {
    pub async fn accept(&self) -> Result<AuthenticatedWindowsConnection, TransportError> {
        // Waiting here keeps a full listener pending in the daemon's select loop
        // instead of returning a ready error that would spin. The accept future
        // remains cancel-safe when shutdown wins that select.
        let permit = Arc::clone(&self.admission)
            .acquire_owned()
            .await
            .map_err(|_| TransportError::InvalidExchangeState)?;

        let server = self.next_server_slot()?;
        match server.connect().await {
            Ok(()) => Ok(AuthenticatedWindowsConnection::server(server, permit)),
            Err(error) if is_peer_disconnect_kind(error.kind()) => {
                Err(TransportError::PeerRejected)
            }
            Err(error) => Err(TransportError::Io(error)),
        }
    }

    fn next_server_slot(&self) -> Result<NamedPipeServer, TransportError> {
        let mut pending = self
            .pending
            .lock()
            .map_err(|_| TransportError::InvalidExchangeState)?;
        let desired_pending = pending_instance_target(self.admission.available_permits());
        while pending.len() < desired_pending {
            let server = windows_local_security::create_operator_pipe(&self.pipe_name, false)
                .map_err(TransportError::Io)?;
            pending.push_back(server);
        }
        pending
            .pop_front()
            .ok_or(TransportError::InvalidExchangeState)
    }
}

#[cfg(windows)]
trait AsyncPipe: tokio::io::AsyncRead + tokio::io::AsyncWrite {}
#[cfg(windows)]
impl<T> AsyncPipe for T where T: tokio::io::AsyncRead + tokio::io::AsyncWrite {}

#[cfg(windows)]
type PipeStream = Box<dyn AsyncPipe + Send + Unpin>;

/// One authenticated request/response exchange over a Windows named pipe.
#[cfg(windows)]
pub struct AuthenticatedWindowsConnection {
    stream: PipeStream,
    _permit: Option<OwnedSemaphorePermit>,
    state: ExchangeState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg(windows)]
enum ExchangeState {
    New,
    RequestWritten(String),
    RequestRead(String),
    Streaming(String),
    Complete,
    Failed,
}

#[cfg(windows)]
impl AuthenticatedWindowsConnection {
    fn client(stream: NamedPipeClient) -> Self {
        Self {
            stream: Box::new(stream),
            _permit: None,
            state: ExchangeState::New,
        }
    }

    fn server(stream: NamedPipeServer, permit: OwnedSemaphorePermit) -> Self {
        Self {
            stream: Box::new(stream),
            _permit: Some(permit),
            state: ExchangeState::New,
        }
    }

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

    pub async fn write_stream_response(
        &mut self,
        frame: &ResponseFrame,
    ) -> Result<(), TransportError> {
        let ExchangeState::RequestRead(request_id) = &self.state else {
            return Err(TransportError::InvalidExchangeState);
        };
        if frame.header.request_id != *request_id || !valid_stream_handshake(frame) {
            self.state = ExchangeState::Failed;
            return Err(TransportError::Protocol(ProtocolError::InvalidFrame));
        }
        match timeout(
            STREAM_WRITE_DEADLINE,
            async_io::write_response(&mut self.stream, frame),
        )
        .await
        {
            Ok(Ok(())) => {
                self.state = ExchangeState::Streaming(request_id.clone());
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
            HEADER_DEADLINE,
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

    pub async fn read_stream_response(
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
            HEADER_DEADLINE,
            BODY_DEADLINE,
        )
        .await
        {
            Ok(frame) if frame.header.status != 200 => {
                self.state = ExchangeState::Complete;
                Ok(frame)
            }
            Ok(frame) if valid_stream_handshake(&frame) => {
                self.state = ExchangeState::Streaming(request_id.clone());
                Ok(frame)
            }
            Ok(_) => {
                self.state = ExchangeState::Failed;
                Err(TransportError::Protocol(ProtocolError::InvalidFrame))
            }
            Err(error) => {
                self.state = ExchangeState::Failed;
                Err(error.into())
            }
        }
    }

    pub async fn read_stream_message(
        &mut self,
        budget: &Arc<InFlightBodyBudget>,
    ) -> Result<Option<StreamMessage>, TransportError> {
        if !matches!(self.state, ExchangeState::Streaming(_)) {
            return Err(TransportError::InvalidExchangeState);
        }
        match async_io::read_stream_message(&mut self.stream, budget, BODY_DEADLINE).await {
            Ok(message) => {
                if message.is_none() {
                    self.state = ExchangeState::Complete;
                }
                Ok(message)
            }
            Err(error) => {
                self.state = ExchangeState::Failed;
                Err(error.into())
            }
        }
    }

    pub async fn read_stream_message_for_session(
        &mut self,
        budget: &Arc<InFlightBodyBudget>,
    ) -> Result<Option<StreamMessage>, TransportError> {
        if !matches!(self.state, ExchangeState::Streaming(_)) {
            return Err(TransportError::InvalidExchangeState);
        }
        match async_io::read_stream_message_for_session(&mut self.stream, budget, BODY_DEADLINE)
            .await
        {
            Ok(message) => {
                if message.is_none() {
                    self.state = ExchangeState::Complete;
                }
                Ok(message)
            }
            Err(error) => {
                self.state = ExchangeState::Failed;
                Err(error.into())
            }
        }
    }

    pub async fn write_stream_message(
        &mut self,
        body: &[u8],
        budget: &Arc<InFlightBodyBudget>,
    ) -> Result<(), TransportError> {
        if !matches!(self.state, ExchangeState::Streaming(_)) {
            return Err(TransportError::InvalidExchangeState);
        }
        match timeout(
            STREAM_WRITE_DEADLINE,
            async_io::write_stream_message(&mut self.stream, body, budget),
        )
        .await
        {
            Ok(Ok(())) => Ok(()),
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

    /// Waits for the stream client to close its one-request session.
    ///
    /// The client sends no more request-side bytes after the subscription. EOF
    /// means cancellation; any additional byte is a protocol violation. The
    /// caller races this read against stream production and never writes while
    /// this mutable read is in progress.
    pub async fn wait_for_stream_disconnect(&mut self) -> Result<(), TransportError> {
        if !matches!(self.state, ExchangeState::Streaming(_)) {
            return Err(TransportError::InvalidExchangeState);
        }
        let mut byte = [0_u8; 1];
        match tokio::io::AsyncReadExt::read(&mut self.stream, &mut byte).await {
            Ok(0) => {
                self.state = ExchangeState::Complete;
                Ok(())
            }
            Ok(_) => {
                self.state = ExchangeState::Failed;
                Err(TransportError::Protocol(ProtocolError::InvalidFrame))
            }
            Err(error) => {
                self.state = ExchangeState::Failed;
                Err(error.into())
            }
        }
    }
}

#[cfg(windows)]
fn valid_stream_handshake(frame: &ResponseFrame) -> bool {
    frame.header.status == 200
        && frame.body().is_empty()
        && frame.header.headers.iter().any(|header| {
            header.name.eq_ignore_ascii_case("content-type")
                && header.value == "application/vnd.litecowork.operator-stream+json"
        })
}

#[cfg(windows)]
fn validate_data_directory(path: &Path) -> Result<(), TransportError> {
    let metadata = std::fs::symlink_metadata(path).map_err(TransportError::Io)?;
    if !metadata.file_type().is_dir() {
        return Err(TransportError::InvalidPrivateDirectory);
    }
    Ok(())
}

#[cfg(windows)]
fn validate_canonical_data_directory(path: &Path) -> Result<(), TransportError> {
    validate_data_directory(path)?;
    let canonical = path.canonicalize().map_err(TransportError::Io)?;
    if canonical != path {
        return Err(TransportError::InvalidPrivateDirectory);
    }
    Ok(())
}

fn endpoint_name_from_utf16(canonical_path: &[u16]) -> String {
    let mut digest = Sha256::new();
    digest.update(ENDPOINT_DOMAIN);
    for unit in canonical_path {
        digest.update(unit.to_le_bytes());
    }
    format!("operator-{}", hex::encode(&digest.finalize()[..16]))
}

fn pending_instance_target(available_permits_after_reservation: usize) -> usize {
    available_permits_after_reservation
        .saturating_add(1)
        .min(INSTANCE_CAPACITY)
}

fn is_peer_disconnect_kind(kind: io::ErrorKind) -> bool {
    matches!(
        kind,
        io::ErrorKind::BrokenPipe
            | io::ErrorKind::ConnectionAborted
            | io::ErrorKind::ConnectionReset
            | io::ErrorKind::NotConnected
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn endpoint_name_is_domain_separated_and_does_not_disclose_its_path() {
        let path: Vec<u16> = "C:\\Users\\Alice\\AppData\\Roaming\\LiteCowork"
            .encode_utf16()
            .collect();
        let name = super::endpoint_name_from_utf16(&path);
        assert_eq!(name, "operator-ed73cadd8b9dcaf314e31b135d6ab071");
        assert!(!name.contains("Alice"));
        let other = "C:\\Users\\Bob\\AppData\\Roaming\\LiteCowork"
            .encode_utf16()
            .collect::<Vec<_>>();
        assert_ne!(name, super::endpoint_name_from_utf16(&other));
    }

    #[test]
    fn pending_pipe_instances_never_exceed_the_eight_connection_capacity() {
        for available_permits in 0..=super::INSTANCE_CAPACITY {
            assert!(super::pending_instance_target(available_permits) <= super::INSTANCE_CAPACITY);
        }
        assert_eq!(super::pending_instance_target(0), 1);
        assert_eq!(super::pending_instance_target(7), 8);
        assert_eq!(super::pending_instance_target(8), 8);
    }

    #[test]
    fn a_client_disconnect_during_pipe_admission_is_rejected_per_connection() {
        for kind in [
            std::io::ErrorKind::BrokenPipe,
            std::io::ErrorKind::ConnectionAborted,
            std::io::ErrorKind::ConnectionReset,
            std::io::ErrorKind::NotConnected,
        ] {
            assert!(super::is_peer_disconnect_kind(kind));
        }
        for kind in [
            std::io::ErrorKind::PermissionDenied,
            std::io::ErrorKind::Other,
            std::io::ErrorKind::TimedOut,
        ] {
            assert!(!super::is_peer_disconnect_kind(kind));
        }
    }
}
