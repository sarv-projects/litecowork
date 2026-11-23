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
const JOURNAL_VERSION: u32 = 3;
const MAX_PENDING_CREATES: usize = 64;
const MAX_JOURNAL_BYTES: usize = 4 * 1024 * 1024;
const MAX_FIRST_TURN_BYTES: usize = 2 * 1024 * 1024;
const MAX_PENDING_SEND_BYTES: usize = 2 * 1024 * 1024;
const MAX_PENDING_SEND_RESOURCE_REFS: usize = 16;
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) first_turn: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct PendingConversationSend {
    pub(super) workspace_id: String,
    pub(super) conversation_id: String,
    pub(super) request_id: String,
    pub(super) content: serde_json::Value,
    pub(super) resource_refs: serde_json::Value,
    pub(super) agent_binding_id: Option<String>,
    pub(super) presentation_preference: Option<String>,
    #[serde(default)]
    pub(super) attachment_labels: Vec<String>,
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
    #[serde(default)]
    sends: Vec<PendingConversationSend>,
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

    pub(super) fn list_sends(&self) -> Result<Vec<PendingConversationSend>, String> {
        let _guard = self.acquire_lock()?;
        Ok(self.read_document()?.sends)
    }

    pub(super) fn record(&self, entry: PendingConversationCreate) -> Result<(), String> {
        validate_entry(&entry)?;
        let _guard = self.acquire_lock()?;
        let mut document = self.read_document()?;

        if let Some(existing) = document
            .entries
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
        if document
            .entries
            .iter()
            .any(|existing| command_signature(existing) == command_signature(&entry))
        {
            return Err(
                "An exact Conversation creation is already awaiting confirmation".to_owned(),
            );
        }
        if document.entries.len() + document.sends.len() >= MAX_PENDING_CREATES {
            return Err(
                "Resolve a pending Conversation creation before starting another".to_owned(),
            );
        }
        document.entries.push(entry);
        self.write_document(&document)
    }

    pub(super) fn record_send(&self, send: PendingConversationSend) -> Result<(), String> {
        validate_send(&send)?;
        let _guard = self.acquire_lock()?;
        let mut document = self.read_document()?;
        if let Some(existing) = document
            .sends
            .iter()
            .find(|existing| existing.request_id == send.request_id)
        {
            if existing == &send {
                return Ok(());
            }
            return Err(
                "Conversation request identity is already pending with different content"
                    .to_owned(),
            );
        }
        if document
            .entries
            .iter()
            .any(|entry| entry.request_id == send.request_id)
        {
            return Err(
                "Conversation request identity is already pending with different content"
                    .to_owned(),
            );
        }
        if document.sends.iter().any(|existing| {
            existing.workspace_id == send.workspace_id
                && existing.conversation_id == send.conversation_id
        }) {
            return Err(
                "Resolve the pending send for this Conversation before sending another".to_owned(),
            );
        }
        if document.entries.len() + document.sends.len() >= MAX_PENDING_CREATES {
            return Err(
                "Resolve a pending Conversation request before starting another".to_owned(),
            );
        }
        document.sends.push(send);
        self.write_document(&document)
    }

    pub(super) fn acknowledge(&self, workspace_id: &str, request_id: &str) -> Result<(), String> {
        if !valid_workspace_id(workspace_id) || !valid_request_id(request_id) {
            return Err("Conversation acknowledgement is invalid".to_owned());
        }
        let _guard = self.acquire_lock()?;
        let mut document = self.read_document()?;
        let Some(position) = document
            .entries
            .iter()
            .position(|entry| entry.workspace_id == workspace_id && entry.request_id == request_id)
        else {
            // Acknowledgement is intentionally idempotent. A stale or wrong-Workspace
            // acknowledgement cannot remove another pending command.
            return Ok(());
        };
        document.entries.remove(position);
        self.write_document(&document)
    }

    pub(super) fn acknowledge_send(
        &self,
        workspace_id: &str,
        conversation_id: &str,
        request_id: &str,
    ) -> Result<(), String> {
        if !valid_workspace_id(workspace_id)
            || !valid_workspace_id(conversation_id)
            || !valid_request_id(request_id)
        {
            return Err("Conversation acknowledgement is invalid".to_owned());
        }
        let _guard = self.acquire_lock()?;
        let mut document = self.read_document()?;
        let Some(position) = document.sends.iter().position(|send| {
            send.workspace_id == workspace_id
                && send.conversation_id == conversation_id
                && send.request_id == request_id
        }) else {
            return Ok(());
        };
        document.sends.remove(position);
        self.write_document(&document)
    }

    fn read_entries(&self) -> Result<Vec<PendingConversationCreate>, String> {
        Ok(self.read_document()?.entries)
    }

    fn read_document(&self) -> Result<JournalDocument, String> {
        let directory = self.ensure_directory()?;
        let path = directory.join(JOURNAL_FILENAME);
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(JournalDocument {
                    version: JOURNAL_VERSION,
                    entries: Vec::new(),
                    sends: Vec::new(),
                });
            }
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
        Ok(document)
    }

    fn write_document(&self, document: &JournalDocument) -> Result<(), String> {
        validate_document(document)?;
        let document = JournalDocument {
            version: JOURNAL_VERSION,
            entries: document.entries.clone(),
            sends: document.sends.clone(),
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
    // v1 held create-only requests, v2 added exact first-turn bodies, and v3 adds
    // existing-Conversation sends. Missing v3 sends deserialize as an empty list.
    if !matches!(document.version, 1 | 2 | JOURNAL_VERSION)
        || document.entries.len() + document.sends.len() > MAX_PENDING_CREATES
    {
        return Err("Conversation recovery storage has an unsupported version or size".to_owned());
    }
    for entry in &document.entries {
        validate_entry(entry)?;
    }
    for send in &document.sends {
        validate_send(send)?;
    }
    let mut request_ids = document
        .entries
        .iter()
        .map(|entry| entry.request_id.as_str())
        .collect::<HashSet<_>>();
    let mut conversations = HashSet::with_capacity(document.sends.len());
    for send in &document.sends {
        if !request_ids.insert(send.request_id.as_str())
            || !conversations.insert((send.workspace_id.as_str(), send.conversation_id.as_str()))
        {
            return Err(
                "Conversation recovery storage contains duplicate pending requests".to_owned(),
            );
        }
    }
    validate_unique_entries(&document.entries)
}

fn validate_send(send: &PendingConversationSend) -> Result<(), String> {
    let content_valid = valid_send_content(&send.content, &send.workspace_id);
    let refs_valid = valid_send_resource_refs(&send.resource_refs, &send.workspace_id);
    let resource_ref_count = send.resource_refs.as_array().map(Vec::len);
    let encoded_size_ok = serde_json::to_vec(&(
        &send.content,
        &send.resource_refs,
        &send.agent_binding_id,
        &send.presentation_preference,
        &send.attachment_labels,
    ))
    .map_or(false, |bytes| bytes.len() <= MAX_PENDING_SEND_BYTES);
    if !valid_workspace_id(&send.workspace_id)
        || !valid_workspace_id(&send.conversation_id)
        || !valid_request_id(&send.request_id)
        || !content_valid
        || !refs_valid
        || resource_ref_count != Some(send.attachment_labels.len())
        || send
            .agent_binding_id
            .as_ref()
            .is_some_and(|binding| !valid_workspace_id(binding))
        || send
            .presentation_preference
            .as_ref()
            .is_some_and(|value| !matches!(value.as_str(), "AUTO" | "SIMPLE" | "RICH"))
        || send.attachment_labels.len() > 256
        || send
            .attachment_labels
            .iter()
            .any(|label| label.chars().count() > 256 || label.chars().any(char::is_control))
        || !encoded_size_ok
    {
        return Err("Conversation send recovery entry is invalid".to_owned());
    }
    Ok(())
}

fn valid_send_content(content: &serde_json::Value, workspace_id: &str) -> bool {
    let Some(blocks) = content.as_array() else {
        return false;
    };
    if blocks.is_empty() || blocks.len() > 256 {
        return false;
    }
    let mut total_text_bytes = 0_usize;
    for block in blocks {
        let Some(object) = block.as_object() else {
            return false;
        };
        match block.get("kind").and_then(serde_json::Value::as_str) {
            Some("TEXT") if object.len() == 2 && object.contains_key("text") => {
                let Some(text) = block.get("text").and_then(serde_json::Value::as_str) else {
                    return false;
                };
                let Some(next_size) = total_text_bytes.checked_add(text.len()) else {
                    return false;
                };
                total_text_bytes = next_size;
                if total_text_bytes > 1024 * 1024 {
                    return false;
                }
            }
            Some("RESOURCE")
                if object.keys().all(|key| {
                    ["kind", "resource_ref", "display_name"].contains(&key.as_str())
                }) && object.contains_key("resource_ref")
                    && block.get("resource_ref").is_some_and(|reference| {
                        valid_send_resource_ref(reference, workspace_id)
                    })
                    && block.get("display_name").is_none_or(|value| {
                        value.is_null()
                            || value.as_str().is_some_and(|name| {
                                name.chars().count() <= 512 && !name.chars().any(char::is_control)
                            })
                    }) => {}
            _ => return false,
        }
    }
    true
}

fn valid_send_resource_refs(refs: &serde_json::Value, workspace_id: &str) -> bool {
    refs.as_array().is_some_and(|refs| {
        refs.len() <= MAX_PENDING_SEND_RESOURCE_REFS
            && refs
                .iter()
                .all(|reference| valid_send_resource_ref(reference, workspace_id))
    })
}

fn valid_send_resource_ref(reference: &serde_json::Value, workspace_id: &str) -> bool {
    let Some(object) = reference.as_object() else {
        return false;
    };
    object
        .keys()
        .all(|key| ["workspace_id", "resource_id", "revision_id"].contains(&key.as_str()))
        && reference
            .get("workspace_id")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|id| id == workspace_id && valid_workspace_id(id))
        && reference
            .get("resource_id")
            .and_then(serde_json::Value::as_str)
            .is_some_and(valid_workspace_id)
        && reference
            .get("revision_id")
            .is_none_or(|value| value.is_null() || value.as_str().is_some_and(valid_workspace_id))
}

fn validate_unique_entries(entries: &[PendingConversationCreate]) -> Result<(), String> {
    let mut request_ids = HashSet::with_capacity(entries.len());
    let mut command_signatures = HashSet::with_capacity(entries.len());
    for entry in entries {
        if !request_ids.insert(entry.request_id.as_str())
            || !command_signatures.insert(command_signature(entry))
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
        || entry
            .first_turn
            .as_ref()
            .is_some_and(|turn| !valid_first_turn(turn, &entry.workspace_id))
    {
        return Err("Conversation creation recovery entry is invalid".to_owned());
    }
    Ok(())
}

fn command_signature(entry: &PendingConversationCreate) -> String {
    serde_json::to_string(&(&entry.workspace_id, &entry.title, &entry.first_turn))
        .unwrap_or_else(|_| String::new())
}

fn valid_first_turn(turn: &serde_json::Value, workspace_id: &str) -> bool {
    let Some(object) = turn.as_object() else {
        return false;
    };
    if object.len() != 4
        || !object.contains_key("content")
        || !object.contains_key("resourceRefs")
        || !object.contains_key("agentBindingId")
        || !object.contains_key("presentationPreference")
    {
        return false;
    }
    let Some(content) = turn.get("content").and_then(serde_json::Value::as_array) else {
        return false;
    };
    let Some(resource_refs) = turn
        .get("resourceRefs")
        .and_then(serde_json::Value::as_array)
    else {
        return false;
    };
    if content.is_empty() || content.len() > 256 || resource_refs.len() > 256 {
        return false;
    }
    if !turn
        .get("agentBindingId")
        .and_then(serde_json::Value::as_str)
        .is_some_and(valid_workspace_id)
    {
        return false;
    }
    if !matches!(
        turn.get("presentationPreference")
            .and_then(serde_json::Value::as_str),
        Some("AUTO" | "SIMPLE" | "RICH")
    ) {
        return false;
    }
    if resource_refs
        .iter()
        .any(|reference| !valid_first_turn_ref(reference, workspace_id))
    {
        return false;
    }
    for block in content {
        let Some(kind) = block.get("kind").and_then(serde_json::Value::as_str) else {
            return false;
        };
        match kind {
            "TEXT" => {
                let Some(text) = block.get("text").and_then(serde_json::Value::as_str) else {
                    return false;
                };
                if block.as_object().is_none_or(|value| value.len() != 2) || text.len() > 256 * 1024
                {
                    return false;
                }
            }
            "RESOURCE" => {
                let Some(reference) = block.get("resourceRef") else {
                    return false;
                };
                if !valid_first_turn_ref(reference, workspace_id) {
                    return false;
                }
                if block
                    .as_object()
                    .is_none_or(|value| value.len() != 2 && value.len() != 3)
                {
                    return false;
                }
                if block.get("displayName").is_some_and(|name| {
                    !name.is_null() && name.as_str().is_none_or(|value| value.len() > 1024)
                }) {
                    return false;
                }
            }
            _ => return false,
        }
    }
    serde_json::to_vec(turn).is_ok_and(|bytes| bytes.len() <= MAX_FIRST_TURN_BYTES)
}

fn valid_first_turn_ref(reference: &serde_json::Value, workspace_id: &str) -> bool {
    let Some(object) = reference.as_object() else {
        return false;
    };
    if object.len() != 2 && object.len() != 3 {
        return false;
    }
    reference
        .get("workspaceId")
        .and_then(serde_json::Value::as_str)
        == Some(workspace_id)
        && reference
            .get("resourceId")
            .and_then(serde_json::Value::as_str)
            .is_some_and(valid_workspace_id)
        && reference
            .get("revisionId")
            .is_none_or(|value| value.is_null() || value.as_str().is_some_and(valid_workspace_id))
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
            first_turn: None,
        }
    }

    fn first_turn(text: &str) -> serde_json::Value {
        serde_json::json!({
            "content": [{"kind":"TEXT","text":text}],
            "resourceRefs": [],
            "agentBindingId": "binding-1",
            "presentationPreference": "RICH"
        })
    }

    fn pending_send(
        conversation_id: &str,
        request_id: &str,
        text: &str,
    ) -> PendingConversationSend {
        PendingConversationSend {
            workspace_id: "workspace-1".to_owned(),
            conversation_id: conversation_id.to_owned(),
            request_id: request_id.to_owned(),
            content: serde_json::json!([{"kind":"TEXT","text":text}]),
            resource_refs: serde_json::json!([]),
            agent_binding_id: Some("binding-1".to_owned()),
            presentation_preference: Some("RICH".to_owned()),
            attachment_labels: Vec::new(),
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
    fn exact_pending_send_survives_restart_and_one_unresolved_send_is_enforced() {
        let directory = TestDirectory::new();
        let journal = ConversationCreateJournal::new(directory.path());
        let create = entry(
            "workspace-1",
            Some("Existing legacy create"),
            "create-request",
        );
        let pending = pending_send("conversation-1", "send-request", "Exact durable send");
        journal
            .record(create.clone())
            .expect("create entry persists");
        journal
            .record_send(pending.clone())
            .expect("send persists before dispatch");

        let restarted = ConversationCreateJournal::new(directory.path());
        assert_eq!(
            restarted.list().expect("legacy entries retained"),
            vec![create]
        );
        assert_eq!(
            restarted.list_sends().expect("pending send reopens"),
            vec![pending.clone()]
        );
        restarted
            .record_send(pending.clone())
            .expect("same key and body is a replay");
        assert!(
            restarted
                .record_send(pending_send(
                    "conversation-1",
                    "second-request",
                    "Different body"
                ))
                .is_err()
        );
        assert!(
            restarted
                .record_send(pending_send(
                    "conversation-2",
                    "send-request",
                    "Different conversation"
                ))
                .is_err()
        );

        restarted
            .acknowledge_send("workspace-1", "other-conversation", "send-request")
            .expect("wrong Conversation cannot acknowledge the send");
        assert_eq!(
            restarted.list_sends().expect("pending request remains"),
            vec![pending]
        );
        restarted
            .acknowledge_send("workspace-1", "conversation-1", "send-request")
            .expect("exact receipt acknowledgement removes the request");
        assert!(
            restarted
                .list_sends()
                .expect("send acknowledged")
                .is_empty()
        );
    }

    #[test]
    fn pending_send_recovery_rejects_foreign_resources_and_uncontracted_content() {
        let mut pending = pending_send("conversation-1", "send-request", "Exact send");
        pending.resource_refs = serde_json::json!([{
            "workspace_id": "workspace-1",
            "resource_id": "resource-1",
            "revision_id": "revision-1"
        }]);
        pending.attachment_labels = vec!["report.txt".to_owned()];
        assert!(validate_send(&pending).is_ok());

        pending.resource_refs[0]["workspace_id"] = serde_json::json!("workspace-2");
        assert!(validate_send(&pending).is_err());

        pending.resource_refs = serde_json::json!([]);
        pending.attachment_labels.clear();
        pending.content = serde_json::json!([{
            "kind": "TEXT",
            "text": "Exact send",
            "native_session_id": "private-handle"
        }]);
        assert!(validate_send(&pending).is_err());

        pending.content = serde_json::json!([{
            "kind": "RESOURCE",
            "resource_ref": {
                "workspace_id": "workspace-1",
                "resource_id": "resource-1"
            },
            "display_name": "report.txt"
        }]);
        assert!(validate_send(&pending).is_ok());
    }

    #[test]
    fn legacy_v1_entries_remain_readable_and_first_turns_with_shared_titles_are_distinct() {
        let directory = TestDirectory::new();
        let journal = ConversationCreateJournal::new(directory.path());
        journal
            .ensure_directory()
            .expect("journal directory exists");
        fs::write(
            journal_file(directory.path()),
            br#"{"version":1,"entries":[{"workspaceId":"workspace-1","title":"Old create","requestId":"legacy-request"}]}"#,
        ).expect("write a v1 fixture");
        let legacy = journal.list().expect("v1 entry stays recoverable");
        assert_eq!(legacy.len(), 1);
        assert_eq!(legacy[0].first_turn, None);

        let mut first = entry("workspace-1", Some("Shared title"), "first-message");
        first.first_turn = Some(first_turn("First exact message"));
        let mut second = entry("workspace-1", Some("Shared title"), "second-message");
        second.first_turn = Some(first_turn("Second exact message"));
        journal
            .record(first.clone())
            .expect("first exact body is recorded");
        journal
            .record(second.clone())
            .expect("different first body does not collide by title");
        assert_eq!(
            journal.list().expect("list migrated journal"),
            vec![legacy[0].clone(), first, second]
        );
        let document: JournalDocument = serde_json::from_slice(
            &fs::read(journal_file(directory.path())).expect("read migrated journal"),
        )
        .expect("journal remains valid");
        assert_eq!(document.version, JOURNAL_VERSION);
    }

    #[test]
    fn legacy_v2_journal_migrates_to_v3_without_dropping_pending_create() {
        let directory = TestDirectory::new();
        let journal = ConversationCreateJournal::new(directory.path());
        journal
            .ensure_directory()
            .expect("journal directory exists");
        let pending_create = entry("workspace-1", Some("Existing draft"), "create-1");
        let legacy_v2 = serde_json::json!({
            "version": 2,
            "entries": [pending_create],
        });
        fs::write(
            journal_file(directory.path()),
            serde_json::to_vec(&legacy_v2).expect("encode v2 fixture"),
        )
        .expect("write legacy v2 document");

        assert_eq!(
            journal.list().expect("read v2 pending create"),
            vec![entry("workspace-1", Some("Existing draft"), "create-1")]
        );
        let pending_send = pending_send("conversation-1", "send-1", "Draft body");
        journal
            .record_send(pending_send.clone())
            .expect("migrate v2 document when recording send");

        let migrated: JournalDocument = serde_json::from_slice(
            &fs::read(journal_file(directory.path())).expect("read migrated journal"),
        )
        .expect("parse v3 document");
        assert_eq!(migrated.version, JOURNAL_VERSION);
        assert_eq!(migrated.entries, vec![pending_create]);
        assert_eq!(migrated.sends, vec![pending_send]);
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
            br#"{"version":4,"entries":[]}"#.as_slice(),
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
