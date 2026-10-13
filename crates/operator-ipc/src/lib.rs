//! Versioned, bounded framing shared by the native Operator and `litecoworkd`.
//!
//! This crate does not authenticate a peer or authorize an Operator command. Callers
//! must establish OS-peer identity before reading a request body and must dispatch
//! accepted requests through the normal Operator authorization path.

pub mod async_io;
#[cfg(unix)]
pub mod unix;
#[cfg(any(windows, test))]
pub mod windows;

/// True only where this crate provides an OS-authenticated local IPC transport.
/// Unsupported platforms must fail closed; no TCP or bearer fallback is provided.
pub const OS_LOCAL_IPC_SUPPORTED: bool = cfg!(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
));

/// Explicit platform gate for callers that must not substitute another transport.
pub fn require_os_local_ipc() -> Result<(), UnsupportedPlatform> {
    if OS_LOCAL_IPC_SUPPORTED {
        Ok(())
    } else {
        Err(UnsupportedPlatform)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnsupportedPlatform;

impl std::fmt::Display for UnsupportedPlatform {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("OS-authenticated local Operator IPC is unsupported on this platform")
    }
}

impl std::error::Error for UnsupportedPlatform {}

use serde::{Deserialize, Serialize};
use std::{
    convert::Infallible,
    io::{self, Read, Write},
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    task::{Context, Poll},
};

pub const PROTOCOL_VERSION: u16 = 1;
/// Native-only selection operation. It is accepted only by the authenticated local IPC
/// transport and is never mounted on an HTTP listener or included in the public API.
pub const PRIVATE_LOCAL_ROOT_SELECTION_PATH: &str = "/__litecowork_local/workspace-root-selection";
pub const MAX_HEADER_BYTES: usize = 16 * 1024;
pub const MAX_BODY_BYTES: usize = 100 * 1024 * 1024;
/// The subscription document is deliberately much smaller than ordinary request bodies.
pub const MAX_STREAM_SUBSCRIBE_BYTES: usize = 32 * 1024;
/// Maximum payload retained for one local Operator stream message.
pub const MAX_STREAM_FRAME_BYTES: usize = 1024 * 1024;
/// The stream contract bounds each output write so slow clients cannot retain a
/// Runtime worker indefinitely.
pub const STREAM_WRITE_DEADLINE: std::time::Duration = std::time::Duration::from_secs(5);
pub const MAX_REQUEST_ID_BYTES: usize = 128;
pub const MAX_PATH_BYTES: usize = 8 * 1024;
pub const MAX_HEADER_COUNT: usize = 32;
pub const MAX_HEADER_NAME_BYTES: usize = 64;
pub const MAX_HEADER_VALUE_BYTES: usize = 4 * 1024;
pub const DEFAULT_MAX_IN_FLIGHT_BODY_BYTES: usize = 128 * 1024 * 1024;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LogicalHeader {
    pub name: String,
    pub value: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RequestHeader {
    pub protocol_version: u16,
    pub request_id: String,
    pub method: String,
    pub path_and_query: String,
    pub headers: Vec<LogicalHeader>,
    pub body_length: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResponseHeader {
    pub protocol_version: u16,
    pub request_id: String,
    pub status: u16,
    pub headers: Vec<LogicalHeader>,
    pub body_length: u64,
}

#[derive(Debug)]
pub enum ProtocolError {
    Io(io::Error),
    InvalidFrame,
    UnsupportedVersion,
    LimitExceeded,
    CapacityExceeded,
}

impl std::fmt::Display for ProtocolError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(_) => formatter.write_str("local Operator IPC failed"),
            Self::InvalidFrame => formatter.write_str("local Operator IPC frame is invalid"),
            Self::UnsupportedVersion => {
                formatter.write_str("local Operator IPC version is unsupported")
            }
            Self::LimitExceeded => {
                formatter.write_str("local Operator IPC frame exceeds its limit")
            }
            Self::CapacityExceeded => formatter.write_str("local Operator IPC capacity is busy"),
        }
    }
}

impl std::error::Error for ProtocolError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for ProtocolError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

#[derive(Debug)]
pub struct RequestFrame {
    pub header: RequestHeader,
    body: bytes::Bytes,
    _permit: Option<BodyBudgetPermit>,
}

impl RequestFrame {
    pub fn body(&self) -> &[u8] {
        &self.body
    }

    /// Moves the body without copying while keeping its budget reservation alive.
    pub fn into_parts(self) -> (RequestHeader, BudgetedBody) {
        (
            self.header,
            BudgetedBody {
                bytes: self.body,
                yielded: false,
                _permit: self._permit,
            },
        )
    }
}

#[derive(Debug)]
pub struct ResponseFrame {
    pub header: ResponseHeader,
    body: bytes::Bytes,
    _permit: Option<BodyBudgetPermit>,
}

/// One bounded stream payload whose in-flight body budget remains reserved while
/// the decoded message is retained by its caller.
#[derive(Debug)]
pub struct StreamMessage {
    body: bytes::Bytes,
    _permit: BodyBudgetPermit,
}

impl StreamMessage {
    pub fn body(&self) -> &[u8] {
        &self.body
    }
}

impl ResponseFrame {
    pub fn body(&self) -> &[u8] {
        &self.body
    }

    /// Moves the body without copying while keeping its budget reservation alive.
    pub fn into_parts(self) -> (ResponseHeader, BudgetedBody) {
        (
            self.header,
            BudgetedBody {
                bytes: self.body,
                yielded: false,
                _permit: self._permit,
            },
        )
    }
}

/// Owned frame bytes whose in-flight budget remains reserved for their lifetime.
#[derive(Debug)]
pub struct BudgetedBody {
    bytes: bytes::Bytes,
    yielded: bool,
    _permit: Option<BodyBudgetPermit>,
}

impl AsRef<[u8]> for BudgetedBody {
    fn as_ref(&self) -> &[u8] {
        &self.bytes
    }
}

impl std::ops::Deref for BudgetedBody {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.bytes
    }
}

impl http_body::Body for BudgetedBody {
    type Data = bytes::Bytes;
    type Error = Infallible;

    fn poll_frame(
        self: Pin<&mut Self>,
        _context: &mut Context<'_>,
    ) -> Poll<Option<Result<http_body::Frame<Self::Data>, Self::Error>>> {
        let this = self.get_mut();
        if this.yielded {
            return Poll::Ready(None);
        }
        this.yielded = true;
        let body = std::mem::take(&mut this.bytes);
        Poll::Ready(Some(Ok(http_body::Frame::data(body))))
    }

    fn is_end_stream(&self) -> bool {
        self.yielded
    }

    fn size_hint(&self) -> http_body::SizeHint {
        let mut hint = http_body::SizeHint::new();
        hint.set_exact(self.bytes.len() as u64);
        hint
    }
}

/// Bounds the combined bodies retained by admitted request/response frames.
///
/// Inbound readers reserve before allocating. Frame constructors reserve after
/// the caller has produced its Vec, so callers that build large bodies must also
/// enforce their operation-specific allocation limit before constructing a frame.
#[derive(Debug)]
pub struct InFlightBodyBudget {
    maximum_bytes: usize,
    reserved_bytes: AtomicUsize,
}

impl InFlightBodyBudget {
    pub fn new(maximum_bytes: usize) -> Result<Arc<Self>, ProtocolError> {
        if maximum_bytes == 0 || maximum_bytes > MAX_BODY_BYTES * 8 {
            return Err(ProtocolError::LimitExceeded);
        }
        Ok(Arc::new(Self {
            maximum_bytes,
            reserved_bytes: AtomicUsize::new(0),
        }))
    }

    pub fn reserve_bytes(
        self: &Arc<Self>,
        bytes: usize,
    ) -> Result<BodyBudgetPermit, ProtocolError> {
        if bytes > MAX_BODY_BYTES {
            return Err(ProtocolError::LimitExceeded);
        }
        let result =
            self.reserved_bytes
                .fetch_update(Ordering::AcqRel, Ordering::Acquire, |reserved| {
                    reserved
                        .checked_add(bytes)
                        .filter(|next| *next <= self.maximum_bytes)
                });
        result.map_err(|_| ProtocolError::CapacityExceeded)?;
        Ok(BodyBudgetPermit {
            budget: Arc::clone(self),
            bytes,
        })
    }
}

#[derive(Debug)]
pub struct BodyBudgetPermit {
    budget: Arc<InFlightBodyBudget>,
    bytes: usize,
}

impl Drop for BodyBudgetPermit {
    fn drop(&mut self) {
        self.budget
            .reserved_bytes
            .fetch_sub(self.bytes, Ordering::AcqRel);
    }
}

impl RequestFrame {
    pub fn new(
        header: RequestHeader,
        body: Vec<u8>,
        budget: &Arc<InFlightBodyBudget>,
    ) -> Result<Self, ProtocolError> {
        validate_request(&header, body.len())?;
        let permit = budget.reserve_bytes(body.len())?;
        Ok(Self {
            header,
            body: bytes::Bytes::from(body),
            _permit: Some(permit),
        })
    }

    pub fn new_bytes(
        header: RequestHeader,
        body: bytes::Bytes,
        budget: &Arc<InFlightBodyBudget>,
    ) -> Result<Self, ProtocolError> {
        validate_request(&header, body.len())?;
        let permit = budget.reserve_bytes(body.len())?;
        Ok(Self {
            header,
            body,
            _permit: Some(permit),
        })
    }
}

impl ResponseFrame {
    pub fn new(
        header: ResponseHeader,
        body: Vec<u8>,
        budget: &Arc<InFlightBodyBudget>,
    ) -> Result<Self, ProtocolError> {
        validate_response(&header, body.len())?;
        let permit = budget.reserve_bytes(body.len())?;
        Ok(Self {
            header,
            body: bytes::Bytes::from(body),
            _permit: Some(permit),
        })
    }

    pub fn new_bytes(
        header: ResponseHeader,
        body: bytes::Bytes,
        budget: &Arc<InFlightBodyBudget>,
    ) -> Result<Self, ProtocolError> {
        validate_response(&header, body.len())?;
        let permit = budget.reserve_bytes(body.len())?;
        Ok(Self {
            header,
            body,
            _permit: Some(permit),
        })
    }

    /// Constructs a response using capacity reserved before its body was collected.
    /// The reservation may be larger than the body so the caller can bound streaming
    /// collection before it allocates the complete response.
    pub fn new_bytes_with_reservation(
        header: ResponseHeader,
        body: bytes::Bytes,
        permit: BodyBudgetPermit,
    ) -> Result<Self, ProtocolError> {
        validate_response(&header, body.len())?;
        if body.len() > permit.bytes {
            return Err(ProtocolError::LimitExceeded);
        }
        Ok(Self {
            header,
            body,
            _permit: Some(permit),
        })
    }
}

pub fn write_request<W: Write>(writer: &mut W, frame: &RequestFrame) -> Result<(), ProtocolError> {
    validate_request(&frame.header, frame.body.len())?;
    write_frame_header(writer, &frame.header)?;
    writer.write_all(&frame.body)?;
    writer.flush()?;
    Ok(())
}

pub fn read_request<R: Read>(
    reader: &mut R,
    budget: &Arc<InFlightBodyBudget>,
) -> Result<RequestFrame, ProtocolError> {
    let header: RequestHeader = read_frame_header(reader)?;
    validate_request_length(&header, header.body_length)?;
    validate_request_fields(&header)?;
    let length = usize::try_from(header.body_length).map_err(|_| ProtocolError::LimitExceeded)?;
    let permit = budget.reserve_bytes(length)?;
    let body = read_body(reader, header.body_length)?;
    validate_request(&header, body.len())?;
    Ok(RequestFrame {
        header,
        body: bytes::Bytes::from(body),
        _permit: Some(permit),
    })
}

pub fn write_response<W: Write>(
    writer: &mut W,
    frame: &ResponseFrame,
) -> Result<(), ProtocolError> {
    validate_response(&frame.header, frame.body.len())?;
    write_frame_header(writer, &frame.header)?;
    writer.write_all(&frame.body)?;
    writer.flush()?;
    Ok(())
}

pub fn read_response<R: Read>(
    reader: &mut R,
    expected_request_id: &str,
    budget: &Arc<InFlightBodyBudget>,
) -> Result<ResponseFrame, ProtocolError> {
    let header: ResponseHeader = read_frame_header(reader)?;
    validate_response_length(&header, header.body_length)?;
    validate_response_fields(&header)?;
    if header.request_id != expected_request_id {
        return Err(ProtocolError::InvalidFrame);
    }
    let length = usize::try_from(header.body_length).map_err(|_| ProtocolError::LimitExceeded)?;
    let permit = budget.reserve_bytes(length)?;
    let body = read_body(reader, header.body_length)?;
    validate_response(&header, body.len())?;
    Ok(ResponseFrame {
        header,
        body: bytes::Bytes::from(body),
        _permit: Some(permit),
    })
}

pub fn validate_request(header: &RequestHeader, body_length: usize) -> Result<(), ProtocolError> {
    validate_request_fields(header)?;
    if body_length > MAX_BODY_BYTES {
        return Err(ProtocolError::LimitExceeded);
    }
    if header.body_length != body_length as u64 {
        return Err(ProtocolError::InvalidFrame);
    }
    Ok(())
}

fn validate_request_fields(header: &RequestHeader) -> Result<(), ProtocolError> {
    if header.protocol_version != PROTOCOL_VERSION {
        return Err(ProtocolError::UnsupportedVersion);
    }
    if !valid_request_id(&header.request_id)
        || !matches!(
            header.method.as_str(),
            "GET" | "POST" | "PUT" | "PATCH" | "DELETE"
        )
        || !valid_relative_path(&header.path_and_query)
        || (header.path_and_query == PRIVATE_LOCAL_ROOT_SELECTION_PATH && header.method != "POST")
    {
        return Err(ProtocolError::InvalidFrame);
    }
    validate_headers(&header.headers, HeaderKind::Request)
}

pub fn validate_response(header: &ResponseHeader, body_length: usize) -> Result<(), ProtocolError> {
    validate_response_fields(header)?;
    if body_length > MAX_BODY_BYTES {
        return Err(ProtocolError::LimitExceeded);
    }
    if header.body_length != body_length as u64 {
        return Err(ProtocolError::InvalidFrame);
    }
    Ok(())
}

fn validate_response_fields(header: &ResponseHeader) -> Result<(), ProtocolError> {
    if header.protocol_version != PROTOCOL_VERSION {
        return Err(ProtocolError::UnsupportedVersion);
    }
    if !valid_request_id(&header.request_id) || !(100..=599).contains(&header.status) {
        return Err(ProtocolError::InvalidFrame);
    }
    if let Some(content_length) = header
        .headers
        .iter()
        .find(|header| header.name.eq_ignore_ascii_case("content-length"))
        && content_length.value.parse::<u64>().ok() != Some(header.body_length)
    {
        return Err(ProtocolError::InvalidFrame);
    }
    validate_headers(&header.headers, HeaderKind::Response)
}

fn write_frame_header<W: Write, T: Serialize>(
    writer: &mut W,
    header: &T,
) -> Result<(), ProtocolError> {
    let bytes = serde_json::to_vec(header).map_err(|_| ProtocolError::InvalidFrame)?;
    if bytes.is_empty() || bytes.len() > MAX_HEADER_BYTES {
        return Err(ProtocolError::LimitExceeded);
    }
    let length = u32::try_from(bytes.len()).map_err(|_| ProtocolError::LimitExceeded)?;
    writer.write_all(&length.to_be_bytes())?;
    writer.write_all(&bytes)?;
    Ok(())
}

fn read_frame_header<R: Read, T: for<'de> Deserialize<'de>>(
    reader: &mut R,
) -> Result<T, ProtocolError> {
    let mut length_bytes = [0_u8; 4];
    reader.read_exact(&mut length_bytes)?;
    let length = u32::from_be_bytes(length_bytes) as usize;
    if length == 0 || length > MAX_HEADER_BYTES {
        return Err(ProtocolError::LimitExceeded);
    }
    let mut bytes = vec![0_u8; length];
    reader.read_exact(&mut bytes)?;
    serde_json::from_slice(&bytes).map_err(|_| ProtocolError::InvalidFrame)
}

fn read_body<R: Read>(reader: &mut R, length: u64) -> Result<Vec<u8>, ProtocolError> {
    if length > MAX_BODY_BYTES as u64 {
        return Err(ProtocolError::LimitExceeded);
    }
    let length = usize::try_from(length).map_err(|_| ProtocolError::LimitExceeded)?;
    let mut body = vec![0_u8; length];
    reader.read_exact(&mut body)?;
    Ok(body)
}

fn validate_request_length(header: &RequestHeader, length: u64) -> Result<(), ProtocolError> {
    let maximum = if header.method == "POST" && header.path_and_query == "/v1/stream" {
        MAX_STREAM_SUBSCRIBE_BYTES as u64
    } else {
        MAX_BODY_BYTES as u64
    };
    if length > maximum || header.body_length != length {
        return Err(ProtocolError::LimitExceeded);
    }
    Ok(())
}

fn validate_response_length(header: &ResponseHeader, length: u64) -> Result<(), ProtocolError> {
    if length > MAX_BODY_BYTES as u64 || header.body_length != length {
        return Err(ProtocolError::LimitExceeded);
    }
    Ok(())
}

fn valid_request_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_REQUEST_ID_BYTES
        && value.bytes().all(|byte| byte.is_ascii_graphic())
}

fn valid_relative_path(value: &str) -> bool {
    if value == PRIVATE_LOCAL_ROOT_SELECTION_PATH {
        return true;
    }
    if value.is_empty()
        || value.len() > MAX_PATH_BYTES
        || !value.starts_with("/v1/")
        || value.starts_with("//")
        || !value.bytes().all(|byte| byte.is_ascii_graphic())
        || value
            .chars()
            .any(|character| matches!(character, '\\' | '\0' | '\r' | '\n' | '#'))
        || value.contains("://")
    {
        return false;
    }
    let path = value.split('?').next().unwrap_or_default();
    !path.split('/').any(|segment| {
        segment == "."
            || segment == ".."
            || segment.to_ascii_lowercase().contains("%2e")
            || segment.to_ascii_lowercase().contains("%2f")
            || segment.to_ascii_lowercase().contains("%5c")
    })
}

#[derive(Clone, Copy)]
enum HeaderKind {
    Request,
    Response,
}

fn validate_headers(headers: &[LogicalHeader], kind: HeaderKind) -> Result<(), ProtocolError> {
    if headers.len() > MAX_HEADER_COUNT {
        return Err(ProtocolError::LimitExceeded);
    }
    let mut seen = std::collections::HashSet::with_capacity(headers.len());
    for header in headers {
        let name = header.name.to_ascii_lowercase();
        if header.name.is_empty()
            || header.name.len() > MAX_HEADER_NAME_BYTES
            || header.value.len() > MAX_HEADER_VALUE_BYTES
            || !header
                .name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
            || header
                .value
                .bytes()
                .any(|byte| !(0x20..=0x7e).contains(&byte))
            || !seen.insert(name.clone())
        {
            return Err(ProtocolError::InvalidFrame);
        }
        let allowed = match kind {
            HeaderKind::Request => matches!(
                name.as_str(),
                "x-workspace-id"
                    | "idempotency-key"
                    | "if-match"
                    | "content-type"
                    | "content-range"
                    | "x-chunk-sha256"
            ),
            HeaderKind::Response => matches!(
                name.as_str(),
                "content-type"
                    | "content-length"
                    | "cache-control"
                    | "x-content-type-options"
                    | "x-resource-media-type"
                    | "x-resource-revision-id"
                    | "x-correlation-id"
                    | "etag"
            ),
        };
        if !allowed {
            return Err(ProtocolError::InvalidFrame);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        InFlightBodyBudget, LogicalHeader, MAX_STREAM_FRAME_BYTES, MAX_STREAM_SUBSCRIBE_BYTES,
        PROTOCOL_VERSION, ProtocolError, RequestHeader, ResponseHeader, async_io, validate_request,
        validate_request_length,
    };
    use serde::Serialize;
    use std::{future::Future, io::ErrorKind, time::Duration};
    use tokio::io::{AsyncWriteExt, DuplexStream};
    use tokio::time::timeout;

    fn request_header(
        request_id: &str,
        path_and_query: &str,
        headers: Vec<LogicalHeader>,
        body_length: u64,
    ) -> RequestHeader {
        RequestHeader {
            protocol_version: PROTOCOL_VERSION,
            request_id: request_id.to_owned(),
            method: "POST".to_owned(),
            path_and_query: path_and_query.to_owned(),
            headers,
            body_length,
        }
    }

    fn frame_bytes<T: Serialize>(header: &T, body: &[u8]) -> Vec<u8> {
        let header = serde_json::to_vec(header).expect("header serializes");
        let mut frame = Vec::with_capacity(4 + header.len() + body.len());
        frame.extend_from_slice(&(header.len() as u32).to_be_bytes());
        frame.extend_from_slice(&header);
        frame.extend_from_slice(body);
        frame
    }

    fn block_on<T>(future: impl Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .expect("test runtime builds")
            .block_on(future)
    }

    async fn feed(stream: &mut DuplexStream, bytes: &[u8]) {
        stream.write_all(bytes).await.expect("test frame writes");
    }

    #[test]
    fn request_validation_rejects_malformed_ids_paths_and_headers() {
        let cases = [
            request_header("", "/v1/tasks", Vec::new(), 0),
            request_header("request\nforged", "/v1/tasks", Vec::new(), 0),
            request_header(&"x".repeat(129), "/v1/tasks", Vec::new(), 0),
            request_header("absolute", "https://localhost/v1/tasks", Vec::new(), 0),
            request_header("traversal", "/v1/tasks/../secrets", Vec::new(), 0),
            request_header(
                "encoded-traversal",
                "/v1/tasks/%2e%2e/secrets",
                Vec::new(),
                0,
            ),
            request_header(
                "duplicate",
                "/v1/tasks",
                vec![
                    LogicalHeader {
                        name: "X-Workspace-ID".to_owned(),
                        value: "workspace-a".to_owned(),
                    },
                    LogicalHeader {
                        name: "x-workspace-id".to_owned(),
                        value: "workspace-b".to_owned(),
                    },
                ],
                0,
            ),
            request_header(
                "forbidden-header",
                "/v1/tasks",
                vec![LogicalHeader {
                    name: "Authorization".to_owned(),
                    value: "Bearer not-allowed".to_owned(),
                }],
                0,
            ),
        ];

        for header in cases {
            assert!(
                validate_request(&header, 0).is_err(),
                "unsafe request should be rejected: {header:?}"
            );
        }
    }

    #[test]
    fn request_validation_rejects_body_length_mismatch_and_unknown_protocol_version() {
        let header = request_header("length-mismatch", "/v1/tasks", Vec::new(), 4);
        assert!(matches!(
            validate_request(&header, 3),
            Err(ProtocolError::InvalidFrame)
        ));

        let mut unsupported = request_header("old-version", "/v1/tasks", Vec::new(), 0);
        unsupported.protocol_version += 1;
        assert!(matches!(
            validate_request(&unsupported, 0),
            Err(ProtocolError::UnsupportedVersion)
        ));
    }

    #[test]
    fn stream_subscription_body_has_its_own_small_transport_limit() {
        let stream_header = request_header(
            "stream-body-limit",
            "/v1/stream",
            Vec::new(),
            (MAX_STREAM_SUBSCRIBE_BYTES + 1) as u64,
        );
        assert!(matches!(
            validate_request_length(&stream_header, stream_header.body_length),
            Err(ProtocolError::LimitExceeded)
        ));

        let ordinary_header = request_header(
            "ordinary-body-limit",
            "/v1/tasks",
            Vec::new(),
            (MAX_STREAM_SUBSCRIBE_BYTES + 1) as u64,
        );
        assert!(validate_request_length(&ordinary_header, ordinary_header.body_length).is_ok());
    }

    #[test]
    fn async_request_reader_times_out_on_incomplete_header() {
        block_on(async {
            let (_writer, mut reader) = tokio::io::duplex(128);
            let budget = InFlightBodyBudget::new(32).expect("body budget");
            let result = timeout(
                Duration::from_secs(1),
                async_io::read_request_with_deadlines(
                    &mut reader,
                    &budget,
                    Duration::from_millis(10),
                    Duration::from_millis(10),
                ),
            )
            .await
            .expect("outer guard prevents a hanging regression");

            assert!(matches!(
                result,
                Err(ProtocolError::Io(error)) if error.kind() == ErrorKind::TimedOut
            ));
        });
    }

    #[test]
    fn stream_messages_are_bounded_length_prefixed_and_budgeted() {
        block_on(async {
            let (mut client, mut server) = tokio::io::duplex(MAX_STREAM_FRAME_BYTES + 16);
            let budget = std::sync::Arc::new(
                InFlightBodyBudget::new(MAX_STREAM_FRAME_BYTES * 2).expect("body budget"),
            );
            let payload = br#"{"type":"stream.ready"}"#;
            assert!(matches!(
                async_io::write_stream_message(
                    &mut client,
                    &vec![b'x'; MAX_STREAM_FRAME_BYTES + 1],
                    &budget,
                )
                .await,
                Err(ProtocolError::LimitExceeded)
            ));
            let writer_budget = budget.clone();
            let writer = tokio::spawn(async move {
                async_io::write_stream_message(&mut client, payload, &writer_budget).await
            });
            let message =
                async_io::read_stream_message(&mut server, &budget, Duration::from_secs(1))
                    .await
                    .expect("stream message reads")
                    .expect("message exists before EOF");
            writer
                .await
                .expect("writer task completes")
                .expect("stream message writes");
            assert_eq!(message.body(), payload);
        });
    }

    #[test]
    fn stalled_stream_writer_can_be_cancelled_and_releases_its_body_budget() {
        block_on(async {
            let budget = std::sync::Arc::new(InFlightBodyBudget::new(1024).expect("body budget"));
            let (mut writer, _unread_peer) = tokio::io::duplex(8);
            let payload = vec![b'x'; 512];

            let write = timeout(
                Duration::from_millis(10),
                async_io::write_stream_message(&mut writer, &payload, &budget),
            )
            .await;
            assert!(
                write.is_err(),
                "an unread peer stalls the bounded frame write"
            );

            let permit = budget
                .reserve_bytes(1024)
                .expect("cancelling a stalled write releases reserved body capacity");
            drop(permit);
        });
    }

    #[test]
    fn stream_reader_distinguishes_clean_eof_between_messages() {
        block_on(async {
            let (mut writer, mut reader) = tokio::io::duplex(16);
            writer.shutdown().await.expect("writer closes cleanly");
            let budget = InFlightBodyBudget::new(16).expect("body budget");

            let message =
                async_io::read_stream_message(&mut reader, &budget, Duration::from_secs(1))
                    .await
                    .expect("clean EOF is not a malformed frame");

            assert!(message.is_none());
        });
    }

    #[test]
    fn stream_session_reader_waits_through_idle_gaps_but_bounds_partial_frames() {
        block_on(async {
            let (mut writer, mut reader) = tokio::io::duplex(64);
            let budget = std::sync::Arc::new(InFlightBodyBudget::new(64).expect("body budget"));

            let idle = timeout(
                Duration::from_millis(10),
                async_io::read_stream_message_for_session(
                    &mut reader,
                    &budget,
                    Duration::from_millis(50),
                ),
            )
            .await;
            assert!(
                idle.is_err(),
                "an idle stream remains open without a reader timeout"
            );

            let payload = br#"{"type":"stream.ready"}"#;
            async_io::write_stream_message(&mut writer, payload, &budget)
                .await
                .expect("a later complete frame writes");
            let message = async_io::read_stream_message_for_session(
                &mut reader,
                &budget,
                Duration::from_millis(50),
            )
            .await
            .expect("frame reads after an idle gap")
            .expect("the session remains open");
            assert_eq!(message.body(), payload);

            let (mut partial_writer, mut partial_reader) = tokio::io::duplex(64);
            feed(&mut partial_writer, &[0, 0]).await;
            let partial = async_io::read_stream_message_for_session(
                &mut partial_reader,
                &budget,
                Duration::from_millis(10),
            )
            .await;
            assert!(matches!(
                partial,
                Err(ProtocolError::Io(error)) if error.kind() == ErrorKind::TimedOut
            ));
        });
    }

    #[test]
    fn stream_reader_rejects_zero_and_oversized_lengths_before_body_allocation() {
        block_on(async {
            let budget = InFlightBodyBudget::new(MAX_STREAM_FRAME_BYTES).expect("body budget");
            for (length, expected) in [
                (0_u32, ProtocolError::InvalidFrame),
                (
                    (MAX_STREAM_FRAME_BYTES as u32) + 1,
                    ProtocolError::LimitExceeded,
                ),
            ] {
                let (mut writer, mut reader) = tokio::io::duplex(8);
                feed(&mut writer, &length.to_be_bytes()).await;
                assert!(matches!(
                    async_io::read_stream_message(&mut reader, &budget, Duration::from_secs(1))
                        .await,
                    Err(error) if std::mem::discriminant(&error) == std::mem::discriminant(&expected)
                ));
                let permit = budget
                    .reserve_bytes(MAX_STREAM_FRAME_BYTES)
                    .expect("invalid lengths never reserve body capacity");
                drop(permit);
            }
        });
    }

    #[test]
    fn stream_reader_releases_budget_after_message_is_dropped() {
        block_on(async {
            let payload = br#"{"type":"stream.ready"}"#;
            let budget = InFlightBodyBudget::new(payload.len()).expect("body budget");
            let (mut writer, mut reader) = tokio::io::duplex(64);
            let writer_budget = budget.clone();
            let writer_task = tokio::spawn(async move {
                async_io::write_stream_message(&mut writer, payload, &writer_budget).await
            });
            let message =
                async_io::read_stream_message(&mut reader, &budget, Duration::from_secs(1))
                    .await
                    .expect("message read")
                    .expect("message exists");
            writer_task
                .await
                .expect("writer task completes")
                .expect("message writes");
            assert!(matches!(
                budget.reserve_bytes(1),
                Err(ProtocolError::CapacityExceeded)
            ));
            drop(message);
            let permit = budget
                .reserve_bytes(payload.len())
                .expect("dropping message releases its budget reservation");
            drop(permit);
        });
    }

    #[test]
    fn async_request_reader_times_out_on_partial_header_prefix_and_json() {
        block_on(async {
            let budget = InFlightBodyBudget::new(32).expect("body budget");
            let header = request_header("partial-header", "/v1/tasks", Vec::new(), 0);
            let encoded = frame_bytes(&header, b"");
            let prefix_length = u32::from_be_bytes(encoded[..4].try_into().expect("prefix"));
            let partial_json = [encoded[..2].to_vec(), encoded[..7].to_vec()];
            assert!(prefix_length > 3);

            for partial in partial_json {
                let (mut writer, mut reader) = tokio::io::duplex(128);
                feed(&mut writer, &partial).await;
                let result = timeout(
                    Duration::from_secs(1),
                    async_io::read_request_with_deadlines(
                        &mut reader,
                        &budget,
                        Duration::from_millis(10),
                        Duration::from_millis(10),
                    ),
                )
                .await
                .expect("outer guard prevents a hanging regression");

                assert!(matches!(
                    result,
                    Err(ProtocolError::Io(error)) if error.kind() == ErrorKind::TimedOut
                ));
            }
        });
    }

    #[test]
    fn async_request_reader_releases_reserved_capacity_after_body_timeout() {
        block_on(async {
            let (mut writer, mut reader) = tokio::io::duplex(256);
            let budget = InFlightBodyBudget::new(8).expect("body budget");
            let header = request_header("partial-body", "/v1/tasks", Vec::new(), 8);
            let encoded = frame_bytes(&header, b"abc");
            feed(&mut writer, &encoded).await;

            let result = timeout(
                Duration::from_secs(1),
                async_io::read_request_with_deadlines(
                    &mut reader,
                    &budget,
                    Duration::from_millis(100),
                    Duration::from_millis(10),
                ),
            )
            .await
            .expect("outer guard prevents a hanging regression");

            assert!(matches!(
                result,
                Err(ProtocolError::Io(error)) if error.kind() == ErrorKind::TimedOut
            ));
            let permit = budget
                .reserve_bytes(8)
                .expect("failed read released its reserved body capacity");
            drop(permit);
        });
    }

    #[test]
    fn async_request_reader_refreshes_idle_deadline_after_each_complete_chunk() {
        block_on(async {
            let (mut writer, mut reader) = tokio::io::duplex(192 * 1024);
            let budget = InFlightBodyBudget::new(192 * 1024).expect("body budget");
            let chunk = vec![b'x'; 64 * 1024];
            let body_length = (chunk.len() * 3) as u64;
            let header = request_header("chunk-progress", "/v1/tasks", Vec::new(), body_length);
            let encoded = frame_bytes(&header, b"");
            let encoded_prefix_length = 4 + usize::try_from(u32::from_be_bytes(
                encoded[..4].try_into().expect("header length prefix"),
            ))
            .expect("header length fits in memory");
            let encoded_header = &encoded[..encoded_prefix_length];
            let reader_budget = budget.clone();
            let read = tokio::spawn(async move {
                async_io::read_request_with_deadlines(
                    &mut reader,
                    &reader_budget,
                    Duration::from_millis(100),
                    Duration::from_millis(30),
                )
                .await
            });

            feed(&mut writer, encoded_header).await;
            for _ in 0..3 {
                feed(&mut writer, &chunk).await;
                tokio::time::sleep(Duration::from_millis(20)).await;
            }

            let result = timeout(Duration::from_secs(1), read)
                .await
                .expect("outer guard prevents a hanging regression")
                .expect("reader task completes");
            let frame = result.expect("each complete chunk refreshes the idle deadline");
            assert_eq!(frame.body().len(), 192 * 1024);
        });
    }

    #[test]
    fn async_request_reader_still_times_out_after_complete_progress_then_stall() {
        block_on(async {
            let (mut writer, mut reader) = tokio::io::duplex(64 * 1024 + 256);
            let body_length = 64 * 1024 + 1;
            let budget = InFlightBodyBudget::new(body_length).expect("body budget");
            let header = request_header(
                "progress-then-stall",
                "/v1/tasks",
                Vec::new(),
                body_length as u64,
            );
            let encoded = frame_bytes(&header, b"");
            let header_end =
                4 + u32::from_be_bytes(encoded[..4].try_into().expect("header length prefix"))
                    as usize;
            let reader_budget = budget.clone();
            let read = tokio::spawn(async move {
                async_io::read_request_with_deadlines(
                    &mut reader,
                    &reader_budget,
                    Duration::from_millis(100),
                    Duration::from_millis(10),
                )
                .await
            });

            feed(&mut writer, &encoded[..header_end]).await;
            feed(&mut writer, &vec![b'x'; 64 * 1024]).await;
            let result = timeout(Duration::from_secs(1), read)
                .await
                .expect("outer guard prevents a hanging regression")
                .expect("reader task completes");

            assert!(matches!(
                result,
                Err(ProtocolError::Io(error)) if error.kind() == ErrorKind::TimedOut
            ));
            let permit = budget
                .reserve_bytes(body_length)
                .expect("timeout after progress releases reserved capacity");
            drop(permit);
        });
    }

    #[test]
    fn async_response_reader_rejects_a_mismatched_request_id() {
        block_on(async {
            let (mut writer, mut reader) = tokio::io::duplex(128);
            let budget = InFlightBodyBudget::new(32).expect("body budget");
            let header = ResponseHeader {
                protocol_version: PROTOCOL_VERSION,
                request_id: "different-request".to_owned(),
                status: 200,
                headers: Vec::new(),
                body_length: 0,
            };
            feed(&mut writer, &frame_bytes(&header, b"")).await;

            let result = async_io::read_response_with_deadlines(
                &mut reader,
                "expected-request",
                &budget,
                Duration::from_millis(100),
                Duration::from_millis(100),
            )
            .await;

            assert!(matches!(result, Err(ProtocolError::InvalidFrame)));
        });
    }
}
