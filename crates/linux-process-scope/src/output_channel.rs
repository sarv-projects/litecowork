use std::io::{self, Read, Write};

pub const MAX_CANDIDATE_BYTES: usize = 1024 * 1024;
pub const MAX_LOG_FRAME_BYTES: usize = 4 * 1024;
pub const MAX_TOTAL_LOG_BYTES: usize = 64 * 1024;

const MAGIC: &[u8; 4] = b"LCOW";
const VERSION: u8 = 1;
const HEADER_BYTES: usize = 10;
const LOG_KIND: u8 = 1;
const CANDIDATE_KIND: u8 = 2;
const NO_CANDIDATE_KIND: u8 = 3;
const REJECTED_KIND: u8 = 4;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CandidateFrame {
    Candidate(Vec<u8>),
    NoCandidate,
    Rejected(CandidateRejection),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum CandidateRejection {
    InvalidCandidate = 1,
    TooLarge = 2,
    ReadFailure = 3,
}

#[derive(Debug)]
pub enum FrameError {
    Io(io::Error),
    CandidateLimit,
    Truncated,
    InvalidMagic,
    UnsupportedVersion,
    UnknownFrame,
    InvalidFrameLength,
    CandidateTooLarge,
    LogsTooLarge,
    MissingTerminal,
    DuplicateOrTrailingFrame,
    UnknownRejection,
}

impl std::fmt::Display for FrameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::Io(_) => "candidate channel I/O failed",
            Self::CandidateLimit => "candidate byte limit is outside the supported bound",
            Self::Truncated => "candidate channel ended in a partial frame",
            Self::InvalidMagic => "candidate channel magic is invalid",
            Self::UnsupportedVersion => "candidate channel version is unsupported",
            Self::UnknownFrame => "candidate channel contains an unknown frame",
            Self::InvalidFrameLength => "candidate channel frame length is invalid",
            Self::CandidateTooLarge => "candidate exceeds the accepted Step byte limit",
            Self::LogsTooLarge => "candidate channel log data exceeds its bound",
            Self::MissingTerminal => "candidate channel has no terminal frame",
            Self::DuplicateOrTrailingFrame => "candidate channel has duplicate or trailing frames",
            Self::UnknownRejection => "candidate channel rejection code is unknown",
        };
        f.write_str(message)
    }
}

impl std::error::Error for FrameError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

pub fn write_log_frame(writer: &mut impl Write, payload: &[u8]) -> io::Result<()> {
    if payload.len() > MAX_LOG_FRAME_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "candidate channel log frame exceeds its bound",
        ));
    }
    write_frame(writer, LOG_KIND, payload)
}

pub fn write_candidate_frame(writer: &mut impl Write, payload: &[u8]) -> io::Result<()> {
    if payload.len() > MAX_CANDIDATE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "candidate frame exceeds the supported bound",
        ));
    }
    write_frame(writer, CANDIDATE_KIND, payload)
}

pub fn write_no_candidate_frame(writer: &mut impl Write) -> io::Result<()> {
    write_frame(writer, NO_CANDIDATE_KIND, &[])
}

pub fn write_rejected_frame(
    writer: &mut impl Write,
    rejection: CandidateRejection,
) -> io::Result<()> {
    write_frame(writer, REJECTED_KIND, &[rejection as u8])
}

/// Read one complete, versioned channel. Callers must still independently prove the
/// owning process scope is quiescent before exposing or persisting candidate bytes.
pub fn read_candidate_channel(
    reader: &mut impl Read,
    candidate_limit: usize,
) -> Result<CandidateFrame, FrameError> {
    if candidate_limit == 0 || candidate_limit > MAX_CANDIDATE_BYTES {
        return Err(FrameError::CandidateLimit);
    }

    let mut total_log_bytes = 0usize;
    let mut terminal = None;
    loop {
        let mut first = [0u8; 1];
        match reader.read(&mut first) {
            Ok(0) if terminal.is_some() => return Ok(terminal.expect("checked terminal")),
            Ok(0) => return Err(FrameError::MissingTerminal),
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(FrameError::Io(error)),
        }
        if terminal.is_some() {
            return Err(FrameError::DuplicateOrTrailingFrame);
        }

        let mut rest = [0u8; HEADER_BYTES - 1];
        reader.read_exact(&mut rest).map_err(map_truncated)?;
        if first[0] != MAGIC[0] || &rest[..3] != &MAGIC[1..] {
            return Err(FrameError::InvalidMagic);
        }
        if rest[3] != VERSION {
            return Err(FrameError::UnsupportedVersion);
        }
        let kind = rest[4];
        let length = u32::from_be_bytes(rest[5..9].try_into().expect("fixed header")) as usize;

        match kind {
            LOG_KIND => {
                if length > MAX_LOG_FRAME_BYTES {
                    return Err(FrameError::InvalidFrameLength);
                }
                total_log_bytes = total_log_bytes
                    .checked_add(length)
                    .ok_or(FrameError::LogsTooLarge)?;
                if total_log_bytes > MAX_TOTAL_LOG_BYTES {
                    return Err(FrameError::LogsTooLarge);
                }
                discard_payload(reader, length)?;
            }
            CANDIDATE_KIND => {
                if length > candidate_limit {
                    return Err(FrameError::CandidateTooLarge);
                }
                let payload = read_payload(reader, length)?;
                terminal = Some(CandidateFrame::Candidate(payload));
            }
            NO_CANDIDATE_KIND => {
                if length != 0 {
                    return Err(FrameError::InvalidFrameLength);
                }
                terminal = Some(CandidateFrame::NoCandidate);
            }
            REJECTED_KIND => {
                if length != 1 {
                    return Err(FrameError::InvalidFrameLength);
                }
                let mut payload = [0u8; 1];
                reader.read_exact(&mut payload).map_err(map_truncated)?;
                let rejection = match payload[0] {
                    value if value == CandidateRejection::InvalidCandidate as u8 => {
                        CandidateRejection::InvalidCandidate
                    }
                    value if value == CandidateRejection::TooLarge as u8 => {
                        CandidateRejection::TooLarge
                    }
                    value if value == CandidateRejection::ReadFailure as u8 => {
                        CandidateRejection::ReadFailure
                    }
                    _ => return Err(FrameError::UnknownRejection),
                };
                terminal = Some(CandidateFrame::Rejected(rejection));
            }
            _ => return Err(FrameError::UnknownFrame),
        }
    }
}

fn write_frame(writer: &mut impl Write, kind: u8, payload: &[u8]) -> io::Result<()> {
    let length = u32::try_from(payload.len())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "frame length overflow"))?;
    writer.write_all(MAGIC)?;
    writer.write_all(&[VERSION, kind])?;
    writer.write_all(&length.to_be_bytes())?;
    writer.write_all(payload)
}

fn read_payload(reader: &mut impl Read, length: usize) -> Result<Vec<u8>, FrameError> {
    let mut payload = vec![0u8; length];
    reader.read_exact(&mut payload).map_err(map_truncated)?;
    Ok(payload)
}

fn discard_payload(reader: &mut impl Read, length: usize) -> Result<(), FrameError> {
    let mut remaining = length;
    let mut buffer = [0u8; 1024];
    while remaining > 0 {
        let next = remaining.min(buffer.len());
        reader
            .read_exact(&mut buffer[..next])
            .map_err(map_truncated)?;
        remaining -= next;
    }
    Ok(())
}

fn map_truncated(error: io::Error) -> FrameError {
    if error.kind() == io::ErrorKind::UnexpectedEof {
        FrameError::Truncated
    } else {
        FrameError::Io(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn decodes_bounded_log_frames_followed_by_one_candidate() {
        let mut encoded = Vec::new();
        write_log_frame(&mut encoded, b"harness output").unwrap();
        write_candidate_frame(&mut encoded, b"\0exact proposed bytes\xff").unwrap();

        assert_eq!(
            read_candidate_channel(&mut Cursor::new(encoded), 64).unwrap(),
            CandidateFrame::Candidate(b"\0exact proposed bytes\xff".to_vec())
        );
    }

    #[test]
    fn decodes_an_explicit_no_candidate_terminal() {
        let mut encoded = Vec::new();
        write_no_candidate_frame(&mut encoded).unwrap();

        assert_eq!(
            read_candidate_channel(&mut Cursor::new(encoded), 64).unwrap(),
            CandidateFrame::NoCandidate
        );
    }

    #[test]
    fn rejects_candidate_frames_over_the_callers_step_limit() {
        let mut encoded = Vec::new();
        write_candidate_frame(&mut encoded, b"12345").unwrap();

        assert!(read_candidate_channel(&mut Cursor::new(encoded), 4).is_err());
    }

    #[test]
    fn rejects_a_zero_candidate_limit_even_for_an_empty_candidate() {
        let mut encoded = Vec::new();
        write_candidate_frame(&mut encoded, b"").unwrap();

        assert!(matches!(
            read_candidate_channel(&mut Cursor::new(encoded), 0),
            Err(FrameError::CandidateLimit)
        ));
    }

    #[test]
    fn rejects_truncated_header_and_payload() {
        let mut encoded = Vec::new();
        write_candidate_frame(&mut encoded, b"candidate").unwrap();
        assert!(read_candidate_channel(&mut Cursor::new(&encoded[..7]), 64).is_err());
        assert!(
            read_candidate_channel(&mut Cursor::new(&encoded[..encoded.len() - 1]), 64).is_err()
        );
    }

    #[test]
    fn rejects_duplicate_terminal_and_trailing_frames() {
        let mut encoded = Vec::new();
        write_no_candidate_frame(&mut encoded).unwrap();
        write_no_candidate_frame(&mut encoded).unwrap();
        assert!(read_candidate_channel(&mut Cursor::new(encoded), 64).is_err());
    }

    #[test]
    fn rejects_a_channel_without_a_terminal_frame() {
        let mut encoded = Vec::new();
        write_log_frame(&mut encoded, b"only a log").unwrap();
        assert!(read_candidate_channel(&mut Cursor::new(encoded), 64).is_err());
    }

    #[test]
    fn rejects_malformed_and_oversized_log_frames() {
        let malformed = Vec::from(&b"NOPE\x01\x02\x00\x00\x00\x00"[..]);
        assert!(read_candidate_channel(&mut Cursor::new(malformed), 64).is_err());

        let mut oversized = Vec::new();
        write_frame(
            &mut oversized,
            LOG_KIND,
            &vec![b'x'; MAX_LOG_FRAME_BYTES + 1],
        )
        .unwrap();
        assert!(read_candidate_channel(&mut Cursor::new(oversized), 64).is_err());

        let mut cumulative = Vec::new();
        for _ in 0..=MAX_TOTAL_LOG_BYTES / MAX_LOG_FRAME_BYTES {
            write_log_frame(&mut cumulative, &vec![b'x'; MAX_LOG_FRAME_BYTES]).unwrap();
        }
        write_no_candidate_frame(&mut cumulative).unwrap();
        assert!(read_candidate_channel(&mut Cursor::new(cumulative), 64).is_err());
    }
}
