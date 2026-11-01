#![allow(unsafe_code)]

#[cfg(any(windows, test))]
use std::io;
#[cfg(any(windows, test))]
use std::time::{Duration, Instant};

#[cfg(any(windows, test))]
fn wait_until_job_empty<F>(
    timeout: Duration,
    poll_interval: Duration,
    mut active_process_count: F,
) -> io::Result<()>
where
    F: FnMut() -> io::Result<u32>,
{
    if poll_interval.is_zero() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "job polling interval must be positive",
        ));
    }
    let deadline = Instant::now().checked_add(timeout).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "job quiescence timeout is out of range",
        )
    })?;
    loop {
        if active_process_count()? == 0 {
            return Ok(());
        }
        let now = Instant::now();
        if now >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "Windows process Job did not become quiescent before its deadline",
            ));
        }
        std::thread::sleep(poll_interval.min(deadline.saturating_duration_since(now)));
    }
}

#[cfg(any(windows, test))]
fn report_exit_only_after_job_empty<F>(
    status: Option<std::process::ExitStatus>,
    active_process_count: F,
) -> io::Result<Option<std::process::ExitStatus>>
where
    F: FnOnce() -> io::Result<u32>,
{
    let Some(status) = status else {
        return Ok(None);
    };
    if active_process_count()? == 0 {
        Ok(Some(status))
    } else {
        Ok(None)
    }
}

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

/// Restricts an existing local directory to its owner and SYSTEM.
/// The DACL is protected from inheritance so a permissive parent cannot widen it.
/// This is intended for LiteCowork-owned coordination directories, not user data.
#[cfg(windows)]
pub fn secure_current_user_directory(path: &std::path::Path) -> io::Result<()> {
    win32::secure_current_user_directory(path)
}

/// Restricts an existing local file to its owner and SYSTEM with a protected DACL.
#[cfg(windows)]
pub fn secure_current_user_file(path: &std::path::Path) -> io::Result<()> {
    win32::secure_current_user_file(path)
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

/// Opaque identity for a local Windows directory, using the complete volume serial and
/// 128-bit file identifier returned by the operating system. Its fields are available only
/// to Runtime code that persists them in private local bindings; it intentionally has no
/// `Debug` implementation.
#[cfg(windows)]
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct DirectoryIdentity {
    volume_serial_number: u64,
    file_id: [u8; 16],
}

#[cfg(windows)]
impl DirectoryIdentity {
    pub fn volume_serial_number(&self) -> u64 {
        self.volume_serial_number
    }

    pub fn file_id(&self) -> [u8; 16] {
        self.file_id
    }
}

/// Verified directory handle retained across a local Runtime identity check.
///
/// This handle is not a user-facing grant and must not be serialized or logged.
#[cfg(windows)]
pub struct VerifiedDirectory {
    _handle: win32::OwnedHandle,
    identity: DirectoryIdentity,
}

#[cfg(windows)]
impl VerifiedDirectory {
    pub fn identity(&self) -> DirectoryIdentity {
        self.identity
    }

    /// Reopens the same local drive-relative path and proves that it still resolves to
    /// this exact volume and 128-bit file identity.
    pub fn revalidate_path(&self, path: &std::path::Path) -> io::Result<()> {
        let current = open_directory_path_without_reparse(path)?;
        if current.identity == self.identity {
            Ok(())
        } else {
            Err(io::Error::other("Windows directory identity changed"))
        }
    }

    /// Creates a new file beneath this retained directory handle, creating missing
    /// parent directories by handle-relative, no-reparse operations. Existing files,
    /// invalid paths, and any unverified/reparse parent fail closed.
    #[cfg(windows)]
    pub fn create_new_file(&self, relative_path: &std::path::Path, bytes: &[u8]) -> io::Result<()> {
        win32::create_new_file_beneath(&self._handle, relative_path, bytes)
    }

    /// Creates a new directory beneath this retained handle, opening every child
    /// relative to its verified parent and rejecting reparse points and collisions.
    #[cfg(windows)]
    pub fn create_new_directory(
        &self,
        relative_path: &std::path::Path,
    ) -> io::Result<VerifiedDirectory> {
        win32::create_new_directory_beneath(&self._handle, relative_path)
    }

    /// Sets the readonly attribute on this already verified directory handle.
    #[cfg(windows)]
    pub fn set_read_only(&self) -> io::Result<()> {
        win32::mark_handle_read_only(&self._handle)
    }

    #[cfg(all(test, windows))]
    pub(crate) fn has_protected_system_owner_dacl(&self) -> io::Result<bool> {
        win32::has_protected_system_owner_dacl(&self._handle)
    }
}

/// Opens an absolute local-drive directory without traversing any reparse-point
/// component. UNC, WSL, device-namespace, and unknown-volume paths fail closed.
#[cfg(windows)]
pub fn open_directory_path_without_reparse(
    path: &std::path::Path,
) -> io::Result<VerifiedDirectory> {
    win32::open_directory_path_without_reparse(path)
}

/// Returns whether two validated absolute local-drive paths overlap by directory
/// ancestry, using Windows ordinal case-insensitive component comparison. UNC,
/// drive-relative, malformed, and noncanonical component paths fail closed.
#[cfg(windows)]
pub fn windows_absolute_paths_overlap(
    left: &std::path::Path,
    right: &std::path::Path,
) -> io::Result<bool> {
    win32::windows_absolute_paths_overlap(left, right)
}

/// A process suspended only after it has been assigned to a private kill-on-close Job Object.
/// Call [`ContainedChild::resume`] to begin executing the command.
#[cfg(windows)]
pub use win32::ContainedChild;

/// Starts a process suspended, assigns it to a kill-on-close Job Object, and returns it without
/// allowing its main thread to execute. If containment setup fails, the suspended process is
/// terminated and reaped; no uncontained process is returned.
#[cfg(windows)]
pub fn spawn_kill_on_drop_job(command: std::process::Command) -> io::Result<ContainedChild> {
    win32::spawn_kill_on_drop_job(command)
}

#[cfg(all(test, windows))]
fn spawn_kill_on_drop_job_fail_before_assignment(
    command: std::process::Command,
) -> io::Result<ContainedChild> {
    win32::spawn_kill_on_drop_job_fail_before_assignment(command)
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
        DirectoryIdentity, LogonSid, OPERATOR_PIPE_MAX_INSTANCES, VerifiedDirectory,
        format_pipe_dacl, format_sid, parse_canonical_sid, pipe_path,
        report_exit_only_after_job_empty, wait_until_job_empty,
    };
    use std::{
        io,
        mem::{offset_of, size_of},
        os::windows::ffi::OsStrExt,
        path::Path,
        process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, ExitStatus},
        ptr, slice,
        time::{Duration, Instant},
    };

    use tokio::net::windows::named_pipe::{
        ClientOptions, NamedPipeClient, NamedPipeServer, ServerOptions,
    };
    use windows_sys::Win32::{
        Foundation::{
            CloseHandle, ERROR_INSUFFICIENT_BUFFER, ERROR_NO_MORE_FILES, GetLastError, HANDLE,
            INVALID_HANDLE_VALUE, LocalFree, OBJ_DONT_REPARSE, STATUS_OBJECT_NAME_COLLISION,
            STATUS_OBJECT_NAME_EXISTS, UNICODE_STRING,
        },
        Globalization::{CSTR_EQUAL, CompareStringOrdinal},
        Security::Authorization::{
            ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1, SE_FILE_OBJECT,
            SetNamedSecurityInfoW,
        },
        Security::{
            DACL_SECURITY_INFORMATION, GetSecurityDescriptorDacl, GetTokenInformation,
            PROTECTED_DACL_SECURITY_INFORMATION, SECURITY_ATTRIBUTES, SID_AND_ATTRIBUTES,
            TOKEN_GROUPS, TOKEN_QUERY, TokenLogonSid,
        },
        Storage::FileSystem::{
            CreateFileW, FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_READONLY,
            FILE_ATTRIBUTE_REPARSE_POINT, FILE_ATTRIBUTE_TAG_INFO, FILE_BASIC_INFO,
            FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_ID_INFO,
            FILE_LIST_DIRECTORY, FILE_NAME_NORMALIZED, FILE_READ_ATTRIBUTES, FILE_SHARE_DELETE,
            FILE_SHARE_READ, FILE_SHARE_WRITE, FILE_WRITE_ATTRIBUTES, FileAttributeTagInfo,
            FileBasicInfo, FileIdInfo, FlushFileBuffers, GetDriveTypeW,
            GetFileInformationByHandleEx, GetFinalPathNameByHandleW, MOVEFILE_REPLACE_EXISTING,
            MOVEFILE_WRITE_THROUGH, MoveFileExW, OPEN_EXISTING, SetFileInformationByHandle,
            VOLUME_NAME_GUID, WriteFile,
        },
        System::{
            Diagnostics::ToolHelp::{
                CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First,
                Thread32Next,
            },
            JobObjects::{
                AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
                JOBOBJECT_BASIC_ACCOUNTING_INFORMATION, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
                JobObjectBasicAccountingInformation, JobObjectExtendedLimitInformation,
                QueryInformationJobObject, SetInformationJobObject, TerminateJobObject,
            },
            Threading::{
                CREATE_SUSPENDED, GetCurrentProcess, OpenProcess, OpenProcessToken, OpenThread,
                PROCESS_SET_QUOTA, PROCESS_TERMINATE, ResumeThread, THREAD_SUSPEND_RESUME,
            },
        },
    };

    use windows_sys::{
        Wdk::{
            Foundation::OBJECT_ATTRIBUTES,
            Storage::FileSystem::{
                FILE_CREATE, FILE_DIRECTORY_FILE, FILE_NON_DIRECTORY_FILE, FILE_OPEN,
                FILE_OPEN_REPARSE_POINT, FILE_SYNCHRONOUS_IO_NONALERT, FILE_WRITE_THROUGH,
                NtCreateFile,
            },
        },
        Win32::System::IO::IO_STATUS_BLOCK,
    };

    const MAX_TOKEN_INFO_BYTES: usize = 64 * 1024;
    const CHILD_WAIT_QUIESCENCE_TIMEOUT: Duration = Duration::from_secs(1);
    const CHILD_WAIT_CLEANUP_TIMEOUT: Duration = Duration::from_secs(2);
    const SID_HEADER_BYTES: usize = 8;
    const SE_GROUP_LOGON_ID: u32 = 0xC000_0000;
    const DRIVE_REMOVABLE: u32 = 2;
    const DRIVE_FIXED: u32 = 3;

    pub(super) struct OwnedHandle(HANDLE);

    // SAFETY: Win32 HANDLE values are process-wide kernel object references and
    // may be waited on, used, and closed from any thread. Ownership remains
    // unique because this wrapper is not Clone and the raw handle is private.
    unsafe impl Send for OwnedHandle {}

    impl Drop for OwnedHandle {
        fn drop(&mut self) {
            if !self.0.is_null() && self.0 != INVALID_HANDLE_VALUE {
                // SAFETY: `OwnedHandle` is constructed only from a successful
                // Win32 handle-producing call and owns that handle until drop.
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

    fn containment_error() -> io::Error {
        io::Error::other("Windows process containment setup failed")
    }

    fn relative_create_error(status: i32) -> io::Error {
        if status == STATUS_OBJECT_NAME_COLLISION || status == STATUS_OBJECT_NAME_EXISTS {
            io::Error::new(io::ErrorKind::AlreadyExists, "staging entry already exists")
        } else {
            redacted_error()
        }
    }

    pub(super) fn open_directory_path_without_reparse(
        path: &Path,
    ) -> io::Result<VerifiedDirectory> {
        let (drive_root, extended_drive_root, components) = split_local_drive_path(path)?;
        // SAFETY: `drive_root` is a validated, NUL-terminated UTF-16 drive-root string.
        let drive_type = unsafe { GetDriveTypeW(drive_root.as_ptr()) };
        if !matches!(drive_type, DRIVE_FIXED | DRIVE_REMOVABLE) {
            return Err(redacted_error());
        }
        // SAFETY: `extended_drive_root` is a validated, NUL-terminated local-drive root;
        // `CreateFileW` returns a new handle or `INVALID_HANDLE_VALUE`.
        let root = unsafe {
            CreateFileW(
                extended_drive_root.as_ptr(),
                FILE_LIST_DIRECTORY | FILE_READ_ATTRIBUTES,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                ptr::null(),
                OPEN_EXISTING,
                FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
                ptr::null_mut(),
            )
        };
        if root.is_null() || root == INVALID_HANDLE_VALUE {
            return Err(redacted_error());
        }
        let mut current = OwnedHandle(root);
        validate_directory_handle(&current)?;
        for component in components {
            let next = open_child_directory(&current, &component)?;
            validate_directory_handle(&next)?;
            current = next;
        }
        let identity = directory_identity(&current)?;
        Ok(VerifiedDirectory {
            _handle: current,
            identity,
        })
    }

    pub(super) fn windows_absolute_paths_overlap(left: &Path, right: &Path) -> io::Result<bool> {
        let left_directory = open_directory_path_without_reparse(left)?;
        let right_directory = open_directory_path_without_reparse(right)?;
        let left_final_path = final_volume_guid_path(&left_directory._handle)?;
        let right_final_path = final_volume_guid_path(&right_directory._handle)?;
        canonical_volume_paths_overlap(&left_final_path, &right_final_path)
    }

    fn final_volume_guid_path(handle: &OwnedHandle) -> io::Result<Vec<u16>> {
        let mut buffer = vec![0u16; 32_768];
        // SAFETY: the owned handle is live and `buffer` is writable for its full capacity.
        // The flags request a normalized final path on the canonical volume-GUID namespace.
        let written = unsafe {
            GetFinalPathNameByHandleW(
                handle.0,
                buffer.as_mut_ptr(),
                u32::try_from(buffer.len()).map_err(|_| redacted_error())?,
                FILE_NAME_NORMALIZED | VOLUME_NAME_GUID,
            )
        };
        if written == 0 || written as usize >= buffer.len() {
            return Err(redacted_error());
        }
        buffer.truncate(written as usize);
        if buffer.contains(&0) || String::from_utf16(&buffer).is_err() {
            return Err(redacted_error());
        }
        Ok(buffer)
    }

    fn canonical_volume_paths_overlap(left: &[u16], right: &[u16]) -> io::Result<bool> {
        let (left_volume, left_components) = split_volume_guid_final_path(left)?;
        let (right_volume, right_components) = split_volume_guid_final_path(right)?;
        if !ordinal_case_insensitive_equal(left_volume, right_volume)? {
            return Ok(false);
        }
        let common_len = left_components.len().min(right_components.len());
        for index in 0..common_len {
            if !ordinal_case_insensitive_equal(left_components[index], right_components[index])? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    #[cfg(test)]
    pub(super) fn test_canonical_volume_paths_overlap(left: &str, right: &str) -> io::Result<bool> {
        canonical_volume_paths_overlap(
            &left.encode_utf16().collect::<Vec<_>>(),
            &right.encode_utf16().collect::<Vec<_>>(),
        )
    }

    fn split_volume_guid_final_path(path: &[u16]) -> io::Result<(&[u16], Vec<&[u16]>)> {
        let prefix: Vec<u16> = r"\\?\Volume{".encode_utf16().collect();
        if path.len() <= prefix.len()
            || !ordinal_case_insensitive_equal(&path[..prefix.len()], &prefix)?
        {
            return Err(redacted_error());
        }
        const GUID_UNITS: usize = 36;
        let volume_end = prefix
            .len()
            .checked_add(GUID_UNITS + 1)
            .ok_or_else(redacted_error)?;
        if path.len() <= volume_end
            || path[volume_end - 1] != b'}' as u16
            || path[volume_end] != b'\\' as u16
        {
            return Err(redacted_error());
        }
        for (index, unit) in path[prefix.len()..prefix.len() + GUID_UNITS]
            .iter()
            .copied()
            .enumerate()
        {
            let is_separator = matches!(index, 8 | 13 | 18 | 23);
            if (is_separator && unit != b'-' as u16)
                || (!is_separator && !u8::try_from(unit).is_ok_and(|byte| byte.is_ascii_hexdigit()))
            {
                return Err(redacted_error());
            }
        }
        let volume = &path[..volume_end];
        let mut components = Vec::new();
        let mut start = volume_end + 1;
        for index in start..=path.len() {
            if index == path.len() || path[index] == b'\\' as u16 {
                if index > start {
                    components.push(&path[start..index]);
                } else if index < path.len() {
                    return Err(redacted_error());
                }
                start = index + 1;
            }
        }
        Ok((volume, components))
    }

    fn ordinal_case_insensitive_equal(left: &[u16], right: &[u16]) -> io::Result<bool> {
        let left_len = i32::try_from(left.len()).map_err(|_| redacted_error())?;
        let right_len = i32::try_from(right.len()).map_err(|_| redacted_error())?;
        // SAFETY: both pointers address valid UTF-16 slices for the explicit positive lengths;
        // CompareStringOrdinal does not retain either pointer. The path parser rejects invalid
        // surrogate sequences before these slices are compared.
        let result =
            unsafe { CompareStringOrdinal(left.as_ptr(), left_len, right.as_ptr(), right_len, 1) };
        if result == 0 {
            return Err(redacted_error());
        }
        Ok(result == CSTR_EQUAL)
    }

    fn split_local_drive_path(path: &Path) -> io::Result<(Vec<u16>, Vec<u16>, Vec<Vec<u16>>)> {
        let path: Vec<u16> = path.as_os_str().encode_wide().collect();
        if path.len() < 3
            || path.len() > 16_896
            || !matches!(path[0], 0x0041..=0x005a | 0x0061..=0x007a)
            || path[1] != b':' as u16
            || path[2] != b'\\' as u16
            || path.contains(&0)
            || path.contains(&(b'/' as u16))
            || String::from_utf16(&path).is_err()
            || (path.len() > 3 && path.last() == Some(&(b'\\' as u16)))
        {
            return Err(redacted_error());
        }
        let mut drive_root = path[..3].to_vec();
        drive_root.push(0);
        let mut extended_drive_root: Vec<u16> = "\\\\?\\".encode_utf16().collect();
        extended_drive_root.extend_from_slice(&path[..3]);
        extended_drive_root.push(0);

        let mut components = Vec::new();
        let mut start = 3;
        for index in 3..=path.len() {
            if index == path.len() || path[index] == b'\\' as u16 {
                let component = &path[start..index];
                if !component.is_empty() {
                    if component == [b'.' as u16]
                        || component == [b'.' as u16, b'.' as u16]
                        || component.iter().any(|unit| *unit == b':' as u16)
                    {
                        return Err(redacted_error());
                    }
                    let name = String::from_utf16(component).map_err(|_| redacted_error())?;
                    if name.ends_with(' ')
                        || name.ends_with('.')
                        || name.chars().any(|character| {
                            character.is_control()
                                || matches!(character, '<' | '>' | ':' | '"' | '|' | '?' | '*')
                        })
                    {
                        return Err(redacted_error());
                    }
                    components.push(component.to_vec());
                } else if index < path.len() {
                    return Err(redacted_error());
                }
                start = index + 1;
            }
        }
        Ok((drive_root, extended_drive_root, components))
    }

    fn open_child_directory(parent: &OwnedHandle, component: &[u16]) -> io::Result<OwnedHandle> {
        let byte_length = component
            .len()
            .checked_mul(size_of::<u16>())
            .and_then(|length| u16::try_from(length).ok())
            .filter(|length| *length != 0)
            .ok_or_else(redacted_error)?;
        let mut name = UNICODE_STRING {
            Length: byte_length,
            MaximumLength: byte_length,
            Buffer: component.as_ptr() as *mut u16,
        };
        let attributes = OBJECT_ATTRIBUTES {
            Length: size_of::<OBJECT_ATTRIBUTES>() as u32,
            RootDirectory: parent.0,
            ObjectName: &mut name,
            Attributes: OBJ_DONT_REPARSE,
            SecurityDescriptor: ptr::null(),
            SecurityQualityOfService: ptr::null(),
        };
        let mut io_status = IO_STATUS_BLOCK::default();
        let mut handle: HANDLE = ptr::null_mut();
        // SAFETY: the parent is an owned live directory handle. `name`, object attributes,
        // and the writable status/output slots all remain valid for the synchronous call.
        // The relative name is one validated component, opened itself with no-reparse flags.
        let status = unsafe {
            NtCreateFile(
                &mut handle,
                FILE_LIST_DIRECTORY | FILE_READ_ATTRIBUTES,
                &attributes,
                &mut io_status,
                ptr::null(),
                0,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                FILE_OPEN,
                FILE_OPEN_REPARSE_POINT,
                ptr::null(),
                0,
            )
        };
        if status < 0 || handle.is_null() || handle == INVALID_HANDLE_VALUE {
            return Err(redacted_error());
        }
        Ok(OwnedHandle(handle))
    }

    pub(super) fn validate_relative_file_path(path: &Path) -> io::Result<Vec<Vec<u16>>> {
        use std::path::Component;

        let mut components = Vec::new();
        let mut total_units = 0usize;
        for component in path.components() {
            let Component::Normal(name) = component else {
                return Err(redacted_error());
            };
            let encoded: Vec<u16> = name.encode_wide().collect();
            if encoded.is_empty()
                || encoded.len() > 255
                || encoded.contains(&0)
                || String::from_utf16(&encoded).is_err()
                || encoded
                    .iter()
                    .any(|unit| matches!(*unit, 0x002f | 0x005c | 0x003a) || *unit < 0x0020)
            {
                return Err(redacted_error());
            }
            let decoded = String::from_utf16(&encoded).map_err(|_| redacted_error())?;
            if decoded.ends_with([' ', '.'])
                || decoded
                    .chars()
                    .any(|character| matches!(character, '<' | '>' | '"' | '|' | '?' | '*'))
            {
                return Err(redacted_error());
            }
            total_units = total_units
                .checked_add(encoded.len())
                .and_then(|length| length.checked_add(usize::from(!components.is_empty())))
                .filter(|length| *length <= 16_896)
                .ok_or_else(redacted_error)?;
            components.push(encoded);
        }
        if components.is_empty() {
            return Err(redacted_error());
        }
        Ok(components)
    }

    fn create_child_directory(parent: &OwnedHandle, component: &[u16]) -> io::Result<OwnedHandle> {
        // Pass a protected owner/SYSTEM DACL into NtCreateFile itself so the newly
        // created directory is never temporarily exposed with the parent's ACL.
        let security = security_descriptor("D:P(A;OICI;FA;;;SY)(A;OICI;FA;;;OW)")?;
        let byte_length = component
            .len()
            .checked_mul(size_of::<u16>())
            .and_then(|length| u16::try_from(length).ok())
            .filter(|length| *length != 0)
            .ok_or_else(redacted_error)?;
        let mut name = UNICODE_STRING {
            Length: byte_length,
            MaximumLength: byte_length,
            Buffer: component.as_ptr() as *mut u16,
        };
        let attributes = OBJECT_ATTRIBUTES {
            Length: size_of::<OBJECT_ATTRIBUTES>() as u32,
            RootDirectory: parent.0,
            ObjectName: &mut name,
            Attributes: OBJ_DONT_REPARSE,
            SecurityDescriptor: security.0.cast(),
            SecurityQualityOfService: ptr::null(),
        };
        let mut io_status = IO_STATUS_BLOCK::default();
        let mut handle: HANDLE = ptr::null_mut();
        // SAFETY: the parent handle remains owned and live, the component was validated
        // as one UTF-16 path component, and all NT structures are initialized/aligned.
        let status = unsafe {
            NtCreateFile(
                &mut handle,
                FILE_LIST_DIRECTORY
                    | FILE_READ_ATTRIBUTES
                    | FILE_WRITE_ATTRIBUTES
                    | windows_sys::Win32::Storage::FileSystem::READ_CONTROL
                    | windows_sys::Win32::Storage::FileSystem::SYNCHRONIZE,
                &attributes,
                &mut io_status,
                ptr::null(),
                FILE_ATTRIBUTE_DIRECTORY,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                FILE_CREATE,
                FILE_DIRECTORY_FILE | FILE_OPEN_REPARSE_POINT | FILE_SYNCHRONOUS_IO_NONALERT,
                ptr::null(),
                0,
            )
        };
        if status < 0 || handle.is_null() || handle == INVALID_HANDLE_VALUE {
            return Err(relative_create_error(status));
        }
        let created = OwnedHandle(handle);
        validate_directory_handle(&created)?;
        Ok(created)
    }

    fn create_child_file(parent: &OwnedHandle, component: &[u16]) -> io::Result<OwnedHandle> {
        let security = security_descriptor("D:P(A;;FA;;;SY)(A;;FA;;;OW)")?;
        let byte_length = component
            .len()
            .checked_mul(size_of::<u16>())
            .and_then(|length| u16::try_from(length).ok())
            .filter(|length| *length != 0)
            .ok_or_else(redacted_error)?;
        let mut name = UNICODE_STRING {
            Length: byte_length,
            MaximumLength: byte_length,
            Buffer: component.as_ptr() as *mut u16,
        };
        let attributes = OBJECT_ATTRIBUTES {
            Length: size_of::<OBJECT_ATTRIBUTES>() as u32,
            RootDirectory: parent.0,
            ObjectName: &mut name,
            Attributes: OBJ_DONT_REPARSE,
            SecurityDescriptor: security.0.cast(),
            SecurityQualityOfService: ptr::null(),
        };
        let mut io_status = IO_STATUS_BLOCK::default();
        let mut handle: HANDLE = ptr::null_mut();
        // SAFETY: parent and UTF-16 name are valid for this synchronous call; FILE_CREATE
        // prevents replacement/open of an existing directory entry, and both NT flags
        // prevent traversal through a reparse point.
        let status = unsafe {
            NtCreateFile(
                &mut handle,
                windows_sys::Win32::Storage::FileSystem::FILE_WRITE_DATA
                    | FILE_READ_ATTRIBUTES
                    | FILE_WRITE_ATTRIBUTES
                    | windows_sys::Win32::Storage::FileSystem::READ_CONTROL
                    | windows_sys::Win32::Storage::FileSystem::SYNCHRONIZE,
                &attributes,
                &mut io_status,
                ptr::null(),
                0,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                FILE_CREATE,
                FILE_NON_DIRECTORY_FILE
                    | FILE_OPEN_REPARSE_POINT
                    | FILE_SYNCHRONOUS_IO_NONALERT
                    | FILE_WRITE_THROUGH,
                ptr::null(),
                0,
            )
        };
        if status < 0 || handle.is_null() || handle == INVALID_HANDLE_VALUE {
            return Err(relative_create_error(status));
        }
        Ok(OwnedHandle(handle))
    }

    pub(super) fn mark_handle_read_only(handle: &OwnedHandle) -> io::Result<()> {
        let mut basic = FILE_BASIC_INFO::default();
        // SAFETY: valid live handle and correctly sized aligned FILE_BASIC_INFO output.
        let read_ok = unsafe {
            GetFileInformationByHandleEx(
                handle.0,
                FileBasicInfo,
                (&mut basic as *mut FILE_BASIC_INFO).cast(),
                size_of::<FILE_BASIC_INFO>() as u32,
            )
        };
        if read_ok == 0 {
            return Err(redacted_error());
        }
        basic.FileAttributes |= FILE_ATTRIBUTE_READONLY;
        // SAFETY: basic was populated by the OS and only its readonly bit was changed.
        let set_ok = unsafe {
            SetFileInformationByHandle(
                handle.0,
                FileBasicInfo,
                (&basic as *const FILE_BASIC_INFO).cast(),
                size_of::<FILE_BASIC_INFO>() as u32,
            )
        };
        if set_ok == 0 {
            return Err(redacted_error());
        }
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn has_protected_system_owner_dacl(handle: &OwnedHandle) -> io::Result<bool> {
        use windows_sys::Win32::{
            Security::Authorization::{GetSecurityInfo, SE_FILE_OBJECT},
            Security::{
                ACL_SIZE_INFORMATION, AclSizeInformation, GetAclInformation,
                GetSecurityDescriptorControl, SE_DACL_PROTECTED,
            },
        };
        let mut dacl = ptr::null_mut();
        let mut descriptor = ptr::null_mut();
        // SAFETY: handle is owned/live and all output slots are initialized. The returned
        // security descriptor is released with LocalFree below; the DACL points within it.
        let status = unsafe {
            GetSecurityInfo(
                handle.0,
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION,
                ptr::null_mut(),
                ptr::null_mut(),
                &mut dacl,
                ptr::null_mut(),
                &mut descriptor,
            )
        };
        if status != 0 || descriptor.is_null() || dacl.is_null() {
            return Err(redacted_error());
        }
        let descriptor = OwnedSecurityDescriptor(descriptor);
        let mut control = 0u16;
        let mut revision = 0u32;
        // SAFETY: descriptor is live and the control/revision outputs are writable.
        if unsafe { GetSecurityDescriptorControl(descriptor.0.cast(), &mut control, &mut revision) }
            == 0
        {
            return Err(redacted_error());
        }
        let mut acl_size = ACL_SIZE_INFORMATION::default();
        // SAFETY: dacl points into the live descriptor and acl_size is correctly sized.
        if unsafe {
            GetAclInformation(
                dacl,
                (&mut acl_size as *mut ACL_SIZE_INFORMATION).cast(),
                size_of::<ACL_SIZE_INFORMATION>() as u32,
                AclSizeInformation,
            )
        } == 0
        {
            return Err(redacted_error());
        }
        Ok(control & SE_DACL_PROTECTED != 0 && acl_size.AceCount == 2)
    }

    pub(super) fn create_new_file_beneath(
        root: &OwnedHandle,
        path: &Path,
        bytes: &[u8],
    ) -> io::Result<()> {
        let components = validate_relative_file_path(path)?;
        let mut current = root;
        let mut opened_directories = Vec::with_capacity(components.len().saturating_sub(1));
        for component in &components[..components.len() - 1] {
            let next = match open_child_directory(current, component) {
                Ok(directory) => directory,
                Err(_) => create_child_directory(current, component)?,
            };
            validate_directory_handle(&next)?;
            opened_directories.push(next);
            current = opened_directories.last().expect("just pushed directory");
        }
        let file = create_child_file(current, components.last().ok_or_else(redacted_error)?)?;
        let mut written_total = 0usize;
        while written_total < bytes.len() {
            let count = (bytes.len() - written_total).min(u32::MAX as usize);
            let mut written = 0u32;
            // SAFETY: file is a live synchronous handle with FILE_WRITE_DATA access and the
            // source slice covers `count` bytes for the duration of this call.
            let ok = unsafe {
                WriteFile(
                    file.0,
                    bytes[written_total..].as_ptr(),
                    count as u32,
                    &mut written,
                    ptr::null_mut(),
                )
            };
            if ok == 0 || written == 0 || written as usize > count {
                return Err(redacted_error());
            }
            written_total += written as usize;
        }
        // SAFETY: the owned file handle is live and writable.
        if unsafe { FlushFileBuffers(file.0) } == 0 {
            return Err(redacted_error());
        }
        mark_handle_read_only(&file)?;
        Ok(())
    }

    pub(super) fn create_new_directory_beneath(
        root: &OwnedHandle,
        path: &Path,
    ) -> io::Result<VerifiedDirectory> {
        let components = validate_relative_file_path(path)?;
        let mut current = root;
        let mut opened_directories = Vec::with_capacity(components.len());
        for component in &components {
            let next = create_child_directory(current, component)?;
            validate_directory_handle(&next)?;
            opened_directories.push(next);
            current = opened_directories.last().expect("just pushed directory");
        }
        let directory = opened_directories.pop().ok_or_else(redacted_error)?;
        let identity = directory_identity(&directory)?;
        Ok(VerifiedDirectory {
            _handle: directory,
            identity,
        })
    }

    fn validate_directory_handle(handle: &OwnedHandle) -> io::Result<()> {
        let mut attributes = FILE_ATTRIBUTE_TAG_INFO::default();
        // SAFETY: `handle` owns a live filesystem handle and the correctly sized, aligned
        // output structure is writable for the duration of the call.
        let ok = unsafe {
            GetFileInformationByHandleEx(
                handle.0,
                FileAttributeTagInfo,
                (&mut attributes as *mut FILE_ATTRIBUTE_TAG_INFO).cast(),
                size_of::<FILE_ATTRIBUTE_TAG_INFO>() as u32,
            )
        };
        if ok == 0
            || attributes.FileAttributes & FILE_ATTRIBUTE_DIRECTORY == 0
            || attributes.FileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
        {
            return Err(redacted_error());
        }
        Ok(())
    }

    fn directory_identity(handle: &OwnedHandle) -> io::Result<DirectoryIdentity> {
        let mut information = FILE_ID_INFO::default();
        // SAFETY: `handle` owns a live filesystem handle and the correctly sized, aligned
        // output structure is writable for the duration of the call.
        let ok = unsafe {
            GetFileInformationByHandleEx(
                handle.0,
                FileIdInfo,
                (&mut information as *mut FILE_ID_INFO).cast(),
                size_of::<FILE_ID_INFO>() as u32,
            )
        };
        if ok == 0
            || information.VolumeSerialNumber == 0
            || information.FileId.Identifier == [0; 16]
        {
            return Err(redacted_error());
        }
        Ok(DirectoryIdentity {
            volume_serial_number: information.VolumeSerialNumber,
            file_id: information.FileId.Identifier,
        })
    }

    pub struct ContainedChild {
        child: Child,
        job: OwnedHandle,
        primary_thread: OwnedHandle,
        exit_status: Option<ExitStatus>,
        resumed: bool,
        finished: bool,
    }

    impl ContainedChild {
        pub fn id(&self) -> u32 {
            self.child.id()
        }

        pub fn resume(&mut self) -> io::Result<()> {
            if self.resumed || self.finished {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "contained process cannot be resumed in its current state",
                ));
            }

            // SAFETY: `primary_thread` is an owned thread handle opened while
            // the process was suspended and retained for this transition.
            let previous_suspend_count = unsafe { ResumeThread(self.primary_thread.0) };
            if previous_suspend_count != 1 {
                self.terminate_and_reap();
                return Err(containment_error());
            }
            self.resumed = true;
            Ok(())
        }

        pub fn take_stdout(&mut self) -> io::Result<Option<ChildStdout>> {
            if !self.resumed || self.finished {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "contained process output is unavailable before resume",
                ));
            }
            Ok(self.child.stdout.take())
        }

        pub fn take_stdin(&mut self) -> io::Result<Option<ChildStdin>> {
            if !self.resumed || self.finished {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "contained process input is unavailable before resume",
                ));
            }
            Ok(self.child.stdin.take())
        }

        pub fn take_stderr(&mut self) -> io::Result<Option<ChildStderr>> {
            if !self.resumed || self.finished {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "contained process error output is unavailable before resume",
                ));
            }
            Ok(self.child.stderr.take())
        }

        pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
            if !self.resumed || self.finished {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "contained process cannot be waited in its current state",
                ));
            }
            let status = match &self.exit_status {
                Some(status) => Some(status.clone()),
                None => self.child.try_wait()?,
            };
            if let Some(status) = status {
                self.exit_status = Some(status.clone());
                let settled = report_exit_only_after_job_empty(Some(status), || {
                    active_job_process_count(&self.job)
                })?;
                if settled.is_none() {
                    return Ok(None);
                }
                self.finished = true;
                return Ok(settled);
            }
            Ok(None)
        }

        pub fn wait(&mut self) -> io::Result<ExitStatus> {
            if !self.resumed || self.finished {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "contained process cannot be waited in its current state",
                ));
            }
            let deadline = Instant::now()
                .checked_add(CHILD_WAIT_QUIESCENCE_TIMEOUT)
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "process wait timeout out of range",
                    )
                })?;
            loop {
                match self.try_wait() {
                    Ok(Some(status)) => return Ok(status),
                    Ok(None) if Instant::now() < deadline => {
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    Ok(None) => {
                        let timeout_error = io::Error::new(
                            io::ErrorKind::TimedOut,
                            "contained process tree did not settle before its deadline",
                        );
                        return self.terminate_reap_and_confirm(timeout_error);
                    }
                    Err(error) => return self.terminate_reap_and_confirm(error),
                }
            }
        }

        fn terminate_reap_and_confirm(&mut self, error: io::Error) -> io::Result<ExitStatus> {
            if self.kill_tree().is_err() {
                let _ = self.child.kill();
                let _ = self.child.wait();
                return Err(error);
            }
            let status = self.child.wait()?;
            self.exit_status = Some(status.clone());
            match self.wait_for_job_quiescence(CHILD_WAIT_CLEANUP_TIMEOUT) {
                Ok(()) if error.kind() == io::ErrorKind::TimedOut => {
                    self.finished = true;
                    Ok(status)
                }
                Ok(()) => {
                    self.finished = true;
                    Err(error)
                }
                Err(_) => Err(error),
            }
        }

        pub fn kill_tree(&mut self) -> io::Result<()> {
            // SAFETY: `job` is a private Job Object created by this crate and
            // configured with KILL_ON_JOB_CLOSE before any child was assigned.
            let terminated = unsafe { TerminateJobObject(self.job.0, 1) };
            if terminated == 0 {
                return Err(containment_error());
            }
            Ok(())
        }

        /// Waits until the Job Object reports no active processes. Direct-child exit
        /// alone is not evidence that descendants have stopped. Query failures and
        /// deadline expiry both return errors, so callers must keep writer authority
        /// fenced unless this exact process tree reaches zero.
        pub fn wait_for_job_quiescence(&self, timeout: Duration) -> io::Result<()> {
            wait_until_job_empty(timeout, Duration::from_millis(10), || {
                active_job_process_count(&self.job)
            })
        }

        fn terminate_and_reap(&mut self) {
            // The process is still suspended until `resume` succeeds. Closing
            // the Job Object terminates any assigned process even if the API
            // call itself fails; Child::kill is the direct-child fallback.
            // SAFETY: `job` is the private Job Object owned by this value.
            unsafe { TerminateJobObject(self.job.0, 1) };
            let _ = self.child.kill();
            let _ = self.child.wait();
            self.finished = true;
        }
    }

    fn active_job_process_count(job: &OwnedHandle) -> io::Result<u32> {
        let mut information = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
        let mut returned_length = 0_u32;
        // SAFETY: `job` owns a live Job Object handle. The output structure and
        // returned-length slot are writable and sized for this information class.
        let queried = unsafe {
            QueryInformationJobObject(
                job.0,
                JobObjectBasicAccountingInformation,
                (&mut information as *mut JOBOBJECT_BASIC_ACCOUNTING_INFORMATION).cast(),
                size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32,
                &mut returned_length,
            )
        };
        if queried == 0
            || returned_length < size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32
        {
            return Err(containment_error());
        }
        Ok(information.ActiveProcesses)
    }

    pub(super) fn spawn_kill_on_drop_job(mut command: Command) -> io::Result<ContainedChild> {
        spawn_kill_on_drop_job_inner(&mut command, false)
    }

    #[cfg(test)]
    pub(super) fn spawn_kill_on_drop_job_fail_before_assignment(
        mut command: Command,
    ) -> io::Result<ContainedChild> {
        spawn_kill_on_drop_job_inner(&mut command, true)
    }

    fn spawn_kill_on_drop_job_inner(
        command: &mut Command,
        inject_assignment_failure: bool,
    ) -> io::Result<ContainedChild> {
        use std::os::windows::process::CommandExt;

        // Configure the kill-on-close policy before process creation.
        // SAFETY: Null security attributes create an unnamed, non-inheritable
        // Job Object that is owned exclusively by the returned wrapper.
        let job = unsafe { CreateJobObjectW(ptr::null(), ptr::null()) };
        if job.is_null() || job == INVALID_HANDLE_VALUE {
            return Err(containment_error());
        }
        let job = OwnedHandle(job);
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        // SAFETY: `limits` has the exact structure and size required by the
        // selected JobObjectExtendedLimitInformation class and remains alive.
        let configured = unsafe {
            SetInformationJobObject(
                job.0,
                JobObjectExtendedLimitInformation,
                (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };
        if configured == 0 {
            return Err(containment_error());
        }

        command.creation_flags(CREATE_SUSPENDED);
        let mut child = command.spawn()?;

        // Retain the primary thread handle while the process is still
        // suspended. Reopening a thread by its numeric ID at resume time could
        // target an unrelated process if the child had been externally killed
        // and the operating system reused its process/thread IDs.
        let primary_thread = match primary_thread_handle(child.id()) {
            Ok(thread) => thread,
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(containment_error());
            }
        };

        if inject_assignment_failure {
            let _ = child.kill();
            let _ = child.wait();
            return Err(containment_error());
        }

        // SAFETY: The child was created suspended, so its main thread cannot
        // execute or create descendants before Job assignment completes.
        let process = unsafe { OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, 0, child.id()) };
        if process.is_null() || process == INVALID_HANDLE_VALUE {
            let _ = child.kill();
            let _ = child.wait();
            return Err(containment_error());
        }
        let process = OwnedHandle(process);
        // SAFETY: The process is still suspended; the job is configured and
        // owned. Assignment occurs before any thread is resumed.
        let assigned = unsafe { AssignProcessToJobObject(job.0, process.0) };
        if assigned == 0 {
            let _ = child.kill();
            let _ = child.wait();
            return Err(containment_error());
        }
        drop(process);

        let contained = ContainedChild {
            child,
            job,
            primary_thread,
            exit_status: None,
            resumed: false,
            finished: false,
        };
        Ok(contained)
    }

    fn primary_thread_handle(process_id: u32) -> io::Result<OwnedHandle> {
        // SAFETY: Thread enumeration is read-only and the snapshot handle is
        // wrapped immediately. The process remains suspended during setup.
        let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) };
        if snapshot.is_null() || snapshot == INVALID_HANDLE_VALUE {
            return Err(containment_error());
        }
        let snapshot = OwnedHandle(snapshot);
        let mut entry = THREADENTRY32 {
            dwSize: size_of::<THREADENTRY32>() as u32,
            ..THREADENTRY32::default()
        };
        // SAFETY: `entry` is initialized with its required size and is writable.
        let mut found = unsafe { Thread32First(snapshot.0, &mut entry) };
        if found == 0 {
            return Err(containment_error());
        }
        let mut thread_id = None;
        loop {
            if entry.th32OwnerProcessID == process_id {
                if thread_id.replace(entry.th32ThreadID).is_some() {
                    return Err(containment_error());
                }
            }
            // SAFETY: `entry` remains a writable THREADENTRY32 with dwSize set.
            found = unsafe { Thread32Next(snapshot.0, &mut entry) };
            if found == 0 {
                // Accept the enumeration only when Toolhelp reports the
                // documented end-of-snapshot condition. A partial snapshot
                // must not be mistaken for proof of a single primary thread.
                // SAFETY: GetLastError reads the immediately preceding
                // Thread32Next result on this thread.
                if unsafe { GetLastError() } != ERROR_NO_MORE_FILES {
                    return Err(containment_error());
                }
                break;
            }
        }
        let thread_id = thread_id.ok_or_else(containment_error)?;
        // SAFETY: `thread_id` was observed as the only thread owned by the
        // still-suspended child. The handle grants only suspend/resume access.
        let thread = unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, thread_id) };
        if thread.is_null() || thread == INVALID_HANDLE_VALUE {
            return Err(containment_error());
        }
        Ok(OwnedHandle(thread))
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

    pub(super) fn secure_current_user_directory(path: &Path) -> io::Result<()> {
        set_private_path_dacl(path, true)
    }

    pub(super) fn secure_current_user_file(path: &Path) -> io::Result<()> {
        set_private_path_dacl(path, false)
    }

    fn set_private_path_dacl(path: &Path, inherit_to_children: bool) -> io::Result<()> {
        use std::os::windows::ffi::OsStrExt;

        // OWNER RIGHTS survives logon-session changes; a TokenLogonSid would
        // strand this persistent directory after the user signs out.
        let sddl = if inherit_to_children {
            "D:P(A;OICI;FA;;;SY)(A;OICI;FA;;;OW)"
        } else {
            "D:P(A;;FA;;;SY)(A;;FA;;;OW)"
        };
        let sddl_wide: Vec<u16> = sddl.encode_utf16().chain(Some(0)).collect();
        let mut descriptor = ptr::null_mut();
        // SAFETY: `sddl_wide` is a valid NUL-terminated UTF-16 SDDL string and
        // `descriptor` is an initialized out-pointer owned by LocalFree below.
        if unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl_wide.as_ptr(),
                SDDL_REVISION_1,
                &mut descriptor,
                ptr::null_mut(),
            )
        } == 0
            || descriptor.is_null()
        {
            return Err(redacted_error());
        }
        let descriptor = OwnedSecurityDescriptor(descriptor);
        let mut dacl_present = 0;
        let mut dacl = ptr::null_mut();
        let mut dacl_defaulted = 0;
        // SAFETY: The descriptor is valid and owned above; the output pointers are
        // initialized and remain valid for the duration of this call.
        if unsafe {
            GetSecurityDescriptorDacl(
                descriptor.0,
                &mut dacl_present,
                &mut dacl,
                &mut dacl_defaulted,
            )
        } == 0
            || dacl_present == 0
            || dacl.is_null()
        {
            return Err(redacted_error());
        }
        let path_wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        // SAFETY: `path_wide` is NUL-terminated; `dacl` points into the live
        // security descriptor. Null owner/group/SACL arguments preserve them.
        let result = unsafe {
            SetNamedSecurityInfoW(
                path_wide.as_ptr(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
                ptr::null_mut(),
                ptr::null_mut(),
                dacl,
                ptr::null(),
            )
        };
        if result != 0 {
            return Err(redacted_error());
        }
        Ok(())
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

#[cfg(all(test, windows))]
mod contained_child_tests {
    use super::{
        ContainedChild, report_exit_only_after_job_empty, spawn_kill_on_drop_job,
        spawn_kill_on_drop_job_fail_before_assignment, wait_until_job_empty,
    };
    use std::{
        fs,
        io::Read,
        os::windows::process::CommandExt,
        process::{Command, Stdio},
        thread,
        time::{Duration, Instant},
    };
    use windows_sys::Win32::{
        Foundation::{CloseHandle, ERROR_INVALID_PARAMETER, GetLastError, WAIT_OBJECT_0},
        System::Threading::{
            CREATE_BREAKAWAY_FROM_JOB, OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject,
        },
    };

    struct TempDir(std::path::PathBuf);

    impl TempDir {
        fn new() -> Self {
            use std::sync::atomic::{AtomicU64, Ordering};

            static SEQUENCE: AtomicU64 = AtomicU64::new(1);
            let path = std::env::temp_dir().join(format!(
                "litecowork-contained-process-{}-{}",
                std::process::id(),
                SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).expect("create unique temporary directory");
            Self(path)
        }

        fn path(&self) -> &std::path::Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    const FIXTURE_MODE: &str = "LITECOWORK_JOB_OBJECT_FIXTURE_MODE";
    const FIXTURE_MARKER: &str = "LITECOWORK_JOB_OBJECT_FIXTURE_MARKER";

    fn fixture_command(mode: &str, marker: &std::path::Path) -> Command {
        let mut command = Command::new(std::env::current_exe().expect("test executable"));
        command
            .args([
                "--exact",
                "contained_child_tests::fixture",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(FIXTURE_MODE, mode)
            .env(FIXTURE_MARKER, marker)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command
    }

    fn wait_for_file(path: &std::path::Path) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if path.exists() {
                return;
            }
            thread::sleep(Duration::from_millis(10));
        }
        panic!("fixture did not create {:?}", path.file_name());
    }

    fn assert_windows_process_exits(process_id: u32) {
        // SAFETY: The process ID came from Child::id or the contained fixture.
        let process = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, process_id) };
        if process.is_null() {
            // SAFETY: GetLastError reads the immediately preceding OpenProcess result.
            assert_eq!(unsafe { GetLastError() }, ERROR_INVALID_PARAMETER);
            return;
        }
        // SAFETY: The process handle is owned by this scope and grants SYNCHRONIZE only.
        let wait_result = unsafe { WaitForSingleObject(process, 2_000) };
        // SAFETY: `process` is a valid handle returned by OpenProcess.
        unsafe { CloseHandle(process) };
        assert_eq!(
            wait_result, WAIT_OBJECT_0,
            "process {process_id} survived Job termination"
        );
    }

    fn descendant_pid(marker: &std::path::Path) -> u32 {
        let pid_file = marker.with_extension("pid");
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if let Ok(contents) = fs::read_to_string(&pid_file)
                && let Ok(process_id) = contents.trim().parse()
            {
                return process_id;
            }
            thread::sleep(Duration::from_millis(10));
        }
        panic!("fixture did not write a complete descendant PID");
    }

    #[test]
    fn fixture() {
        let Ok(mode) = std::env::var(FIXTURE_MODE) else {
            return;
        };
        let marker = std::path::PathBuf::from(
            std::env::var_os(FIXTURE_MARKER).expect("fixture marker path"),
        );
        match mode.as_str() {
            "sentinel" => fs::write(marker, b"started").expect("write sentinel"),
            "grandchild" => {
                fs::write(marker.with_extension("pid"), std::process::id().to_string())
                    .expect("write descendant PID");
                thread::sleep(Duration::from_secs(60));
            }
            "spawner" => {
                println!("contained probe started");
                let descendant = fixture_command("grandchild", &marker)
                    .spawn()
                    .expect("spawn descendant");
                fs::write(marker.with_extension("pid"), descendant.id().to_string())
                    .expect("write descendant PID");
                thread::sleep(Duration::from_secs(60));
            }
            "exit-after-spawn" => {
                let descendant = fixture_command("grandchild", &marker)
                    .spawn()
                    .expect("spawn descendant");
                // Keep the direct child alive until the fixture has started; this
                // distinguishes a live descendant from a process creation in flight.
                wait_for_file(&marker.with_extension("pid"));
                fs::write(
                    marker.with_extension("spawned-pid"),
                    descendant.id().to_string(),
                )
                .expect("write descendant PID");
            }
            "breakaway-spawner" => {
                let mut command = fixture_command("grandchild", &marker);
                command.creation_flags(CREATE_BREAKAWAY_FROM_JOB);
                match command.spawn() {
                    Ok(descendant) => {
                        fs::write(marker.with_extension("pid"), descendant.id().to_string())
                            .expect("write breakaway attempt PID");
                        thread::sleep(Duration::from_secs(60));
                    }
                    Err(_) => fs::write(marker.with_extension("breakaway"), b"blocked")
                        .expect("record rejected Job breakaway"),
                }
            }
            _ => panic!("unknown fixture mode"),
        }
    }

    fn start_spawner(directory: &TempDir) -> (ContainedChild, std::path::PathBuf) {
        let marker = directory.path().join("probe-started");
        let mut child = spawn_kill_on_drop_job(fixture_command("spawner", &marker))
            .expect("create suspended process and assign Job before execution");
        child.resume().expect("resume contained probe");
        (child, marker)
    }

    #[test]
    fn immediate_process_code_runs_only_after_contained_handle_is_returned_and_resumed() {
        let directory = TempDir::new();
        let marker = directory.path().join("started");
        let mut child = spawn_kill_on_drop_job(fixture_command("sentinel", &marker))
            .expect("create suspended process and assign Job");

        assert!(
            !marker.exists(),
            "process must not run before explicit resume"
        );
        assert!(child.take_stdin().is_err(), "stdin is gated until resume");
        assert!(child.take_stdout().is_err(), "stdout is gated until resume");
        assert!(child.take_stderr().is_err(), "stderr is gated until resume");
        assert!(child.try_wait().is_err(), "wait is gated until resume");
        child.resume().expect("resume contained process");
        assert!(child.resume().is_err(), "a contained process resumes once");
        assert!(child.take_stdin().unwrap().is_some());
        assert!(child.take_stderr().unwrap().is_some());
        let status = child.wait().expect("wait for contained process");
        assert!(status.success());
        assert_eq!(fs::read(marker).expect("read sentinel"), b"started");
    }

    #[test]
    fn kill_tree_closes_inherited_stdout_and_terminates_descendants() {
        let directory = TempDir::new();
        let (mut child, marker) = start_spawner(&directory);
        let stdout = child
            .take_stdout()
            .expect("resumed child output")
            .expect("piped stdout");
        let reader = thread::spawn(move || {
            let mut output = Vec::new();
            let mut stdout = stdout;
            stdout.read_to_end(&mut output).expect("read probe output");
            output
        });
        let descendant = descendant_pid(&marker);

        child
            .kill_tree()
            .expect("terminate all contained processes");
        let _ = child.wait().expect("reap direct child");
        let output = reader.join().expect("reader exits after Job termination");
        assert!(
            output
                .windows(b"contained probe started".len())
                .any(|window| window == b"contained probe started")
        );
        assert_windows_process_exits(descendant);
    }

    #[test]
    fn direct_child_exit_does_not_prove_job_tree_quiescence() {
        use std::time::Instant;

        let directory = TempDir::new();
        let marker = directory.path().join("exit-after-spawn");
        let mut child = spawn_kill_on_drop_job(fixture_command("exit-after-spawn", &marker))
            .expect("create suspended process and assign Job before execution");
        child.resume().expect("resume contained process");
        let descendant = descendant_pid(&marker);
        let started = Instant::now();
        assert!(
            child
                .wait()
                .expect("wait terminates and confirms the complete Job after its deadline")
                .success()
        );
        assert!(started.elapsed() >= Duration::from_millis(900));
        assert_windows_process_exits(descendant);
    }

    #[test]
    fn try_wait_keeps_direct_exit_unsettled_until_job_zero() {
        let directory = TempDir::new();
        let marker = directory.path().join("try-wait-parent-exits");
        let mut child = spawn_kill_on_drop_job(fixture_command("exit-after-spawn", &marker))
            .expect("create suspended process and assign Job before execution");
        let direct_child_pid = child.id();
        child.resume().expect("resume contained process");
        let descendant = descendant_pid(&marker);
        assert_windows_process_exits(direct_child_pid);

        assert_eq!(
            child.try_wait().expect("query direct exit and Job state"),
            None,
            "a reaped direct child with a live descendant is not a settled Job"
        );
        child.kill_tree().expect("terminate the remaining Job tree");
        assert!(
            child
                .try_wait()
                .expect("recheck Job after termination")
                .expect("Job zero returns the preserved direct exit status")
                .success()
        );
        assert_windows_process_exits(descendant);
    }

    #[test]
    fn wait_reports_normal_completion_after_job_is_empty() {
        let directory = TempDir::new();
        let mut child = spawn_kill_on_drop_job(fixture_command(
            "sentinel",
            &directory.path().join("normal-completion"),
        ))
        .expect("create suspended process and assign Job before execution");
        child.resume().expect("resume contained process");
        assert!(child.wait().expect("wait for empty Job").success());
        child
            .wait_for_job_quiescence(Duration::from_millis(1))
            .expect("returned status is settled only after Job reaches zero");
    }

    #[test]
    fn job_quiescence_wait_times_out_while_a_descendant_is_alive() {
        let directory = TempDir::new();
        let (mut child, marker) = start_spawner(&directory);
        let descendant = descendant_pid(&marker);
        let timeout = child
            .wait_for_job_quiescence(Duration::from_millis(80))
            .expect_err("a live descendant cannot be treated as quiescent");
        assert_eq!(timeout.kind(), std::io::ErrorKind::TimedOut);
        child.kill_tree().expect("terminate the Job process tree");
        let _ = child.wait().expect("reap direct child");
        child
            .wait_for_job_quiescence(Duration::from_secs(2))
            .expect("observe Job reaches zero active processes");
        assert_windows_process_exits(descendant);
    }

    #[test]
    fn job_quiescence_query_errors_fail_closed() {
        let mut calls = 0;
        let error = wait_until_job_empty(Duration::from_secs(1), Duration::from_millis(1), || {
            calls += 1;
            Err(std::io::Error::other("injected query failure"))
        })
        .expect_err("query failure cannot create a quiescence result");
        assert_eq!(error.kind(), std::io::ErrorKind::Other);
        assert_eq!(
            calls, 1,
            "query errors must return without retrying as success"
        );
    }

    #[test]
    fn exit_status_is_not_reported_when_job_query_fails() {
        let directory = TempDir::new();
        let mut child = spawn_kill_on_drop_job(fixture_command(
            "sentinel",
            &directory.path().join("query-error"),
        ))
        .expect("create suspended process and assign Job before execution");
        child.resume().expect("resume contained process");
        let status = child.wait().expect("settle normal Job for query seam");
        let error = report_exit_only_after_job_empty(Some(status), || {
            Err(std::io::Error::other("injected Job query failure"))
        })
        .expect_err("query failure must prevent a settled result");
        assert_eq!(error.kind(), std::io::ErrorKind::Other);
    }

    #[test]
    fn dropping_contained_child_terminates_descendant_tree() {
        let directory = TempDir::new();
        let (child, marker) = start_spawner(&directory);
        let direct_child = child.id();
        let descendant = descendant_pid(&marker);

        drop(child);

        assert_windows_process_exits(direct_child);
        assert_windows_process_exits(descendant);
    }

    #[test]
    fn assigned_job_does_not_allow_descendants_to_break_away() {
        let directory = TempDir::new();
        let marker = directory.path().join("breakaway-probe");
        let mut child = spawn_kill_on_drop_job(fixture_command("breakaway-spawner", &marker))
            .expect("create suspended process and assign Job before execution");
        child.resume().expect("resume contained probe");

        let deadline = Instant::now() + Duration::from_secs(5);
        let mut attempted_child = None;
        while Instant::now() < deadline {
            let pid_file = marker.with_extension("pid");
            if pid_file.exists() {
                attempted_child = Some(
                    fs::read_to_string(pid_file)
                        .expect("read breakaway attempt PID")
                        .trim()
                        .parse::<u32>()
                        .expect("valid breakaway attempt PID"),
                );
                break;
            }
            if marker.with_extension("breakaway").exists() {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }

        child.kill_tree().expect("terminate assigned Job");
        let _ = child.wait().expect("reap direct child");
        if let Some(descendant) = attempted_child {
            assert_windows_process_exits(descendant);
        } else {
            assert_eq!(
                fs::read(marker.with_extension("breakaway"))
                    .expect("read denied breakaway evidence"),
                b"blocked",
                "descendant creation should fail when the Job denies breakaway"
            );
        }
    }

    #[test]
    fn containment_setup_failure_kills_the_suspended_process_before_returning_error() {
        let directory = TempDir::new();
        let marker = directory.path().join("must-not-run");

        assert!(
            spawn_kill_on_drop_job_fail_before_assignment(fixture_command("sentinel", &marker))
                .is_err()
        );
        thread::sleep(Duration::from_millis(50));
        assert!(
            !marker.exists(),
            "failed setup must not run uncontained code"
        );
    }
}

#[cfg(all(test, windows))]
mod workspace_root_directory_tests {
    use super::{open_directory_path_without_reparse, win32::validate_relative_file_path};
    use std::{
        fs, io,
        process::Command,
        sync::atomic::{AtomicU64, Ordering},
    };

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(1);

    struct TempDir(std::path::PathBuf);

    impl TempDir {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "litecowork-root-handle-{}-{}",
                std::process::id(),
                NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).expect("create unique temporary directory");
            Self(path)
        }

        fn path(&self) -> &std::path::Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn opens_local_directory_and_revalidates_full_volume_file_identity() {
        let temporary = TempDir::new();
        let selected = temporary.path().join("selected");
        let nested = selected.join("nested").join("leaf");
        fs::create_dir_all(&nested).expect("create nested selected folder");

        let opened = open_directory_path_without_reparse(&nested)
            .expect("open the selected folder without following reparse points");
        let identity = opened.identity();
        opened
            .revalidate_path(&nested)
            .expect("same path continues to identify the opened directory");
        assert!(
            open_directory_path_without_reparse(&nested)
                .expect("reopen selected folder")
                .identity()
                == identity
        );
    }

    #[test]
    fn path_replacement_with_another_directory_fails_identity_revalidation() {
        let temporary = TempDir::new();
        let selected = temporary.path().join("selected");
        let moved = temporary.path().join("moved-original");
        fs::create_dir(&selected).expect("create selected folder");
        let opened = open_directory_path_without_reparse(&selected).expect("open selected folder");

        fs::rename(&selected, &moved)
            .expect("move opened folder while handle allows delete sharing");
        fs::create_dir(&selected).expect("replace selected path with a different directory");

        assert!(opened.revalidate_path(&selected).is_err());
    }

    #[test]
    fn refuses_junctions_in_every_path_component() {
        let temporary = TempDir::new();
        let target = temporary.path().join("target");
        let junction = temporary.path().join("junction");
        fs::create_dir(&target).expect("create junction target");
        let result = Command::new("cmd.exe")
            .args(["/C", "mklink", "/J"])
            .arg(&junction)
            .arg(&target)
            .output()
            .expect("run Windows junction creation command");
        assert!(result.status.success(), "junction setup failed");

        assert!(open_directory_path_without_reparse(&junction).is_err());
        assert!(open_directory_path_without_reparse(&junction.join("child")).is_err());
    }

    #[test]
    fn rejects_unc_and_wsl_locations() {
        for path in [
            std::path::Path::new(r"\\server\share\folder"),
            std::path::Path::new(r"\\wsl$\Ubuntu\home\user"),
            std::path::Path::new(r"\\wsl.localhost\Ubuntu\home\user"),
        ] {
            assert!(open_directory_path_without_reparse(path).is_err());
        }
    }

    #[test]
    fn creates_nested_relative_files_from_the_verified_directory_handle() {
        let temporary = TempDir::new();
        let root = temporary.path().join("safe-root");
        fs::create_dir(&root).expect("create root");
        let opened = open_directory_path_without_reparse(&root).expect("open safe root");

        opened
            .create_new_file(
                std::path::Path::new("nested/path/input.txt"),
                b"exact bytes",
            )
            .expect("create child path relative to the retained root handle");

        assert_eq!(
            fs::read(root.join("nested/path/input.txt")).expect("read created file"),
            b"exact bytes"
        );
    }

    #[test]
    fn relative_directory_creation_installs_a_protected_system_owner_dacl() {
        let temporary = TempDir::new();
        let root = temporary.path().join("safe-root");
        fs::create_dir(&root).expect("create root");
        let opened = open_directory_path_without_reparse(&root).expect("open safe root");

        let child = opened
            .create_new_directory(std::path::Path::new("private-child"))
            .expect("create protected child directory");

        assert!(child.has_protected_system_owner_dacl().unwrap());
    }

    #[test]
    fn relative_file_creation_rejects_a_junction_without_writing_outside_root() {
        let temporary = TempDir::new();
        let root = temporary.path().join("safe-root");
        let target = temporary.path().join("outside-target");
        let junction = root.join("escape");
        fs::create_dir(&root).expect("create root");
        fs::create_dir(&target).expect("create outside target");
        let opened = open_directory_path_without_reparse(&root).expect("open safe root");
        let result = Command::new("cmd.exe")
            .args(["/C", "mklink", "/J"])
            .arg(&junction)
            .arg(&target)
            .output()
            .expect("run Windows junction creation command");
        assert!(result.status.success(), "junction setup failed");

        assert!(
            opened
                .create_new_file(
                    std::path::Path::new("escape/payload.txt"),
                    b"must not escape"
                )
                .is_err()
        );
        assert!(!target.join("payload.txt").exists());
    }

    #[test]
    fn relative_file_creation_rejects_absolute_and_parent_paths() {
        let temporary = TempDir::new();
        let root = temporary.path().join("safe-root");
        fs::create_dir(&root).expect("create root");
        let opened = open_directory_path_without_reparse(&root).expect("open safe root");

        for path in [
            std::path::Path::new("../outside.txt"),
            std::path::Path::new("C:\\outside.txt"),
            std::path::Path::new("nested/../../outside.txt"),
        ] {
            assert!(
                opened.create_new_file(path, b"no").is_err(),
                "accepted {path:?}"
            );
        }
    }

    #[test]
    fn relative_file_creation_is_create_new_and_never_overwrites() {
        let temporary = TempDir::new();
        let root = temporary.path().join("safe-root");
        fs::create_dir(&root).expect("create root");
        let opened = open_directory_path_without_reparse(&root).expect("open safe root");

        opened
            .create_new_file(std::path::Path::new("result.txt"), b"first")
            .expect("create new file");
        let error = opened
            .create_new_file(std::path::Path::new("result.txt"), b"replacement")
            .expect_err("existing file must not be overwritten");

        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read(root.join("result.txt")).unwrap(), b"first");
    }

    #[test]
    fn relative_creation_stays_on_the_opened_directory_after_path_replacement() {
        let temporary = TempDir::new();
        let root = temporary.path().join("selected-root");
        let moved = temporary.path().join("moved-original");
        fs::create_dir(&root).expect("create root");
        let opened = open_directory_path_without_reparse(&root).expect("open root handle");

        fs::rename(&root, &moved).expect("rename opened directory");
        fs::create_dir(&root).expect("replace the visible path");
        opened
            .create_new_file(
                std::path::Path::new("pinned.txt"),
                b"stays under opened handle",
            )
            .expect("relative creation must remain anchored to the verified directory handle");

        assert_eq!(
            fs::read(moved.join("pinned.txt")).unwrap(),
            b"stays under opened handle"
        );
        assert!(!root.join("pinned.txt").exists());
        assert!(opened.revalidate_path(&root).is_err());
    }

    #[test]
    fn relative_file_path_has_a_bounded_total_utf16_length() {
        let component = "a".repeat(240);
        let too_long = std::iter::repeat_n(component.as_str(), 71)
            .collect::<Vec<_>>()
            .join("\\");
        assert!(validate_relative_file_path(std::path::Path::new(&too_long)).is_err());
    }
}

#[cfg(all(test, windows))]
mod windows_root_overlap_tests {
    use super::win32::test_canonical_volume_paths_overlap as overlaps;
    use super::windows_absolute_paths_overlap;
    use std::{fs, path::Path, process::Command};

    #[test]
    fn drive_letter_and_ancestor_comparison_is_ordinal_case_insensitive() {
        assert!(
            overlaps(
                r"\\?\Volume{11111111-1111-1111-1111-111111111111}\Work\Project",
                r"\\?\Volume{11111111-1111-1111-1111-111111111111}\work\project\Child",
            )
            .expect("valid canonical volume paths")
        );
        assert!(
            overlaps(
                r"\\?\Volume{22222222-2222-2222-2222-222222222222}\Work\Project\Child",
                r"\\?\Volume{22222222-2222-2222-2222-222222222222}\work\project",
            )
            .expect("valid canonical volume paths")
        );
    }

    #[test]
    fn sibling_component_prefixes_do_not_overlap() {
        assert!(
            !overlaps(
                r"\\?\Volume{11111111-1111-1111-1111-111111111111}\Work\Project",
                r"\\?\Volume{11111111-1111-1111-1111-111111111111}\Work\Project-old",
            )
            .expect("valid canonical volume paths")
        );
    }

    #[test]
    fn unicode_case_pairs_use_windows_ordinal_comparison() {
        assert!(
            overlaps(
                "\\\\?\\Volume{11111111-1111-1111-1111-111111111111}\\Projects\\\u{00c4}gent",
                "\\\\?\\Volume{11111111-1111-1111-1111-111111111111}\\projects\\\u{00e4}GENT\\child",
            )
            .expect("valid canonical volume paths")
        );
    }

    #[test]
    fn different_drive_volumes_do_not_overlap() {
        assert!(
            !overlaps(
                r"\\?\Volume{11111111-1111-1111-1111-111111111111}\Work\Project",
                r"\\?\Volume{22222222-2222-2222-2222-222222222222}\Work\Project\Child",
            )
            .expect("valid canonical volume paths")
        );
    }

    #[test]
    fn malformed_volume_guid_final_path_fails_closed() {
        assert!(overlaps(r"C:\Work", r"C:\Work").is_err());
        assert!(
            overlaps(
                r"\\?\Volume{missing-guid}\Work",
                r"\\?\Volume{missing-guid}\Work"
            )
            .is_err()
        );
    }

    #[test]
    fn subst_drive_aliases_resolve_to_the_same_volume_path_when_supported() {
        let nonce = format!("litecowork-subst-{}", std::process::id());
        let root = std::env::temp_dir().join(nonce);
        let parent = root.join("parent");
        let child = parent.join("child");
        fs::create_dir_all(&child).expect("create temporary test tree");

        // SAFETY: GetLogicalDrives has no pointer arguments or retained state.
        let used_drive_mask =
            unsafe { windows_sys::Win32::Storage::FileSystem::GetLogicalDrives() };
        let drive_letter = ('R'..='Z')
            .find(|letter| used_drive_mask & (1 << (u32::from(*letter) - u32::from('A'))) == 0);
        let Some(drive_letter) = drive_letter else {
            let _ = fs::remove_dir_all(&root);
            eprintln!("SUBST alias coverage unavailable: no free drive letter");
            return;
        };
        let drive = format!("{drive_letter}:");
        let mapping = Command::new("subst.exe")
            .arg(&drive)
            .arg(&parent)
            .status()
            .expect("run SUBST to create temporary alias");
        if !mapping.success() {
            let _ = fs::remove_dir_all(&root);
            eprintln!("SUBST alias coverage unavailable: host rejected temporary mapping");
            return;
        }
        struct SubstCleanup(String);
        impl Drop for SubstCleanup {
            fn drop(&mut self) {
                let _ = Command::new("subst.exe")
                    .args([self.0.as_str(), "/D"])
                    .status();
            }
        }
        let _cleanup = SubstCleanup(drive.clone());
        let aliased_child = format!("{drive}\\child");
        assert!(
            windows_absolute_paths_overlap(&parent, Path::new(&aliased_child))
                .expect("resolve both directory handles to volume GUID paths")
        );
        drop(_cleanup);
        fs::remove_dir_all(&root).expect("remove temporary test tree");
    }
}
