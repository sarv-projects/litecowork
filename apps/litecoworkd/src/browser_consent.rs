//! Browser profile, consent, and observation-scope policy.
//!
//! `docs/CHROME-BROWSER-LLD.md` is explicit about the boundaries this module keeps:
//!
//! * Chrome 136+ disregards remote debugging switches against its default user
//!   directory, and Playwright does not support automating the default Chrome user
//!   profile. LiteCowork therefore never points a provider at the default profile and
//!   never copies encrypted cookie material.
//! * Managed profiles are separate, user-visible, data-minimized and
//!   permission-controlled.
//! * Connecting an already running Chrome is a *consent-based attach*: per-session,
//!   explicit, revocable, version-gated, and accompanied by a high-trust warning.
//! * Observation happens only inside an explicitly authorized scope.
//!
//! Every refusal below is fail-closed. Nothing here grants browser authority; it only
//! decides whether a requested provider operation may be attempted at all.

use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::{
    fs,
    path::{Path, PathBuf},
};

/// The browser execution mode a session runs in.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BrowserMode {
    /// An isolated browser data directory owned by LiteCowork.
    ManagedChromium,
    /// A consent-based attach to an already running compatible Chrome.
    ConnectExistingChrome,
    /// The user owns every action; the agent may only observe authorized scope.
    ManualOnly,
    /// A non-interactive managed browser for authorized background steps.
    HeadlessTask,
    /// No qualified provider is available on this host.
    Unavailable,
}

impl BrowserMode {
    /// Whether a mode may be driven by an agent at all.
    pub fn allows_agent_input(self) -> bool {
        matches!(
            self,
            Self::ManagedChromium | Self::ConnectExistingChrome | Self::HeadlessTask
        )
    }
}

/// Why an attach to an existing Chrome was refused.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ChromeAttachRefusal {
    /// The candidate directory is the default Chrome user-data directory. Chrome 136+
    /// ignores remote debugging there and Playwright refuses to automate it.
    DefaultUserProfileNotAutomatable,
    /// The candidate directory is not a LiteCowork-managed profile.
    ProfileNotManagedByLiteCowork,
    /// No per-session consent exists for this attach.
    ConsentMissing,
    /// The consent was revoked.
    ConsentRevoked,
    /// The consent expired.
    ConsentExpired,
    /// The running Chrome is older than the version this attach path supports.
    BrowserVersionTooOld {
        required_major: u32,
        actual_major: u32,
    },
    /// The high-trust warning was not acknowledged.
    HighTrustAcknowledgementMissing,
    /// The attached profile does not match the consented profile reference.
    UnexpectedProfile,
}

impl ChromeAttachRefusal {
    pub fn reason_code(&self) -> &'static str {
        match self {
            Self::DefaultUserProfileNotAutomatable => {
                "BROWSER_DEFAULT_PROFILE_NOT_AUTOMATABLE"
            }
            Self::ProfileNotManagedByLiteCowork => "BROWSER_PROFILE_NOT_MANAGED",
            Self::ConsentMissing => "BROWSER_CONSENT_MISSING",
            Self::ConsentRevoked => "BROWSER_CONSENT_REVOKED",
            Self::ConsentExpired => "BROWSER_CONSENT_EXPIRED",
            Self::BrowserVersionTooOld { .. } => "BROWSER_VERSION_UNSUPPORTED",
            Self::HighTrustAcknowledgementMissing => "BROWSER_HIGH_TRUST_NOT_ACKNOWLEDGED",
            Self::UnexpectedProfile => "BROWSER_UNEXPECTED_PROFILE",
        }
    }
}

/// Per-session consent to attach to an already running Chrome.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ChromeAttachConsent {
    pub consent_id: String,
    pub workspace_id: String,
    /// The Chrome major version the user saw and consented to.
    pub consented_browser_major_version: u32,
    /// Opaque profile reference the consent is bound to. Never a filesystem path.
    pub authorized_profile_ref: String,
    pub granted_at_ms: i64,
    pub expires_at_ms: i64,
    /// The user acknowledged that an existing session may expose open tabs, session
    /// storage, and authenticated sites.
    pub high_trust_acknowledged: bool,
    pub revoked_at_ms: Option<i64>,
}

/// Minimum Chrome major version this attach path supports.
///
/// Chrome 144+ is the first release whose DevTools auto-connect surface is a
/// candidate for an existing-session attach. Anything older is refused rather than
/// attempted through an unsupported switch.
pub const MIN_EXISTING_CHROME_MAJOR_VERSION: u32 = 144;

/// The outcome of an attach request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ChromeAttachDecision {
    Attached { consent_id: String },
    Refused(ChromeAttachRefusal),
}

/// Decide whether LiteCowork may attach to an already running Chrome.
///
/// The candidate profile directory must be a LiteCowork-managed isolated profile, it
/// must not be the default Chrome user-data directory, the consent must exist, be
/// unrevoked, unexpired, bound to the observed Chrome version, and the high-trust
/// acknowledgement must be present.
pub fn decide_existing_chrome_attach(
    candidate_profile_directory: &Path,
    consent: Option<&ChromeAttachConsent>,
    observed_browser_major_version: u32,
    now_ms: i64,
) -> ChromeAttachDecision {
    if is_default_chrome_user_data_directory(candidate_profile_directory) {
        return ChromeAttachDecision::Refused(
            ChromeAttachRefusal::DefaultUserProfileNotAutomatable,
        );
    }
    if !is_litecowork_managed_profile(candidate_profile_directory) {
        return ChromeAttachDecision::Refused(
            ChromeAttachRefusal::ProfileNotManagedByLiteCowork,
        );
    }
    let Some(consent) = consent else {
        return ChromeAttachDecision::Refused(ChromeAttachRefusal::ConsentMissing);
    };
    if consent.revoked_at_ms.is_some() {
        return ChromeAttachDecision::Refused(ChromeAttachRefusal::ConsentRevoked);
    }
    if now_ms >= consent.expires_at_ms {
        return ChromeAttachDecision::Refused(ChromeAttachRefusal::ConsentExpired);
    }
    if observed_browser_major_version < MIN_EXISTING_CHROME_MAJOR_VERSION {
        return ChromeAttachDecision::Refused(ChromeAttachRefusal::BrowserVersionTooOld {
            required_major: MIN_EXISTING_CHROME_MAJOR_VERSION,
            actual_major: observed_browser_major_version,
        });
    }
    if observed_browser_major_version != consent.consented_browser_major_version {
        return ChromeAttachDecision::Refused(ChromeAttachRefusal::BrowserVersionTooOld {
            required_major: consent.consented_browser_major_version,
            actual_major: observed_browser_major_version,
        });
    }
    if !consent.high_trust_acknowledged {
        return ChromeAttachDecision::Refused(
            ChromeAttachRefusal::HighTrustAcknowledgementMissing,
        );
    }
    ChromeAttachDecision::Attached {
        consent_id: consent.consent_id.clone(),
    }
}

/// The marker file that identifies a LiteCowork-managed browser profile directory.
const MANAGED_PROFILE_MARKER: &str = ".litecowork-managed-profile";

/// Create an isolated, LiteCowork-managed browser profile directory.
///
/// The directory is created empty and is marked as managed. It is never seeded from a
/// default user profile and never receives credential material.
pub fn create_managed_profile(
    root: &Path,
    profile_ref: &str,
) -> Result<PathBuf, BrowserProfileError> {
    if profile_ref.trim().is_empty()
        || profile_ref.len() > 128
        || !profile_ref
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(BrowserProfileError::InvalidProfileRef);
    }
    if is_default_chrome_user_data_directory(root) {
        return Err(BrowserProfileError::DefaultUserProfileRefused);
    }
    fs::create_dir_all(root).map_err(BrowserProfileError::Io)?;
    let directory = root.join(format!("profile-{profile_ref}"));
    fs::create_dir_all(&directory).map_err(BrowserProfileError::Io)?;
    let marker = directory.join(MANAGED_PROFILE_MARKER);
    fs::write(&marker, b"litecowork-managed-profile\n").map_err(BrowserProfileError::Io)?;
    Ok(directory)
}

/// Whether a directory is a LiteCowork-managed browser profile.
pub fn is_litecowork_managed_profile(candidate: &Path) -> bool {
    candidate.join(MANAGED_PROFILE_MARKER).is_file()
}

/// The default Chrome/Chromium user-data directories for the current platform.
///
/// The list is intentionally conservative: it names the directories Chrome itself
/// uses, plus the Chromium and Edge variants that share the same restriction class.
pub fn default_chrome_user_data_directories() -> Vec<PathBuf> {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from);
    let Some(home) = home else {
        return Vec::new();
    };
    let mut directories = vec![
        home.join(".config/google-chrome"),
        home.join(".config/chromium"),
        home.join(".config/microsoft-edge"),
        home.join("Library/Application Support/Google/Chrome"),
        home.join("Library/Application Support/Chromium"),
        home.join("AppData/Local/Google/Chrome/User Data"),
        home.join("AppData/Local/Chromium/User Data"),
    ];
    if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) {
        directories.push(local_app_data.join("Google/Chrome/User Data"));
        directories.push(local_app_data.join("Chromium/User Data"));
    }
    directories
}

/// Whether a candidate directory is, or is inside, a default Chrome user-data
/// directory.
pub fn is_default_chrome_user_data_directory(candidate: &Path) -> bool {
    let candidate = normalize(candidate);
    default_chrome_user_data_directories()
        .iter()
        .any(|default| {
            let default = normalize(default);
            candidate == default || candidate.starts_with(&default)
        })
}

fn normalize(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

/// Errors raised while managing a browser profile directory.
#[derive(Debug)]
pub enum BrowserProfileError {
    InvalidProfileRef,
    DefaultUserProfileRefused,
    Io(std::io::Error),
}

impl std::fmt::Display for BrowserProfileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidProfileRef => write!(f, "invalid browser profile reference"),
            Self::DefaultUserProfileRefused => {
                write!(f, "the default Chrome user profile cannot be used")
            }
            Self::Io(error) => write!(f, "browser profile I/O failed: {error}"),
        }
    }
}

impl std::error::Error for BrowserProfileError {}

/// A file offered for a browser profile import.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProfileImportEntry {
    /// Path relative to the export root.
    pub relative_path: String,
    pub size_bytes: u64,
}

/// The result of previewing a profile import.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProfileImportPreview {
    pub accepted: Vec<String>,
    pub rejected: Vec<ProfileImportRejection>,
    pub preview_digest: String,
}

/// Why one import entry was rejected.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProfileImportRejection {
    /// Credential or session material is never imported.
    CredentialMaterial,
    /// The entry is not on the allowlist of non-secret bookmarks/settings files.
    NotAllowlisted,
    /// The entry exceeded the per-file or total size bound.
    TooLarge,
    /// The export contained more entries than the bound allows.
    TooManyEntries,
    /// The relative path was not a plain relative path.
    InvalidPath,
}

/// Bounds for a profile import preview.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProfileImportLimits {
    pub max_entries: usize,
    pub max_entry_bytes: u64,
    pub max_total_bytes: u64,
}

impl Default for ProfileImportLimits {
    fn default() -> Self {
        Self {
            max_entries: 256,
            max_entry_bytes: 8 * 1024 * 1024,
            max_total_bytes: 32 * 1024 * 1024,
        }
    }
}

/// Preview a browser profile import without touching the filesystem.
///
/// Only bookmarks and settings are eligible. Credential, cookie, history, session,
/// and password material is rejected by name so that an import can never become a
/// sign-in import.
pub fn preview_profile_import(
    entries: &[ProfileImportEntry],
    limits: ProfileImportLimits,
) -> ProfileImportPreview {
    let mut accepted = Vec::new();
    let mut rejected = Vec::new();
    let mut total = 0_u64;
    for entry in entries {
        if entries.len() > limits.max_entries {
            rejected.push(ProfileImportRejection::TooManyEntries);
            break;
        }
        let name = entry.relative_path.to_ascii_lowercase();
        let file_name = name.rsplit('/').next().unwrap_or_default().to_owned();
        if !is_safe_relative_path(&entry.relative_path) {
            rejected.push(ProfileImportRejection::InvalidPath);
            continue;
        }
        if is_credential_material(&file_name) {
            rejected.push(ProfileImportRejection::CredentialMaterial);
            continue;
        }
        if !is_allowlisted_profile_file(&file_name) {
            rejected.push(ProfileImportRejection::NotAllowlisted);
            continue;
        }
        if entry.size_bytes > limits.max_entry_bytes {
            rejected.push(ProfileImportRejection::TooLarge);
            continue;
        }
        let Some(next_total) = total.checked_add(entry.size_bytes) else {
            rejected.push(ProfileImportRejection::TooLarge);
            continue;
        };
        if next_total > limits.max_total_bytes {
            rejected.push(ProfileImportRejection::TooLarge);
            continue;
        }
        total = next_total;
        accepted.push(entry.relative_path.clone());
    }
    accepted.sort();
    rejected.sort_by_key(|rejection| format!("{rejection:?}"));
    let preview_digest = format!(
        "sha256:{}",
        hex::encode(sha2::Sha256::digest(
            accepted.join("\n").as_bytes()
        ))
    );
    ProfileImportPreview {
        accepted,
        rejected,
        preview_digest,
    }
}

fn is_safe_relative_path(value: &str) -> bool {
    if value.is_empty() || value.starts_with('/') || value.contains('\\') {
        return false;
    }
    value.split('/').all(|component| {
        !component.is_empty()
            && component != "."
            && component != ".."
            && !component.ends_with(['.', ' '])
    })
}

fn is_credential_material(file_name: &str) -> bool {
    const CREDENTIAL_MARKERS: [&str; 14] = [
        "cookie",
        "login data",
        "logindata",
        "password",
        "credential",
        "web data",
        "webdata",
        "local state",
        "localstate",
        "history",
        "session",
        "token",
        "secret",
        "key",
    ];
    CREDENTIAL_MARKERS
        .iter()
        .any(|marker| file_name.contains(marker))
}

fn is_allowlisted_profile_file(file_name: &str) -> bool {
    matches!(
        file_name,
        "bookmarks"
            | "bookmarks.bak"
            | "preferences"
            | "preferences.bak"
            | "secure_preferences"
            | "favicons"
            | "favicons.db"
    )
}

/// A sensitive region that must never be captured.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SensitiveRegionKind {
    PasswordField,
    PaymentField,
    IdentityPrompt,
    Captcha,
    OneTimeCode,
    Clipboard,
}

/// A sensitive region inside an observed surface.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SensitiveRegion {
    pub kind: SensitiveRegionKind,
    pub label: String,
}

/// Classify an observed element label or accessibility role as a sensitive region.
///
/// Password fields, payments, identity prompts, CAPTCHA, one-time codes, and the
/// clipboard require human handling. LiteCowork never promises to work around them.
pub fn classify_sensitive_region(label: &str) -> Option<SensitiveRegionKind> {
    let lowered = label.to_ascii_lowercase();
    if lowered.contains("password") || lowered.contains("passwd") {
        return Some(SensitiveRegionKind::PasswordField);
    }
    if lowered.contains("captcha") || lowered.contains("recaptcha") {
        return Some(SensitiveRegionKind::Captcha);
    }
    if lowered.contains("one-time") || lowered.contains("otp") || lowered.contains("mfa") {
        return Some(SensitiveRegionKind::OneTimeCode);
    }
    if lowered.contains("card number")
        || lowered.contains("credit card")
        || lowered.contains("payment")
        || lowered.contains("cvv")
        || lowered.contains("cvc")
    {
        return Some(SensitiveRegionKind::PaymentField);
    }
    if lowered.contains("clipboard") {
        return Some(SensitiveRegionKind::Clipboard);
    }
    if lowered.contains("identity verification")
        || lowered.contains("verify your identity")
        || lowered.contains("government id")
        || lowered.contains("two-factor")
    {
        return Some(SensitiveRegionKind::IdentityPrompt);
    }
    None
}

/// One authorized observation scope.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObservationScope {
    pub session_id: String,
    /// Browser window or tab the scope covers. `None` means the whole session.
    pub window_id: Option<String>,
    /// Exact origin the scope covers. `None` means every origin in the session.
    pub origin: Option<String>,
    pub allow_screenshot: bool,
    pub allow_dom: bool,
    pub allow_accessibility_tree: bool,
    /// Regions inside the scope that must never be captured.
    pub excluded_regions: Vec<SensitiveRegion>,
}

/// An explicit, revocable, expiring observation consent.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObservationConsent {
    pub consent_id: String,
    pub workspace_id: String,
    pub scopes: Vec<ObservationScope>,
    pub granted_at_ms: i64,
    pub expires_at_ms: i64,
    pub revoked_at_ms: Option<i64>,
}

/// The kind of capture an observation request asks for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObservationKind {
    Screenshot,
    Dom,
    AccessibilityTree,
}

/// A request to observe one browser surface.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObservationRequest {
    pub session_id: String,
    pub window_id: Option<String>,
    pub origin: Option<String>,
    pub kind: ObservationKind,
}

/// Why an observation was refused.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ObservationRefusal {
    ConsentMissing,
    ConsentRevoked,
    ConsentExpired,
    SessionNotInScope,
    WindowNotInScope,
    OriginNotInScope,
    CaptureKindNotPermitted,
    SensitiveRegion,
    ProviderUnavailable,
}

impl ObservationRefusal {
    pub fn reason_code(&self) -> &'static str {
        match self {
            Self::ConsentMissing => "OBSERVATION_CONSENT_MISSING",
            Self::ConsentRevoked => "OBSERVATION_CONSENT_REVOKED",
            Self::ConsentExpired => "OBSERVATION_CONSENT_EXPIRED",
            Self::SessionNotInScope => "OBSERVATION_SESSION_NOT_IN_SCOPE",
            Self::WindowNotInScope => "OBSERVATION_WINDOW_NOT_IN_SCOPE",
            Self::OriginNotInScope => "OBSERVATION_ORIGIN_NOT_IN_SCOPE",
            Self::CaptureKindNotPermitted => "OBSERVATION_CAPTURE_KIND_NOT_PERMITTED",
            Self::SensitiveRegion => "OBSERVATION_SENSITIVE_REGION",
            Self::ProviderUnavailable => "OBSERVATION_PROVIDER_UNAVAILABLE",
        }
    }
}

/// An observation that is authorized to be captured.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorizedObservation {
    pub consent_id: String,
    pub session_id: String,
    pub kind: ObservationKind,
    pub window_id: Option<String>,
    pub origin: Option<String>,
}

/// Authorize an observation against an explicit consent.
///
/// The consent must exist, be unrevoked, unexpired, and cover the requested session,
/// window, origin, and capture kind. A scope that names no window or origin covers
/// the whole session; a scope that names one requires an exact match.
pub fn authorize_observation(
    request: &ObservationRequest,
    consent: Option<&ObservationConsent>,
    now_ms: i64,
    provider_available: bool,
) -> Result<AuthorizedObservation, ObservationRefusal> {
    if !provider_available {
        return Err(ObservationRefusal::ProviderUnavailable);
    }
    let Some(consent) = consent else {
        return Err(ObservationRefusal::ConsentMissing);
    };
    if consent.revoked_at_ms.is_some() {
        return Err(ObservationRefusal::ConsentRevoked);
    }
    if now_ms >= consent.expires_at_ms {
        return Err(ObservationRefusal::ConsentExpired);
    }
    let scope = consent
        .scopes
        .iter()
        .find(|scope| scope.session_id == request.session_id)
        .ok_or(ObservationRefusal::SessionNotInScope)?;
    if let Some(window_id) = &scope.window_id {
        if request.window_id.as_ref() != Some(window_id) {
            return Err(ObservationRefusal::WindowNotInScope);
        }
    }
    if let Some(origin) = &scope.origin {
        if request.origin.as_ref() != Some(origin) {
            return Err(ObservationRefusal::OriginNotInScope);
        }
    }
    let permitted = match request.kind {
        ObservationKind::Screenshot => scope.allow_screenshot,
        ObservationKind::Dom => scope.allow_dom,
        ObservationKind::AccessibilityTree => scope.allow_accessibility_tree,
    };
    if !permitted {
        return Err(ObservationRefusal::CaptureKindNotPermitted);
    }
    Ok(AuthorizedObservation {
        consent_id: consent.consent_id.clone(),
        session_id: request.session_id.clone(),
        kind: request.kind,
        window_id: request.window_id.clone(),
        origin: request.origin.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn consent(now: i64) -> ChromeAttachConsent {
        ChromeAttachConsent {
            consent_id: "consent-1".to_owned(),
            workspace_id: "workspace-1".to_owned(),
            consented_browser_major_version: MIN_EXISTING_CHROME_MAJOR_VERSION,
            authorized_profile_ref: "profile-1".to_owned(),
            granted_at_ms: now,
            expires_at_ms: now + 60_000,
            high_trust_acknowledged: true,
            revoked_at_ms: None,
        }
    }

    #[test]
    fn a_managed_profile_directory_is_created_and_marked() {
        let temp = tempdir().unwrap();
        let directory = create_managed_profile(temp.path(), "profile-1").expect("managed profile");
        assert!(directory.is_dir());
        assert!(is_litecowork_managed_profile(&directory));
        assert!(!is_litecowork_managed_profile(temp.path()));
    }

    #[test]
    fn the_default_chrome_user_profile_is_never_automatable() {
        let home = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(PathBuf::from)
            .expect("home directory");
        let default_profile = home.join(".config/google-chrome");
        assert!(is_default_chrome_user_data_directory(&default_profile));
        assert!(
            is_default_chrome_user_data_directory(&default_profile.join("Default"))
        );

        let temp = tempdir().unwrap();
        let directory = create_managed_profile(temp.path(), "profile-1").expect("managed profile");
        assert!(!is_default_chrome_user_data_directory(&directory));
    }

    #[test]
    fn an_existing_chrome_attach_requires_every_consent_element() {
        let temp = tempdir().unwrap();
        let directory = create_managed_profile(temp.path(), "profile-1").expect("managed profile");
        let now = 1_000;
        let consent = consent(now);

        assert_eq!(
            decide_existing_chrome_attach(&directory, Some(&consent), MIN_EXISTING_CHROME_MAJOR_VERSION, now),
            ChromeAttachDecision::Attached {
                consent_id: "consent-1".to_owned()
            }
        );

        assert_eq!(
            decide_existing_chrome_attach(&directory, None, MIN_EXISTING_CHROME_MAJOR_VERSION, now),
            ChromeAttachDecision::Refused(ChromeAttachRefusal::ConsentMissing)
        );

        let mut revoked = consent.clone();
        revoked.revoked_at_ms = Some(now);
        assert_eq!(
            decide_existing_chrome_attach(&directory, Some(&revoked), MIN_EXISTING_CHROME_MAJOR_VERSION, now),
            ChromeAttachDecision::Refused(ChromeAttachRefusal::ConsentRevoked)
        );

        assert_eq!(
            decide_existing_chrome_attach(&directory, Some(&consent), MIN_EXISTING_CHROME_MAJOR_VERSION, now + 60_000),
            ChromeAttachDecision::Refused(ChromeAttachRefusal::ConsentExpired)
        );

        let mut unacknowledged = consent.clone();
        unacknowledged.high_trust_acknowledged = false;
        assert_eq!(
            decide_existing_chrome_attach(&directory, Some(&unacknowledged), MIN_EXISTING_CHROME_MAJOR_VERSION, now),
            ChromeAttachDecision::Refused(
                ChromeAttachRefusal::HighTrustAcknowledgementMissing
            )
        );
    }

    #[test]
    fn an_existing_chrome_attach_refuses_an_old_browser_version() {
        let temp = tempdir().unwrap();
        let directory = create_managed_profile(temp.path(), "profile-1").expect("managed profile");
        let now = 1_000;
        let consent = consent(now);
        assert_eq!(
            decide_existing_chrome_attach(&directory, Some(&consent), 136, now),
            ChromeAttachDecision::Refused(ChromeAttachRefusal::BrowserVersionTooOld {
                required_major: MIN_EXISTING_CHROME_MAJOR_VERSION,
                actual_major: 136,
            })
        );
    }

    #[test]
    fn an_existing_chrome_attach_refuses_an_unmanaged_profile() {
        let temp = tempdir().unwrap();
        let directory = temp.path().join("not-managed");
        std::fs::create_dir_all(&directory).unwrap();
        let now = 1_000;
        assert_eq!(
            decide_existing_chrome_attach(&directory, Some(&consent(now)), MIN_EXISTING_CHROME_MAJOR_VERSION, now),
            ChromeAttachDecision::Refused(
                ChromeAttachRefusal::ProfileNotManagedByLiteCowork
            )
        );
    }

    #[test]
    fn a_profile_import_never_accepts_credential_material() {
        let entries = vec![
            ProfileImportEntry {
                relative_path: "Bookmarks".to_owned(),
                size_bytes: 128,
            },
            ProfileImportEntry {
                relative_path: "Default/Cookies".to_owned(),
                size_bytes: 128,
            },
            ProfileImportEntry {
                relative_path: "Default/Login Data".to_owned(),
                size_bytes: 128,
            },
            ProfileImportEntry {
                relative_path: "Default/Preferences".to_owned(),
                size_bytes: 128,
            },
        ];
        let preview = preview_profile_import(&entries, ProfileImportLimits::default());
        assert_eq!(preview.accepted, vec!["Bookmarks", "Default/Preferences"]);
        assert_eq!(
            preview.rejected,
            vec![
                ProfileImportRejection::CredentialMaterial,
                ProfileImportRejection::CredentialMaterial
            ]
        );
        assert!(preview.preview_digest.starts_with("sha256:"));
    }

    #[test]
    fn a_profile_import_rejects_traversal_and_oversized_entries() {
        let entries = vec![
            ProfileImportEntry {
                relative_path: "../Bookmarks".to_owned(),
                size_bytes: 10,
            },
            ProfileImportEntry {
                relative_path: "Bookmarks".to_owned(),
                size_bytes: 1024,
            },
        ];
        let limits = ProfileImportLimits {
            max_entries: 8,
            max_entry_bytes: 64,
            max_total_bytes: 1024,
        };
        let preview = preview_profile_import(&entries, limits);
        assert!(preview.accepted.is_empty());
        assert_eq!(
            preview.rejected,
            vec![
                ProfileImportRejection::InvalidPath,
                ProfileImportRejection::TooLarge
            ]
        );
    }

    #[test]
    fn sensitive_regions_are_classified_and_refused() {
        assert_eq!(
            classify_sensitive_region("Password"),
            Some(SensitiveRegionKind::PasswordField)
        );
        assert_eq!(
            classify_sensitive_region("Card number"),
            Some(SensitiveRegionKind::PaymentField)
        );
        assert_eq!(
            classify_sensitive_region("reCAPTCHA"),
            Some(SensitiveRegionKind::Captcha)
        );
        assert_eq!(
            classify_sensitive_region("Verify your identity"),
            Some(SensitiveRegionKind::IdentityPrompt)
        );
        assert_eq!(classify_sensitive_region("Submit"), None);
    }

    #[test]
    fn observation_requires_an_explicit_unexpired_consent() {
        let request = ObservationRequest {
            session_id: "session-1".to_owned(),
            window_id: Some("tab-1".to_owned()),
            origin: Some("https://example.test".to_owned()),
            kind: ObservationKind::Screenshot,
        };
        let now = 1_000;
        let consent = ObservationConsent {
            consent_id: "consent-1".to_owned(),
            workspace_id: "workspace-1".to_owned(),
            scopes: vec![ObservationScope {
                session_id: "session-1".to_owned(),
                window_id: Some("tab-1".to_owned()),
                origin: Some("https://example.test".to_owned()),
                allow_screenshot: true,
                allow_dom: true,
                allow_accessibility_tree: false,
                excluded_regions: vec![],
            }],
            granted_at_ms: now,
            expires_at_ms: now + 60_000,
            revoked_at_ms: None,
        };

        assert!(authorize_observation(&request, Some(&consent), now, true).is_ok());
        assert_eq!(
            authorize_observation(&request, None, now, true).unwrap_err(),
            ObservationRefusal::ConsentMissing
        );

        let mut revoked = consent.clone();
        revoked.revoked_at_ms = Some(now);
        assert_eq!(
            authorize_observation(&request, Some(&revoked), now, true).unwrap_err(),
            ObservationRefusal::ConsentRevoked
        );
        assert_eq!(
            authorize_observation(&request, Some(&consent), now + 60_000, true).unwrap_err(),
            ObservationRefusal::ConsentExpired
        );
        assert_eq!(
            authorize_observation(&request, Some(&consent), now, false).unwrap_err(),
            ObservationRefusal::ProviderUnavailable
        );
    }

    #[test]
    fn observation_refuses_an_unscoped_window_origin_or_capture_kind() {
        let now = 1_000;
        let consent = ObservationConsent {
            consent_id: "consent-1".to_owned(),
            workspace_id: "workspace-1".to_owned(),
            scopes: vec![ObservationScope {
                session_id: "session-1".to_owned(),
                window_id: Some("tab-1".to_owned()),
                origin: Some("https://example.test".to_owned()),
                allow_screenshot: true,
                allow_dom: false,
                allow_accessibility_tree: false,
                excluded_regions: vec![],
            }],
            granted_at_ms: now,
            expires_at_ms: now + 60_000,
            revoked_at_ms: None,
        };

        let other_window = ObservationRequest {
            session_id: "session-1".to_owned(),
            window_id: Some("tab-2".to_owned()),
            origin: Some("https://example.test".to_owned()),
            kind: ObservationKind::Screenshot,
        };
        assert_eq!(
            authorize_observation(&other_window, Some(&consent), now, true).unwrap_err(),
            ObservationRefusal::WindowNotInScope
        );

        let other_origin = ObservationRequest {
            session_id: "session-1".to_owned(),
            window_id: Some("tab-1".to_owned()),
            origin: Some("https://other.test".to_owned()),
            kind: ObservationKind::Screenshot,
        };
        assert_eq!(
            authorize_observation(&other_origin, Some(&consent), now, true).unwrap_err(),
            ObservationRefusal::OriginNotInScope
        );

        let dom = ObservationRequest {
            session_id: "session-1".to_owned(),
            window_id: Some("tab-1".to_owned()),
            origin: Some("https://example.test".to_owned()),
            kind: ObservationKind::Dom,
        };
        assert_eq!(
            authorize_observation(&dom, Some(&consent), now, true).unwrap_err(),
            ObservationRefusal::CaptureKindNotPermitted
        );

        let other_session = ObservationRequest {
            session_id: "session-2".to_owned(),
            window_id: Some("tab-1".to_owned()),
            origin: Some("https://example.test".to_owned()),
            kind: ObservationKind::Screenshot,
        };
        assert_eq!(
            authorize_observation(&other_session, Some(&consent), now, true).unwrap_err(),
            ObservationRefusal::SessionNotInScope
        );
    }
}
