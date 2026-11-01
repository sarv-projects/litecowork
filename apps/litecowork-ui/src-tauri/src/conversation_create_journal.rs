use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        Mutex, MutexGuard,
        atomic::{AtomicU64, Ordering},
    },
};

const JOURNAL_DIRECTORY: &str = "conversation-create-recovery";
const JOURNAL_FILENAME: &str = "pending.v1.json";
const JOURNAL_LOCK_FILENAME: &str = "pending.v1.lock";
const JOURNAL_VERSION: u32 = 1;
const MAX_PENDING_CREATES: usize = 64;
const MAX_JOURNAL_BYTES: usize = 128 * 1024;
const MAX_WORKSPACE_ID_BYTES: usize = 256;
const MAX_REQUEST_ID_BYTES: usize = 200;
const MAX_TITLE_CHARS: usize = 160;
const MAX_TEMP_FILE_ATTEMPTS: u8 = 8;

static JOURNAL_LOCK: Mutex<()> = Mutex::new(());
static TEMP_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct PendingConversationCreate {
    pub(super) workspace_id: String,
    #[serde(deserialize_with = "deserialize_required_title")]
    pub(super) title: Option<String>,
    pub(super) request_id: String,
}

fn deserialize_required_title<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<String>::deserialize(deserializer)
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct JournalDocument {
    version: u32,
    entries: Vec<PendingConversationCreate>,
}

pub(super) struct ConversationCreateJournal {
    app_data_dir: PathBuf,
}

struct ConversationCreateJournalGuard {
    _process_guard: MutexGuard<'static, ()>,
    _file_lock: File,
}

impl ConversationCreateJournal {
    pub(super) fn new(app_data_dir: impl AsRef<Path>) -> Self {
        Self {
            app_data_dir: app_data_dir.as_ref().to_path_buf(),
        }
    }

    pub(super) fn list(&self) -> Result<Vec<PendingConversationCreate>, String> {
        let _guard = self.acquire_lock()?;
        self.read_entries()
    }

    pub(super) fn record(&self, entry: PendingConversationCreate) -> Result<(), String> {
        validate_entry(&entry)?;
        let _guard = self.acquire_lock()?;
        let mut entries = self.read_entries()?;

        if let Some(existing) = entries
            .iter()
            .find(|existing| existing.request_id == entry.request_id)
        {
            if existing == &entry {
                return Ok(());
            }
            return Err(
                "Conversation request identity is already pending with different content"
                    .to_owned(),
            );
        }
        if entries.iter().any(|existing| {
            existing.workspace_id == entry.workspace_id && existing.title == entry.title
        }) {
            return Err(
                "An exact Conversation creation is already awaiting confirmation".to_owned(),
            );
        }
        if entries.len() >= MAX_PENDING_CREATES {
            return Err(
                "Resolve a pending Conversation creation before starting another".to_owned(),
            );
        }
        entries.push(entry);
        self.write_entries(&entries)
    }

    pub(super) fn acknowledge(&self, workspace_id: &str, request_id: &str) -> Result<(), String> {
        if !valid_workspace_id(workspace_id) || !valid_request_id(request_id) {
            return Err("Conversation acknowledgement is invalid".to_owned());
        }
        let _guard = self.acquire_lock()?;
        let mut entries = self.read_entries()?;
        let Some(position) = entries
            .iter()
            .position(|entry| entry.workspace_id == workspace_id && entry.request_id == request_id)
        else {
            // Acknowledgement is intentionally idempotent. A stale or wrong-Workspace
            // acknowledgement cannot remove another pending command.
            return Ok(());
        };
        entries.remove(position);
        self.write_entries(&entries)
    }

    fn read_entries(&self) -> Result<Vec<PendingConversationCreate>, String> {
        let directory = self.ensure_directory()?;
        let path = directory.join(JOURNAL_FILENAME);
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(_) => return Err("Conversation recovery storage could not be read".to_owned()),
        };
        validate_journal_file_metadata(&metadata, &directory)?;

        let mut file = File::open(&path)
            .map_err(|_| "Conversation recovery storage could not be read".to_owned())?;
        let opened_metadata = file
            .metadata()
            .map_err(|_| "Conversation recovery storage could not be read".to_owned())?;
        validate_opened_file_identity(&metadata, &opened_metadata)?;

        let mut bytes = Vec::new();
        Read::by_ref(&mut file)
            .take((MAX_JOURNAL_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| "Conversation recovery storage could not be read".to_owned())?;
        if bytes.len() > MAX_JOURNAL_BYTES {
            return Err("Conversation recovery storage exceeds its size limit".to_owned());
        }
        if bytes.is_empty() {
            return Err("Conversation recovery storage is corrupt".to_owned());
        }
        let document: JournalDocument = serde_json::from_slice(&bytes)
            .map_err(|_| "Conversation recovery storage is corrupt".to_owned())?;
        validate_document(&document)?;
        Ok(document.entries)
    }

    fn write_entries(&self, entries: &[PendingConversationCreate]) -> Result<(), String> {
        if entries.len() > MAX_PENDING_CREATES {
            return Err("Conversation recovery storage exceeds its entry limit".to_owned());
        }
        for entry in entries {
            validate_entry(entry)?;
        }
        validate_unique_entries(entries)?;
        let document = JournalDocument {
            version: JOURNAL_VERSION,
            entries: entries.to_vec(),
        };
        let bytes = serde_json::to_vec(&document)
            .map_err(|_| "Conversation recovery storage could not be encoded".to_owned())?;
        if bytes.len() > MAX_JOURNAL_BYTES {
            return Err("Conversation recovery storage exceeds its size limit".to_owned());
        }

        let directory = self.ensure_directory()?;
        let destination = directory.join(JOURNAL_FILENAME);
        validate_existing_destination(&destination, &directory)?;
        let temporary = write_private_temporary(&directory, &bytes)?;
        let replace_result = replace_journal_file(&temporary, &destination);
        if replace_result.is_err() {
            let _ = fs::remove_file(&temporary);
            return Err("Conversation recovery storage could not be committed".to_owned());
        }
        #[cfg(unix)]
        sync_directory(&directory)?;
        Ok(())
    }

    fn ensure_directory(&self) -> Result<PathBuf, String> {
        ensure_real_directory(&self.app_data_dir, true)?;
        let directory = self.app_data_dir.join(JOURNAL_DIRECTORY);
        ensure_real_directory(&directory, false)?;
        validate_private_directory(&directory, &self.app_data_dir)?;
        Ok(directory)
    }

    fn acquire_lock(&self) -> Result<ConversationCreateJournalGuard, String> {
        let process_guard = JOURNAL_LOCK
            .lock()
            .map_err(|_| "Conversation recovery storage is unavailable".to_owned())?;
        let directory = self.ensure_directory()?;
        let lock_file = open_private_lock_file(&directory.join(JOURNAL_LOCK_FILENAME), &directory)?;
        lock_file
            .lock_exclusive()
            .map_err(|_| "Conversation recovery storage is unavailable".to_owned())?;
        Ok(ConversationCreateJournalGuard {
            _process_guard: process_guard,
            _file_lock: lock_file,
        })
    }
}

fn validate_document(document: &JournalDocument) -> Result<(), String> {
    if document.version != JOURNAL_VERSION || document.entries.len() > MAX_PENDING_CREATES {
        return Err("Conversation recovery storage has an unsupported version or size".to_owned());
    }
    for entry in &document.entries {
        validate_entry(entry)?;
    }
    validate_unique_entries(&document.entries)
}

fn validate_unique_entries(entries: &[PendingConversationCreate]) -> Result<(), String> {
    let mut request_ids = HashSet::with_capacity(entries.len());
    let mut command_signatures = HashSet::with_capacity(entries.len());
    for entry in entries {
        if !request_ids.insert(entry.request_id.as_str())
            || !command_signatures.insert((entry.workspace_id.as_str(), entry.title.as_deref()))
        {
            return Err(
                "Conversation recovery storage contains duplicate pending requests".to_owned(),
            );
        }
    }
    Ok(())
}

fn validate_entry(entry: &PendingConversationCreate) -> Result<(), String> {
    if !valid_workspace_id(&entry.workspace_id)
        || !valid_request_id(&entry.request_id)
        || entry.title.as_ref().is_some_and(|title| {
            title.trim() != title
                || title.is_empty()
                || title.chars().count() > MAX_TITLE_CHARS
                || title.chars().any(char::is_control)
        })
    {
        return Err("Conversation creation recovery entry is invalid".to_owned());
    }
    Ok(())
}

pub(super) fn valid_workspace_id(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= MAX_WORKSPACE_ID_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

pub(super) fn valid_request_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_REQUEST_ID_BYTES
        && value.bytes().all(|byte| byte.is_ascii_graphic())
}

pub(super) fn normalize_title(title: Option<String>) -> Result<Option<String>, String> {
    let Some(title) = title else {
        return Ok(None);
    };
    let title = title.trim().to_owned();
    if title.is_empty()
        || title.chars().count() > MAX_TITLE_CHARS
        || title.chars().any(char::is_control)
    {
        return Err("Conversation title must contain 1 to 160 printable characters".to_owned());
    }
    Ok(Some(title))
}

fn ensure_real_directory(path: &Path, recursive: bool) -> Result<(), String> {
    let mut created = false;
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err("Conversation recovery storage is unsafe".to_owned());
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            if recursive {
                create_private_directory_tree(path)?;
            } else if let Err(error) = create_private_directory(path) {
                if error.kind() != std::io::ErrorKind::AlreadyExists {
                    return Err("Conversation recovery storage is unavailable".to_owned());
                }
            } else {
                created = true;
            }
            let metadata = fs::symlink_metadata(path)
                .map_err(|_| "Conversation recovery storage is unavailable".to_owned())?;
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err("Conversation recovery storage is unsafe".to_owned());
            }
        }
        Err(_) => return Err("Conversation recovery storage is unavailable".to_owned()),
    }
    #[cfg(unix)]
    if created {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .map_err(|_| "Conversation recovery storage permissions are unavailable".to_owned())?;
    }
    Ok(())
}

fn create_private_directory(path: &Path) -> std::io::Result<()> {
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path)
}

fn create_private_directory_tree(path: &Path) -> Result<(), String> {
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder
        .recursive(true)
        .create(path)
        .map_err(|_| "Conversation recovery storage is unavailable".to_owned())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .map_err(|_| "Conversation recovery storage permissions are unavailable".to_owned())?;
    }
    Ok(())
}

fn validate_private_directory(directory: &Path, app_data_dir: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(directory)
        .map_err(|_| "Conversation recovery storage is unavailable".to_owned())?;
    let app_data_metadata = fs::metadata(app_data_dir)
        .map_err(|_| "Conversation recovery storage is unavailable".to_owned())?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err("Conversation recovery storage is unsafe".to_owned());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.mode() & 0o077 != 0
            || metadata.mode() & 0o700 != 0o700
            || metadata.uid() != app_data_metadata.uid()
        {
            return Err("Conversation recovery storage permissions are unsafe".to_owned());
        }
    }
    #[cfg(not(unix))]
    let _ = app_data_metadata;
    Ok(())
}

fn validate_journal_file_metadata(metadata: &fs::Metadata, directory: &Path) -> Result<(), String> {
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("Conversation recovery storage is unsafe".to_owned());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let directory_metadata = fs::metadata(directory)
            .map_err(|_| "Conversation recovery storage is unavailable".to_owned())?;
        if metadata.mode() & 0o077 != 0
            || metadata.uid() != directory_metadata.uid()
            || metadata.nlink() != 1
        {
            return Err("Conversation recovery storage permissions are unsafe".to_owned());
        }
    }
    #[cfg(not(unix))]
    let _ = directory;
    Ok(())
}

fn open_private_lock_file(path: &Path, directory: &Path) -> Result<File, String> {
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options
        .open(path)
        .map_err(|_| "Conversation recovery storage is unavailable".to_owned())?;
    let path_metadata = fs::symlink_metadata(path)
        .map_err(|_| "Conversation recovery storage is unavailable".to_owned())?;
    validate_journal_file_metadata(&path_metadata, directory)?;
    let opened_metadata = file
        .metadata()
        .map_err(|_| "Conversation recovery storage is unavailable".to_owned())?;
    validate_opened_file_identity(&path_metadata, &opened_metadata)?;
    Ok(file)
}

fn validate_opened_file_identity(
    path_metadata: &fs::Metadata,
    opened_metadata: &fs::Metadata,
) -> Result<(), String> {
    if !opened_metadata.is_file() {
        return Err("Conversation recovery storage is unsafe".to_owned());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if path_metadata.dev() != opened_metadata.dev()
            || path_metadata.ino() != opened_metadata.ino()
        {
            return Err("Conversation recovery storage changed while opening".to_owned());
        }
    }
    #[cfg(not(unix))]
    let _ = path_metadata;
    Ok(())
}

fn validate_existing_destination(destination: &Path, directory: &Path) -> Result<(), String> {
    match fs::symlink_metadata(destination) {
        Ok(metadata) => validate_journal_file_metadata(&metadata, directory),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err("Conversation recovery storage is unavailable".to_owned()),
    }
}

fn write_private_temporary(directory: &Path, bytes: &[u8]) -> Result<PathBuf, String> {
    for attempt in 0..MAX_TEMP_FILE_ATTEMPTS {
        let sequence = TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let temporary = directory.join(format!(
            ".pending-{}-{sequence}-{attempt}.tmp",
            std::process::id()
        ));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = match options.open(&temporary) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(_) => return Err("Conversation recovery storage is unavailable".to_owned()),
        };
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if fs::set_permissions(&temporary, fs::Permissions::from_mode(0o600)).is_err() {
                drop(file);
                let _ = fs::remove_file(&temporary);
                return Err("Conversation recovery storage permissions are unavailable".to_owned());
            }
        }
        let write_result = file.write_all(bytes).and_then(|()| file.sync_all());
        if write_result.is_err() {
            drop(file);
            let _ = fs::remove_file(&temporary);
            return Err("Conversation recovery storage could not be written".to_owned());
        }
        return Ok(temporary);
    }
    Err("Conversation recovery storage is unavailable".to_owned())
}

fn replace_journal_file(source: &Path, destination: &Path) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        return windows_local_security::replace_file_write_through(source, destination);
    }
    #[cfg(not(windows))]
    fs::rename(source, destination)
}

#[cfg(unix)]
fn sync_directory(directory: &Path) -> Result<(), String> {
    File::open(directory)
        .and_then(|file| file.sync_all())
        .map_err(|_| "Conversation recovery storage could not be committed".to_owned())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        path::{Path, PathBuf},
        sync::atomic::{AtomicU64, Ordering},
    };

    static TEST_DIRECTORY_SEQUENCE: AtomicU64 = AtomicU64::new(1);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "litecowork-conversation-journal-{}-{}",
                std::process::id(),
                TEST_DIRECTORY_SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).expect("create unique temporary directory");
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&path, fs::Permissions::from_mode(0o700))
                    .expect("private temporary directory");
            }
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn entry(
        workspace_id: &str,
        title: Option<&str>,
        request_id: &str,
    ) -> PendingConversationCreate {
        PendingConversationCreate {
            workspace_id: workspace_id.to_owned(),
            title: title.map(str::to_owned),
            request_id: request_id.to_owned(),
        }
    }

    fn journal_file(app_data: &Path) -> std::path::PathBuf {
        app_data.join(JOURNAL_DIRECTORY).join(JOURNAL_FILENAME)
    }

    #[test]
    fn exact_pending_create_identity_survives_journal_reopen_and_replay() {
        let directory = TestDirectory::new();
        let pending = entry("workspace-1", Some("Draft"), "request-1");
        let first_process = ConversationCreateJournal::new(directory.path());
        first_process
            .record(pending.clone())
            .expect("persist before send");

        let restarted_process = ConversationCreateJournal::new(directory.path());
        assert_eq!(
            restarted_process.list().expect("read after restart"),
            vec![pending.clone()]
        );
        restarted_process
            .record(pending.clone())
            .expect("exact retry keeps its original identity");
        assert_eq!(
            restarted_process.list().expect("list exact replay"),
            vec![pending]
        );
    }

    #[test]
    fn journal_is_bounded_without_evicting_ambiguous_create_identities() {
        let directory = TestDirectory::new();
        let journal = ConversationCreateJournal::new(directory.path());
        for index in 0..MAX_PENDING_CREATES {
            journal
                .record(entry(
                    "workspace-1",
                    Some(&format!("Draft {index}")),
                    &format!("request-{index}"),
                ))
                .expect("bounded pending create is persisted");
        }
        let before = journal.list().expect("list all pending records");
        assert_eq!(before.len(), MAX_PENDING_CREATES);
        assert!(
            journal
                .record(entry(
                    "workspace-1",
                    Some("Draft overflow"),
                    "request-overflow"
                ))
                .is_err()
        );
        assert_eq!(journal.list().expect("existing records remain"), before);
    }

    #[test]
    fn corrupt_or_oversized_journal_fails_closed_without_replacing_bytes() {
        let directory = TestDirectory::new();
        let journal = ConversationCreateJournal::new(directory.path());
        journal
            .ensure_directory()
            .expect("journal directory exists");
        let path = journal_file(directory.path());

        let corrupt = br#"{"version":1,"entries":[],"unexpected":true}"#;
        fs::write(&path, corrupt).expect("write corrupt fixture");
        assert!(journal.list().is_err());
        assert!(
            journal
                .record(entry("workspace-1", Some("Draft"), "request-1"))
                .is_err()
        );
        assert_eq!(fs::read(&path).expect("corrupt bytes remain"), corrupt);

        let oversized = vec![b'x'; MAX_JOURNAL_BYTES + 1];
        fs::write(&path, &oversized).expect("write oversized fixture");
        assert!(journal.list().is_err());
        assert!(
            journal
                .record(entry("workspace-1", Some("Draft"), "request-1"))
                .is_err()
        );
        assert_eq!(fs::read(&path).expect("oversized bytes remain"), oversized);
    }

    #[test]
    fn journal_rejects_unsupported_or_incomplete_documents_without_replacing_bytes() {
        let directory = TestDirectory::new();
        let journal = ConversationCreateJournal::new(directory.path());
        journal
            .ensure_directory()
            .expect("journal directory exists");
        let path = journal_file(directory.path());

        for invalid in [
            br#"{"version":2,"entries":[]}"#.as_slice(),
            br#"{"version":1,"entries":[{"workspaceId":"workspace-1","requestId":"request-1"}]}"#
                .as_slice(),
        ] {
            fs::write(&path, invalid).expect("write invalid journal fixture");
            assert!(journal.list().is_err());
            assert!(
                journal
                    .record(entry("workspace-1", Some("Draft"), "request-1"))
                    .is_err()
            );
            assert_eq!(fs::read(&path).expect("invalid bytes remain"), invalid);
        }
    }

    #[test]
    fn acknowledgement_removes_only_the_exact_workspace_and_request_and_is_idempotent() {
        let directory = TestDirectory::new();
        let journal = ConversationCreateJournal::new(directory.path());
        let first = entry("workspace-1", Some("Draft"), "request-1");
        let second = entry("workspace-2", Some("Draft"), "request-2");
        journal.record(first.clone()).expect("first pending create");
        journal
            .record(second.clone())
            .expect("second pending create");

        journal
            .acknowledge("workspace-2", "request-1")
            .expect("wrong workspace cannot acknowledge another entry");
        journal
            .acknowledge("workspace-1", "request-missing")
            .expect("unknown request acknowledgement is harmless");
        assert_eq!(
            journal.list().expect("records remain"),
            vec![first.clone(), second.clone()]
        );

        journal
            .acknowledge("workspace-1", "request-1")
            .expect("exact acknowledgement removes entry");
        journal
            .acknowledge("workspace-1", "request-1")
            .expect("repeated acknowledgement succeeds");
        assert_eq!(journal.list().expect("second entry remains"), vec![second]);
    }

    #[test]
    fn invalid_entry_and_unsafe_journal_storage_preserve_existing_state() {
        let directory = TestDirectory::new();
        let journal = ConversationCreateJournal::new(directory.path());
        let saved = entry("workspace-1", Some("Draft"), "request-1");
        journal.record(saved.clone()).expect("initial entry");
        let path = journal_file(directory.path());
        let before = fs::read(&path).expect("read journal bytes");

        assert!(
            journal
                .record(entry("../workspace", Some("Draft"), "request-2"))
                .is_err()
        );
        assert!(
            journal
                .record(entry("workspace-1", Some("\n"), "request-2"))
                .is_err()
        );
        assert!(
            journal
                .record(entry("workspace-1", Some("Draft"), "request-1"))
                .is_ok()
        );
        assert_eq!(
            fs::read(&path).expect("invalid requests do not rewrite journal"),
            before
        );
        assert_eq!(
            journal.list().expect("original pending request remains"),
            vec![saved]
        );
    }

    #[test]
    fn a_new_same_title_request_is_allowed_after_the_ambiguous_entry_is_acknowledged() {
        let directory = TestDirectory::new();
        let journal = ConversationCreateJournal::new(directory.path());
        journal
            .record(entry("workspace-1", Some("Draft"), "request-1"))
            .expect("initial pending request");
        assert!(
            journal
                .record(entry("workspace-1", Some("Draft"), "request-2"))
                .is_err()
        );

        journal
            .acknowledge("workspace-1", "request-1")
            .expect("validated response acknowledges exact request");
        journal
            .record(entry("workspace-1", Some("Draft"), "request-2"))
            .expect("same title is not a permanent deduplication key");
        assert_eq!(
            journal.list().expect("new request remains pending"),
            vec![entry("workspace-1", Some("Draft"), "request-2")]
        );
    }

    #[cfg(unix)]
    #[test]
    fn symlink_journal_path_fails_closed_and_does_not_modify_its_target() {
        use std::os::unix::fs::symlink;

        let directory = TestDirectory::new();
        let journal = ConversationCreateJournal::new(directory.path());
        journal
            .ensure_directory()
            .expect("journal directory exists");
        let path = journal_file(directory.path());
        let target = directory.path().join("outside.json");
        let sentinel = b"owner data must remain untouched";
        fs::write(&target, sentinel).expect("write target sentinel");
        symlink(&target, &path).expect("install symlink at journal path");

        assert!(journal.list().is_err());
        assert!(
            journal
                .record(entry("workspace-1", Some("Draft"), "request-1"))
                .is_err()
        );
        assert_eq!(
            fs::read(&target).expect("target remains readable"),
            sentinel
        );
        assert!(
            fs::symlink_metadata(&path)
                .expect("journal path remains a symlink")
                .file_type()
                .is_symlink()
        );
    }

    #[cfg(unix)]
    #[test]
    fn symlink_lock_path_fails_closed_and_does_not_lock_or_modify_its_target() {
        use std::os::unix::fs::{PermissionsExt, symlink};

        let directory = TestDirectory::new();
        let journal = ConversationCreateJournal::new(directory.path());
        let journal_directory = journal
            .ensure_directory()
            .expect("journal directory exists");
        let lock_path = journal_directory.join(JOURNAL_LOCK_FILENAME);
        let target = directory.path().join("outside.lock");
        let sentinel = b"owner data must remain untouched";
        fs::write(&target, sentinel).expect("write lock target sentinel");
        fs::set_permissions(&target, fs::Permissions::from_mode(0o600))
            .expect("make lock target owner-only");
        symlink(&target, &lock_path).expect("install symlink at lock path");

        assert!(journal.list().is_err());
        assert_eq!(
            fs::read(&target).expect("lock target remains readable"),
            sentinel
        );
        assert!(
            fs::symlink_metadata(&lock_path)
                .expect("lock path remains a symlink")
                .file_type()
                .is_symlink()
        );
    }

    #[cfg(unix)]
    #[test]
    fn journal_file_is_owner_only_and_storage_permission_failure_preserves_it() {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};

        let directory = TestDirectory::new();
        let journal = ConversationCreateJournal::new(directory.path());
        let original = entry("workspace-1", Some("Draft"), "request-1");
        journal.record(original.clone()).expect("initial entry");
        let path = journal_file(directory.path());
        let before = fs::read(&path).expect("read original journal");
        assert_eq!(
            fs::metadata(&path).expect("journal metadata").mode() & 0o777,
            0o600
        );

        let private_directory = path.parent().expect("journal parent");
        let original_mode = fs::metadata(private_directory)
            .expect("private journal directory metadata")
            .mode()
            & 0o777;
        assert_eq!(original_mode, 0o700);
        fs::set_permissions(private_directory, fs::Permissions::from_mode(0o500))
            .expect("make storage unwritable to the owner");
        assert!(
            journal
                .record(entry("workspace-2", Some("Other"), "request-2"))
                .is_err()
        );
        assert_eq!(
            fs::read(&path).expect("failed write preserves original"),
            before
        );
        fs::set_permissions(private_directory, fs::Permissions::from_mode(0o700))
            .expect("restore private directory permissions");
        assert_eq!(
            journal.list().expect("original still readable"),
            vec![original]
        );
    }

    #[test]
    fn journal_lock_excludes_an_independently_opened_handle() {
        use fs2::FileExt;

        let directory = TestDirectory::new();
        let journal = ConversationCreateJournal::new(directory.path());
        let journal_directory = journal
            .ensure_directory()
            .expect("journal directory exists");
        let lock_path = journal_directory.join(JOURNAL_LOCK_FILENAME);
        let first = open_private_lock_file(&lock_path, &journal_directory)
            .expect("open first private lock handle");
        first
            .lock_exclusive()
            .expect("acquire first exclusive lock");
        let second = open_private_lock_file(&lock_path, &journal_directory)
            .expect("open second private lock handle");
        assert!(second.try_lock_exclusive().is_err());

        first.unlock().expect("release first exclusive lock");
        second
            .try_lock_exclusive()
            .expect("second handle acquires released lock");
        second.unlock().expect("release second exclusive lock");
    }
}
