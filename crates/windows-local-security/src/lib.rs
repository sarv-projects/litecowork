#![allow(unsafe_code)]

#[cfg(any(windows, test))]
use std::io;

/// A validated canonical Windows SID for the current logon session.
///
/// The SID is intentionally not serializable or printable via `Debug`.
#[cfg(windows)]
pub struct LogonSid(String);

#[cfg(windows)]
impl LogonSid {
    /// Returns the canonical SID string, suitable for a Windows SDDL ACE.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Obtains the process token's single `TokenLogonSid`, returning a canonical SID.
///
/// Errors are deliberately generic and never include token or OS details.
#[cfg(windows)]
pub fn current_logon_sid() -> io::Result<LogonSid> {
    win32::current_logon_sid()
}

/// Creates a local operator pipe server with a protected SYSTEM/current-logon DACL.
///
/// `name` is a local pipe leaf name, without a path or prefix. Set
/// `first_instance` when this process must be the first server instance.
#[cfg(windows)]
pub fn create_operator_pipe(
    name: &str,
    first_instance: bool,
) -> io::Result<tokio::net::windows::named_pipe::NamedPipeServer> {
    win32::create_operator_pipe(name, first_instance)
}

/// Opens a local operator named pipe client using only a local pipe leaf name.
#[cfg(windows)]
pub fn connect_operator_pipe(
    name: &str,
) -> io::Result<tokio::net::windows::named_pipe::NamedPipeClient> {
    win32::connect_operator_pipe(name)
}

/// Atomically replaces `destination` with a source file using Windows
/// write-through move semantics. The caller must flush the source file first.
/// Source and destination must be on the same volume.
#[cfg(windows)]
pub fn replace_file_write_through(
    source: &std::path::Path,
    destination: &std::path::Path,
) -> io::Result<()> {
    win32::replace_file_write_through(source, destination)
}

#[cfg(any(windows, test))]
fn parse_canonical_sid(value: &str) -> Option<(u64, Vec<u32>)> {
    let mut parts = value.split('-');
    if parts.next()? != "S" || parts.next()? != "1" {
        return None;
    }
    let authority = parse_decimal(parts.next()?, (1_u64 << 48) - 1)?;
    let subauthorities: Vec<u32> = parts
        .map(|part| parse_decimal(part, u32::MAX as u64).map(|n| n as u32))
        .collect::<Option<_>>()?;
    if subauthorities.len() > 15 || format_sid(authority, &subauthorities) != value {
        return None;
    }
    Some((authority, subauthorities))
}

#[cfg(any(windows, test))]
fn parse_decimal(value: &str, maximum: u64) -> Option<u64> {
    if value.is_empty() || (value.len() > 1 && value.starts_with('0')) {
        return None;
    }
    let parsed = value.parse::<u64>().ok()?;
    (parsed <= maximum).then_some(parsed)
}

#[cfg(any(windows, test))]
fn format_sid(authority: u64, subauthorities: &[u32]) -> String {
    let mut value = format!("S-1-{authority}");
    for subauthority in subauthorities {
        value.push('-');
        value.push_str(&subauthority.to_string());
    }
    value
}

#[cfg(any(windows, test))]
fn pipe_path(name: &str) -> io::Result<String> {
    if name.is_empty()
        || name == "."
        || name == ".."
        || name
            .chars()
            .any(|character| matches!(character, '\0' | '\\' | '/' | ':'))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid local pipe name",
        ));
    }
    Ok(format!("\\\\.\\pipe\\{name}"))
}

#[cfg(any(windows, test))]
fn format_pipe_dacl(sid: &str) -> Option<String> {
    parse_canonical_sid(sid)?;
    Some(format!("O:SYD:P(A;;GA;;;SY)(A;;GA;;;{sid})"))
}

#[cfg(any(windows, test))]
const OPERATOR_PIPE_MAX_INSTANCES: u32 = 8;

#[cfg(windows)]
mod win32 {
    use super::{
        LogonSid, OPERATOR_PIPE_MAX_INSTANCES, format_pipe_dacl, format_sid, parse_canonical_sid,
        pipe_path,
    };
    use std::{
        io,
        mem::{offset_of, size_of},
        os::windows::ffi::OsStrExt,
        path::Path,
        ptr, slice,
    };

    use tokio::net::windows::named_pipe::{
        ClientOptions, NamedPipeClient, NamedPipeServer, ServerOptions,
    };
    use windows_sys::Win32::{
        Foundation::{
            CloseHandle, ERROR_INSUFFICIENT_BUFFER, GetLastError, HANDLE, INVALID_HANDLE_VALUE,
            LocalFree,
        },
        Security::Authorization::{
            ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
        },
        Security::{
            GetTokenInformation, SECURITY_ATTRIBUTES, SID_AND_ATTRIBUTES, TOKEN_GROUPS,
            TOKEN_QUERY, TokenLogonSid,
        },
        Storage::FileSystem::{MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW},
        System::Threading::{GetCurrentProcess, OpenProcessToken},
    };

    const MAX_TOKEN_INFO_BYTES: usize = 64 * 1024;
    const SID_HEADER_BYTES: usize = 8;
    const SE_GROUP_LOGON_ID: u32 = 0xC000_0000;

    struct OwnedHandle(HANDLE);

    impl Drop for OwnedHandle {
        fn drop(&mut self) {
            if !self.0.is_null() && self.0 != INVALID_HANDLE_VALUE {
                // SAFETY: `OwnedHandle` is constructed only from a successful
                // OpenProcessToken call and owns that handle until this drop.
                unsafe { CloseHandle(self.0) };
            }
        }
    }

    struct OwnedSecurityDescriptor(*mut std::ffi::c_void);

    impl Drop for OwnedSecurityDescriptor {
        fn drop(&mut self) {
            if !self.0.is_null() {
                // SAFETY: ConvertStringSecurityDescriptorToSecurityDescriptorW
                // allocates this descriptor with LocalAlloc, so LocalFree owns it.
                unsafe { LocalFree(self.0) };
            }
        }
    }

    fn redacted_error() -> io::Error {
        io::Error::other("Windows local operator security operation failed")
    }

    pub(super) fn replace_file_write_through(source: &Path, destination: &Path) -> io::Result<()> {
        let source = path_to_windows_string(source)?;
        let destination = path_to_windows_string(destination)?;
        // SAFETY: Both input buffers are owned, NUL-terminated UTF-16 strings
        // that remain alive for the synchronous MoveFileExW call. The flags
        // request same-volume replacement with write-through; COPY_ALLOWED is
        // deliberately omitted so a non-atomic copy/delete fallback cannot run.
        let result = unsafe {
            MoveFileExW(
                source.as_ptr(),
                destination.as_ptr(),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        };
        if result == 0 {
            return Err(redacted_error());
        }
        Ok(())
    }

    fn path_to_windows_string(path: &Path) -> io::Result<Vec<u16>> {
        let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
        if wide.contains(&0) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Windows filesystem path is invalid",
            ));
        }
        wide.push(0);
        Ok(wide)
    }

    pub(super) fn current_logon_sid() -> io::Result<LogonSid> {
        let mut token = ptr::null_mut();
        // SAFETY: GetCurrentProcess returns a pseudo-handle valid for this call;
        // `token` is a valid writable HANDLE slot.
        let opened = unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) };
        if opened == 0 || token.is_null() || token == INVALID_HANDLE_VALUE {
            return Err(redacted_error());
        }
        let token = OwnedHandle(token);

        let mut required = 0_u32;
        // SAFETY: The null buffer and zero length are the documented sizing
        // query; `required` is a valid writable output pointer.
        let queried = unsafe {
            GetTokenInformation(token.0, TokenLogonSid, ptr::null_mut(), 0, &mut required)
        };
        // SAFETY: GetLastError reads the calling thread's last Win32 error from
        // the immediately preceding GetTokenInformation sizing query.
        let query_error = unsafe { GetLastError() };
        if queried != 0
            || query_error != ERROR_INSUFFICIENT_BUFFER
            || required == 0
            || required as usize > MAX_TOKEN_INFO_BYTES
        {
            return Err(redacted_error());
        }

        let mut information = vec![0_u8; required as usize];
        let mut returned = 0_u32;
        // SAFETY: `information` has `required` writable bytes and `returned` is
        // a valid output slot. The token remains owned for the call.
        let fetched = unsafe {
            GetTokenInformation(
                token.0,
                TokenLogonSid,
                information.as_mut_ptr().cast(),
                required,
                &mut returned,
            )
        };
        if fetched == 0 || returned == 0 || returned as usize > information.len() {
            return Err(redacted_error());
        }

        let actual = returned as usize;
        let groups_offset = offset_of!(TOKEN_GROUPS, Groups);
        let first_group_end = groups_offset
            .checked_add(size_of::<SID_AND_ATTRIBUTES>())
            .ok_or_else(redacted_error)?;
        if actual < first_group_end {
            return Err(redacted_error());
        }

        // SAFETY: `actual >= first_group_end` proves the GroupCount and first
        // SID_AND_ATTRIBUTES are fully within the initialized buffer.
        let group_count = unsafe { ptr::read_unaligned(information.as_ptr().cast::<u32>()) };
        if group_count != 1 {
            return Err(redacted_error());
        }
        // SAFETY: The checked offset and size above prove this entry is in-bounds.
        let entry = unsafe {
            ptr::read_unaligned(
                information
                    .as_ptr()
                    .add(groups_offset)
                    .cast::<SID_AND_ATTRIBUTES>(),
            )
        };
        if entry.Attributes & SE_GROUP_LOGON_ID != SE_GROUP_LOGON_ID || entry.Sid.is_null() {
            return Err(redacted_error());
        }

        let buffer_start = information.as_ptr() as usize;
        let buffer_end = buffer_start
            .checked_add(actual)
            .ok_or_else(redacted_error)?;
        let sid_start = entry.Sid as usize;
        if sid_start < buffer_start || sid_start >= buffer_end {
            return Err(redacted_error());
        }
        let sid_offset = sid_start - buffer_start;
        if actual - sid_offset < SID_HEADER_BYTES {
            return Err(redacted_error());
        }
        // SAFETY: The pointer was proven to start within the buffer and the
        // SID header length is checked before this bounded slice is formed.
        let header = unsafe { slice::from_raw_parts(entry.Sid.cast::<u8>(), SID_HEADER_BYTES) };
        if header[0] != 1 || header[1] > 15 {
            return Err(redacted_error());
        }
        let sid_len = SID_HEADER_BYTES + usize::from(header[1]) * size_of::<u32>();
        if actual - sid_offset < sid_len {
            return Err(redacted_error());
        }
        // SAFETY: The complete SID extent is now verified to lie in the
        // initialized token-information buffer; copy it before the buffer drops.
        let sid_bytes = unsafe { slice::from_raw_parts(entry.Sid.cast::<u8>(), sid_len) };
        let authority = sid_bytes[2..8]
            .iter()
            .fold(0_u64, |value, byte| (value << 8) | u64::from(*byte));
        let (subauthority_chunks, _) = sid_bytes[8..].as_chunks::<4>();
        let subauthorities = subauthority_chunks
            .iter()
            .map(|chunk| u32::from_le_bytes(*chunk))
            .collect::<Vec<_>>();
        let canonical = format_sid(authority, &subauthorities);
        if parse_canonical_sid(&canonical).is_none() {
            return Err(redacted_error());
        }
        Ok(LogonSid(canonical))
    }

    fn sddl_for_logon_sid(sid: &LogonSid) -> io::Result<String> {
        format_pipe_dacl(sid.as_str())
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid logon SID"))
    }

    fn security_descriptor(sddl: &str) -> io::Result<OwnedSecurityDescriptor> {
        let wide: Vec<u16> = sddl.encode_utf16().chain(std::iter::once(0)).collect();
        let mut descriptor = ptr::null_mut();
        // SAFETY: `wide` is NUL-terminated and alive during the call;
        // `descriptor` is a valid output slot.
        let converted = unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                wide.as_ptr(),
                SDDL_REVISION_1,
                &mut descriptor,
                ptr::null_mut(),
            )
        };
        if converted == 0 || descriptor.is_null() {
            return Err(redacted_error());
        }
        Ok(OwnedSecurityDescriptor(descriptor))
    }

    fn local_pipe_name(name: &str) -> io::Result<String> {
        pipe_path(name)
    }

    pub(super) fn create_operator_pipe(
        name: &str,
        first_instance: bool,
    ) -> io::Result<NamedPipeServer> {
        let name = local_pipe_name(name)?;
        let sid = current_logon_sid()?;
        let sddl = sddl_for_logon_sid(&sid)?;
        let descriptor = security_descriptor(&sddl)?;
        let mut attributes = SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor.0,
            bInheritHandle: 0,
        };
        let mut options = ServerOptions::new();
        options
            .access_inbound(true)
            .access_outbound(true)
            .max_instances(OPERATOR_PIPE_MAX_INSTANCES as usize)
            .reject_remote_clients(true)
            .first_pipe_instance(first_instance);

        // SAFETY: `attributes` and its security descriptor remain alive and
        // valid throughout the synchronous CreateNamedPipeW call. The descriptor
        // has the protected DACL constructed above, inheritance is disabled, and
        // Tokio takes ownership of the successful pipe handle before returning.
        unsafe {
            options.create_with_security_attributes_raw(
                name,
                (&mut attributes as *mut SECURITY_ATTRIBUTES).cast(),
            )
        }
        .map_err(|error| {
            io::Error::new(
                error.kind(),
                "Windows local operator security operation failed",
            )
        })
    }

    pub(super) fn connect_operator_pipe(name: &str) -> io::Result<NamedPipeClient> {
        let name = local_pipe_name(name)?;
        ClientOptions::new()
            .open(name)
            .map_err(|_| redacted_error())
    }
}

#[cfg(test)]
mod tests {
    use super::{format_pipe_dacl, format_sid, parse_canonical_sid, pipe_path};

    #[test]
    fn accepts_and_formats_canonical_sid_values() {
        let sid = "S-1-5-21-123456789-987654321-1001";
        let (authority, subauthorities) = parse_canonical_sid(sid).expect("valid SID");
        assert_eq!(format_sid(authority, &subauthorities), sid);
        assert_eq!(parse_canonical_sid("S-1-0-0"), Some((0, vec![0])));
        assert_eq!(
            parse_canonical_sid("S-1-281474976710655-4294967295"),
            Some(((1_u64 << 48) - 1, vec![u32::MAX]))
        );
    }

    #[test]
    fn rejects_malformed_or_noncanonical_sid_values() {
        for invalid in [
            "",
            "S-2-5-18",
            "s-1-5-18",
            "S-1-05-18",
            "S-1-5-018",
            "S-1-281474976710656-1",
            "S-1-5-4294967296",
            "S-1-5-1-2-3-4-5-6-7-8-9-10-11-12-13-14-15-16",
            "S-1-5-1-trailing",
        ] {
            assert_eq!(parse_canonical_sid(invalid), None, "accepted {invalid}");
        }
    }

    #[test]
    fn formats_a_protected_dacl_for_only_system_and_the_logon_sid() {
        assert_eq!(
            format_pipe_dacl("S-1-5-21-123456789-987654321-1001").as_deref(),
            Some("O:SYD:P(A;;GA;;;SY)(A;;GA;;;S-1-5-21-123456789-987654321-1001)"),
        );
        assert_eq!(format_pipe_dacl("S-1-5-invalid"), None);
    }

    #[test]
    fn local_pipe_names_cannot_select_another_namespace() {
        assert_eq!(
            pipe_path("litecowork-operator").unwrap(),
            r"\\.\pipe\litecowork-operator"
        );
        for invalid in [
            "",
            ".",
            "..",
            r"remote\pipe",
            "remote/pipe",
            r"\remote",
            "C:pipe",
            "nul\0byte",
        ] {
            assert!(pipe_path(invalid).is_err(), "accepted {invalid:?}");
        }
    }

    #[test]
    fn operator_pipe_instance_limit_matches_shared_admission_contract() {
        assert_eq!(super::OPERATOR_PIPE_MAX_INSTANCES, 8);
    }

    #[cfg(windows)]
    #[test]
    fn write_through_file_replace_creates_and_replaces_with_the_exact_bytes() {
        use std::{
            fs,
            sync::atomic::{AtomicU64, Ordering},
        };

        static SEQUENCE: AtomicU64 = AtomicU64::new(1);
        let directory = std::env::temp_dir().join(format!(
            "litecowork-durable-replace-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&directory).expect("create unique temporary directory");
        let destination = directory.join("pending.json");
        let first_source = directory.join("first.tmp");
        fs::write(&first_source, b"first generation").expect("write first source");
        super::replace_file_write_through(&first_source, &destination)
            .expect("publish first generation");
        assert_eq!(
            fs::read(&destination).expect("read first generation"),
            b"first generation"
        );

        let second_source = directory.join("second.tmp");
        fs::write(&second_source, b"second generation").expect("write second source");
        super::replace_file_write_through(&second_source, &destination)
            .expect("replace generation");
        assert_eq!(
            fs::read(&destination).expect("read replacement"),
            b"second generation"
        );
        fs::remove_dir_all(directory).expect("remove temporary directory");
    }

    #[cfg(windows)]
    #[test]
    fn write_through_file_replace_failure_preserves_the_existing_destination() {
        use std::{
            fs,
            sync::atomic::{AtomicU64, Ordering},
        };

        static SEQUENCE: AtomicU64 = AtomicU64::new(1);
        let directory = std::env::temp_dir().join(format!(
            "litecowork-durable-replace-failure-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&directory).expect("create unique temporary directory");
        let destination = directory.join("pending.json");
        let missing_source = directory.join("missing.tmp");
        fs::write(&destination, b"previous generation").expect("write existing generation");

        assert!(super::replace_file_write_through(&missing_source, &destination).is_err());
        assert_eq!(
            fs::read(&destination).expect("existing generation remains readable"),
            b"previous generation"
        );

        fs::remove_dir_all(directory).expect("remove temporary directory");
    }
}
