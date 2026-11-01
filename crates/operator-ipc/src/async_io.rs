//! Tokio framing for authenticated local IPC streams.
//!
//! Callers own peer authentication, connection admission, operation limits, and
//! timeouts. Each read reserves body capacity before allocation and retains that
//! reservation in the returned frame. Cancelling or failing a read drops the
//! reservation; discard the connection afterwards because a partial frame cannot
//! safely be resumed by starting another read. A failed or cancelled write may
//! also leave a partial frame, so callers must discard the connection then too.

use std::{sync::Arc, time::Duration};

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::time::timeout;

use crate::{
    InFlightBodyBudget, MAX_HEADER_BYTES, MAX_STREAM_FRAME_BYTES, ProtocolError, RequestFrame,
    RequestHeader, ResponseFrame, ResponseHeader, StreamMessage, validate_request,
    validate_request_fields, validate_request_length, validate_response, validate_response_fields,
    validate_response_length,
};

/// Reads one request. Authenticate the peer before calling this function.
pub async fn read_request<R: AsyncRead + Unpin + ?Sized>(
    reader: &mut R,
    budget: &Arc<InFlightBodyBudget>,
) -> Result<RequestFrame, ProtocolError> {
    let header: RequestHeader = read_frame_header(reader).await?;
    validate_request_length(&header, header.body_length)?;
    validate_request_fields(&header)?;
    let length = usize::try_from(header.body_length).map_err(|_| ProtocolError::LimitExceeded)?;
    let permit = budget.reserve_bytes(length)?;
    let mut body = vec![0_u8; length];
    reader.read_exact(&mut body).await?;
    validate_request(&header, body.len())?;
    Ok(RequestFrame {
        header,
        body: bytes::Bytes::from(body),
        _permit: Some(permit),
    })
}

/// Reads one request with separate header and body deadlines.
///
/// The header deadline begins when this function is called. The body deadline is
/// an idle deadline refreshed only after each complete 64 KiB chunk. A failed or
/// timed-out read makes the stream unusable for another exchange.
pub async fn read_request_with_deadlines<R: AsyncRead + Unpin + ?Sized>(
    reader: &mut R,
    budget: &Arc<InFlightBodyBudget>,
    header_deadline: Duration,
    body_deadline: Duration,
) -> Result<RequestFrame, ProtocolError> {
    let header = timeout(header_deadline, read_frame_header(reader))
        .await
        .map_err(|_| timeout_error())??;
    validate_request_length(&header, header.body_length)?;
    validate_request_fields(&header)?;
    let length = usize::try_from(header.body_length).map_err(|_| ProtocolError::LimitExceeded)?;
    let permit = budget.reserve_bytes(length)?;
    let mut body = vec![0_u8; length];
    read_body_with_progress(reader, &mut body, body_deadline).await?;
    validate_request(&header, body.len())?;
    Ok(RequestFrame {
        header,
        body: bytes::Bytes::from(body),
        _permit: Some(permit),
    })
}

/// Reads one response correlated to the request that was sent on this connection.
pub async fn read_response<R: AsyncRead + Unpin + ?Sized>(
    reader: &mut R,
    expected_request_id: &str,
    budget: &Arc<InFlightBodyBudget>,
) -> Result<ResponseFrame, ProtocolError> {
    let header: ResponseHeader = read_frame_header(reader).await?;
    validate_response_length(&header, header.body_length)?;
    validate_response_fields(&header)?;
    if header.request_id != expected_request_id {
        return Err(ProtocolError::InvalidFrame);
    }
    let length = usize::try_from(header.body_length).map_err(|_| ProtocolError::LimitExceeded)?;
    let permit = budget.reserve_bytes(length)?;
    let mut body = vec![0_u8; length];
    reader.read_exact(&mut body).await?;
    validate_response(&header, body.len())?;
    Ok(ResponseFrame {
        header,
        body: bytes::Bytes::from(body),
        _permit: Some(permit),
    })
}

/// Reads one correlated response with separate header and body deadlines.
pub async fn read_response_with_deadlines<R: AsyncRead + Unpin + ?Sized>(
    reader: &mut R,
    expected_request_id: &str,
    budget: &Arc<InFlightBodyBudget>,
    header_deadline: Duration,
    body_deadline: Duration,
) -> Result<ResponseFrame, ProtocolError> {
    let header = timeout(header_deadline, read_frame_header(reader))
        .await
        .map_err(|_| timeout_error())??;
    validate_response_length(&header, header.body_length)?;
    validate_response_fields(&header)?;
    if header.request_id != expected_request_id {
        return Err(ProtocolError::InvalidFrame);
    }
    let length = usize::try_from(header.body_length).map_err(|_| ProtocolError::LimitExceeded)?;
    let permit = budget.reserve_bytes(length)?;
    let mut body = vec![0_u8; length];
    read_body_with_progress(reader, &mut body, body_deadline).await?;
    validate_response(&header, body.len())?;
    Ok(ResponseFrame {
        header,
        body: bytes::Bytes::from(body),
        _permit: Some(permit),
    })
}

/// Writes and flushes one request without closing the connection.
pub async fn write_request<W: AsyncWrite + Unpin + ?Sized>(
    writer: &mut W,
    frame: &RequestFrame,
) -> Result<(), ProtocolError> {
    validate_request(&frame.header, frame.body.len())?;
    write_frame_header(writer, &frame.header).await?;
    writer.write_all(&frame.body).await?;
    writer.flush().await?;
    Ok(())
}

/// Writes and flushes one response; the caller closes the exchange afterwards.
pub async fn write_response<W: AsyncWrite + Unpin + ?Sized>(
    writer: &mut W,
    frame: &ResponseFrame,
) -> Result<(), ProtocolError> {
    validate_response(&frame.header, frame.body.len())?;
    write_frame_header(writer, &frame.header).await?;
    writer.write_all(&frame.body).await?;
    writer.flush().await?;
    Ok(())
}

/// Reads one length-prefixed UTF-8/JSON stream payload. The caller validates its
/// message schema after decoding; this layer enforces the transport size and memory
/// budget before allocating the body. A failed or cancelled read requires closing
/// the connection because a partial message cannot be resumed safely.
pub async fn read_stream_message<R: AsyncRead + Unpin + ?Sized>(
    reader: &mut R,
    budget: &Arc<InFlightBodyBudget>,
    idle_deadline: Duration,
) -> Result<Option<StreamMessage>, ProtocolError> {
    let mut length_bytes = [0_u8; 4];
    let first_byte = timeout(idle_deadline, reader.read(&mut length_bytes[..1]))
        .await
        .map_err(|_| timeout_error())??;
    if first_byte == 0 {
        return Ok(None);
    }
    timeout(idle_deadline, reader.read_exact(&mut length_bytes[1..]))
        .await
        .map_err(|_| timeout_error())??;
    let length = u32::from_be_bytes(length_bytes) as usize;
    if length == 0 {
        return Err(ProtocolError::InvalidFrame);
    }
    if length > MAX_STREAM_FRAME_BYTES {
        return Err(ProtocolError::LimitExceeded);
    }

    let permit = budget.reserve_bytes(length)?;
    let mut body = vec![0_u8; length];
    read_body_with_progress(reader, &mut body, idle_deadline).await?;
    Ok(Some(StreamMessage {
        body: bytes::Bytes::from(body),
        _permit: permit,
    }))
}

/// Reads one stream message for a long-lived client session.
///
/// Waiting between frames has no idle deadline because the stream protocol does
/// not require heartbeat traffic. Once a frame starts, the remaining length prefix
/// uses `frame_deadline`, and the body uses that idle deadline refreshed after each
/// complete 64 KiB chunk. Size and in-flight memory limits match
/// [`read_stream_message`].
pub async fn read_stream_message_for_session<R: AsyncRead + Unpin + ?Sized>(
    reader: &mut R,
    budget: &Arc<InFlightBodyBudget>,
    frame_deadline: Duration,
) -> Result<Option<StreamMessage>, ProtocolError> {
    let mut length_bytes = [0_u8; 4];
    let first_byte = reader.read(&mut length_bytes[..1]).await?;
    if first_byte == 0 {
        return Ok(None);
    }
    timeout(frame_deadline, reader.read_exact(&mut length_bytes[1..]))
        .await
        .map_err(|_| timeout_error())??;
    let length = u32::from_be_bytes(length_bytes) as usize;
    if length == 0 {
        return Err(ProtocolError::InvalidFrame);
    }
    if length > MAX_STREAM_FRAME_BYTES {
        return Err(ProtocolError::LimitExceeded);
    }

    let permit = budget.reserve_bytes(length)?;
    let mut body = vec![0_u8; length];
    read_body_with_progress(reader, &mut body, frame_deadline).await?;
    Ok(Some(StreamMessage {
        body: bytes::Bytes::from(body),
        _permit: permit,
    }))
}

/// Writes one length-prefixed stream payload, reserving shared in-flight body
/// capacity for the duration of the write. Callers apply the per-session write
/// deadline and close the connection if the write fails or is cancelled.
pub async fn write_stream_message<W: AsyncWrite + Unpin + ?Sized>(
    writer: &mut W,
    body: &[u8],
    budget: &Arc<InFlightBodyBudget>,
) -> Result<(), ProtocolError> {
    if body.is_empty() {
        return Err(ProtocolError::InvalidFrame);
    }
    if body.len() > MAX_STREAM_FRAME_BYTES {
        return Err(ProtocolError::LimitExceeded);
    }
    let _permit = budget.reserve_bytes(body.len())?;
    let length = u32::try_from(body.len()).map_err(|_| ProtocolError::LimitExceeded)?;
    writer.write_all(&length.to_be_bytes()).await?;
    writer.write_all(body).await?;
    writer.flush().await?;
    Ok(())
}

async fn read_frame_header<R: AsyncRead + Unpin + ?Sized, T: for<'de> Deserialize<'de>>(
    reader: &mut R,
) -> Result<T, ProtocolError> {
    let mut length_bytes = [0_u8; 4];
    reader.read_exact(&mut length_bytes).await?;
    let length = u32::from_be_bytes(length_bytes) as usize;
    if length == 0 || length > MAX_HEADER_BYTES {
        return Err(ProtocolError::LimitExceeded);
    }
    let mut bytes = vec![0_u8; length];
    reader.read_exact(&mut bytes).await?;
    serde_json::from_slice(&bytes).map_err(|_| ProtocolError::InvalidFrame)
}

async fn write_frame_header<W: AsyncWrite + Unpin + ?Sized, T: Serialize>(
    writer: &mut W,
    header: &T,
) -> Result<(), ProtocolError> {
    let bytes = serde_json::to_vec(header).map_err(|_| ProtocolError::InvalidFrame)?;
    if bytes.is_empty() || bytes.len() > MAX_HEADER_BYTES {
        return Err(ProtocolError::LimitExceeded);
    }
    let length = u32::try_from(bytes.len()).map_err(|_| ProtocolError::LimitExceeded)?;
    writer.write_all(&length.to_be_bytes()).await?;
    writer.write_all(&bytes).await?;
    Ok(())
}

fn timeout_error() -> ProtocolError {
    ProtocolError::Io(std::io::Error::new(
        std::io::ErrorKind::TimedOut,
        "local Operator IPC deadline elapsed",
    ))
}

async fn read_body_with_progress<R: AsyncRead + Unpin + ?Sized>(
    reader: &mut R,
    body: &mut [u8],
    idle_deadline: Duration,
) -> Result<(), ProtocolError> {
    // Reset the bounded idle deadline only after a complete 64 KiB chunk arrives.
    // This bounds both slow-drip behavior and the time between actual progress.
    for chunk in body.chunks_mut(64 * 1024) {
        timeout(idle_deadline, reader.read_exact(chunk))
            .await
            .map_err(|_| timeout_error())??;
    }
    Ok(())
}
