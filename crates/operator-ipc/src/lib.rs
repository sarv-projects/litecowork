//! Versioned, bounded framing shared by the native Operator and `litecoworkd`.
//!
//! This crate does not authenticate a peer or authorize an Operator command. Callers
//! must establish OS-peer identity before reading a request body and must dispatch
//! accepted requests through the normal Operator authorization path.

pub mod async_io;
#[cfg(unix)]
pub mod unix;

/// True only where this crate provides an OS-authenticated Unix socket transport.
/// Unsupported platforms must fail closed; no TCP or bearer fallback is provided.
pub const OS_LOCAL_IPC_SUPPORTED: bool = cfg!(any(target_os = "linux", target_os = "macos"));

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
    io::{self, Read, Write},
    convert::Infallible,
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
pub const PRIVATE_LOCAL_ROOT_SELECTION_PATH: &str =
    "/__litecowork_local/workspace-root-selection";
pub const MAX_HEADER_BYTES: usize = 16 * 1024;
pub const MAX_BODY_BYTES: usize = 100 * 1024 * 1024;
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

    pub fn reserve_bytes(self: &Arc<Self>, bytes: usize) -> Result<BodyBudgetPermit, ProtocolError> {
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
        || (header.path_and_query == PRIVATE_LOCAL_ROOT_SELECTION_PATH
            && header.method != "POST")
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
    {
        if content_length.value.parse::<u64>().ok() != Some(header.body_length) {
            return Err(ProtocolError::InvalidFrame);
        }
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
    if length > MAX_BODY_BYTES as u64 || header.body_length != length {
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
