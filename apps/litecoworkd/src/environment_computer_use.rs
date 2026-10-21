//! Desktop computer use and opt-in observation policy.
//!
//! `docs/ENVIRONMENTS.md` and `docs/CHROME-BROWSER-LLD.md` require that a Coworker
//! has no implicit computer access, that observation happens only inside an explicitly
//! authorized scope, that screen capture permissions are separate and revocable, and
//! that an unsupported OS or action is surfaced instead of simulated.
//!
//! This module is the policy boundary. It decides whether a requested computer action
//! or observation may be attempted. It is not a provider: no qualified capture or
//! input-injection backend is wired in this build, so the honest probe reports the
//! provider as unavailable and every request is refused.

use crate::browser_consent::{SensitiveRegion, SensitiveRegionKind};

/// Whether a qualified computer provider exists on this host.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ComputerProviderAvailability {
    /// A qualified provider exists and may be used.
    Available,
    /// The host has no qualified provider for this platform.
    UnsupportedPlatform,
    /// A provider exists but the operating system has not granted the permission.
    PermissionRequired,
    /// No provider is reachable right now.
    Unavailable,
}

/// The capabilities of the local computer provider.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComputerProviderCapabilities {
    pub platform: String,
    pub availability: ComputerProviderAvailability,
    pub screen_capture_available: bool,
    pub input_injection_available: bool,
    /// Whether a display server was detected at all.
    pub display_present: bool,
    pub reasons: Vec<String>,
}

/// Probe the local computer provider.
///
/// The probe never guesses. A platform LiteCowork has not qualified is reported as
/// unsupported, and a missing display server is reported as unavailable, so that the
/// product can show a truthful reason instead of a phantom cursor.
pub fn probe_computer_provider() -> ComputerProviderCapabilities {
    let platform = std::env::consts::OS.to_owned();
    let mut reasons = Vec::new();

    #[cfg(target_os = "linux")]
    {
        let display_present =
            std::env::var_os("DISPLAY").is_some() || std::env::var_os("WAYLAND_DISPLAY").is_some();
        let availability = if display_present {
            reasons.push(
                "the XDG desktop portal screen-cast and remote-desktop paths are not qualified"
                    .to_owned(),
            );
            ComputerProviderAvailability::UnsupportedPlatform
        } else {
            reasons.push("no display server is available to this session".to_owned());
            ComputerProviderAvailability::Unavailable
        };
        ComputerProviderCapabilities {
            platform,
            availability,
            screen_capture_available: false,
            input_injection_available: false,
            display_present,
            reasons,
        }
    }

    #[cfg(not(target_os = "linux"))]
    {
        reasons.push(
            "no qualified computer-use provider is implemented for this platform".to_owned(),
        );
        ComputerProviderCapabilities {
            platform,
            availability: ComputerProviderAvailability::UnsupportedPlatform,
            screen_capture_available: false,
            input_injection_available: false,
            display_present: false,
            reasons,
        }
    }
}

/// What caused a computer-use request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ComputerUseTrigger {
    /// The user opened the Quick Entry surface.
    QuickEntryOpened,
    /// The user opened the Workbench.
    WorkbenchOpened,
    /// A Task explicitly asked for a computer action.
    TaskRequest,
    /// A routine or automation asked for a computer action.
    ScheduledRequest,
}

/// One authorized observation scope.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComputerObservationScope {
    pub scope_id: String,
    /// The application the scope covers. `None` covers every application.
    pub application_id: Option<String>,
    /// The window the scope covers. `None` covers every window of the application.
    pub window_id: Option<String>,
    pub allow_screen_capture: bool,
    pub allow_input_injection: bool,
    /// Regions inside the scope that must never be captured.
    pub excluded_regions: Vec<SensitiveRegion>,
}

/// An explicit, revocable, expiring computer-observation consent.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComputerObservationConsent {
    pub consent_id: String,
    pub workspace_id: String,
    pub scopes: Vec<ComputerObservationScope>,
    pub granted_at_ms: i64,
    pub expires_at_ms: i64,
    pub revoked_at_ms: Option<i64>,
}

/// A requested computer action.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ComputerAction {
    Click {
        application_id: String,
        window_id: String,
        x: i32,
        y: i32,
    },
    TypeText {
        application_id: String,
        window_id: String,
        text: String,
    },
    KeyPress {
        application_id: String,
        window_id: String,
        key: String,
    },
    Scroll {
        application_id: String,
        window_id: String,
        delta_y: i32,
    },
    Drag {
        application_id: String,
        window_id: String,
        from_x: i32,
        from_y: i32,
        to_x: i32,
        to_y: i32,
    },
    CaptureScreen,
    CaptureWindow {
        application_id: String,
        window_id: String,
    },
    FocusWindow {
        application_id: String,
        window_id: String,
    },
}

impl ComputerAction {
    /// Whether the action captures the screen or a window.
    pub fn is_capture(&self) -> bool {
        matches!(
            self,
            Self::CaptureScreen | Self::CaptureWindow { .. }
        )
    }

    /// The application the action targets, when it targets one.
    pub fn application_id(&self) -> Option<&str> {
        match self {
            Self::Click { application_id, .. }
            | Self::TypeText { application_id, .. }
            | Self::KeyPress { application_id, .. }
            | Self::Scroll { application_id, .. }
            | Self::Drag { application_id, .. }
            | Self::CaptureWindow { application_id, .. }
            | Self::FocusWindow { application_id, .. } => Some(application_id),
            Self::CaptureScreen => None,
        }
    }

    /// The window the action targets, when it targets one.
    pub fn window_id(&self) -> Option<&str> {
        match self {
            Self::Click { window_id, .. }
            | Self::TypeText { window_id, .. }
            | Self::KeyPress { window_id, .. }
            | Self::Scroll { window_id, .. }
            | Self::Drag { window_id, .. }
            | Self::CaptureWindow { window_id, .. }
            | Self::FocusWindow { window_id, .. } => Some(window_id),
            Self::CaptureScreen => None,
        }
    }
}

/// Who currently owns desktop input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ComputerControlOwner {
    Agent,
    Human,
}

/// The observed desktop state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComputerUseState {
    /// Whether the operating system session is locked or the screen is secure.
    pub screen_locked: bool,
    pub control_owner: ComputerControlOwner,
    /// The control epoch of the desktop lease.
    pub control_epoch: u64,
    /// The capabilities observed by the provider probe.
    pub capabilities: ComputerProviderCapabilities,
}

/// Why a computer action or observation was refused.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ComputerUseRefusal {
    ProviderUnavailable,
    ScreenLocked,
    HumanOwnsInput,
    ConsentMissing,
    ConsentRevoked,
    ConsentExpired,
    ApplicationNotInScope,
    WindowNotInScope,
    CaptureNotPermitted,
    InputNotPermitted,
    SensitiveRegion,
    UnsupportedAction,
    /// A request that asked to work around site or system security was refused.
    SecurityBypassRefused,
    /// Opening Quick Entry must never capture the screen.
    QuickEntryCaptureRefused,
}

impl ComputerUseRefusal {
    pub fn reason_code(&self) -> &'static str {
        match self {
            Self::ProviderUnavailable => "COMPUTER_PROVIDER_UNAVAILABLE",
            Self::ScreenLocked => "COMPUTER_SCREEN_LOCKED",
            Self::HumanOwnsInput => "COMPUTER_HUMAN_OWNS_INPUT",
            Self::ConsentMissing => "COMPUTER_CONSENT_MISSING",
            Self::ConsentRevoked => "COMPUTER_CONSENT_REVOKED",
            Self::ConsentExpired => "COMPUTER_CONSENT_EXPIRED",
            Self::ApplicationNotInScope => "COMPUTER_APPLICATION_NOT_IN_SCOPE",
            Self::WindowNotInScope => "COMPUTER_WINDOW_NOT_IN_SCOPE",
            Self::CaptureNotPermitted => "COMPUTER_CAPTURE_NOT_PERMITTED",
            Self::InputNotPermitted => "COMPUTER_INPUT_NOT_PERMITTED",
            Self::SensitiveRegion => "COMPUTER_SENSITIVE_REGION",
            Self::UnsupportedAction => "COMPUTER_ACTION_UNSUPPORTED",
            Self::SecurityBypassRefused => "COMPUTER_SECURITY_BYPASS_REFUSED",
            Self::QuickEntryCaptureRefused => "COMPUTER_QUICK_ENTRY_CAPTURE_REFUSED",
        }
    }
}

/// A computer action that passed every policy check.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorizedComputerAction {
    pub consent_id: String,
    pub scope_id: String,
    pub action: ComputerAction,
    pub control_epoch: u64,
}

/// A request to perform a computer action.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComputerActionRequest {
    pub trigger: ComputerUseTrigger,
    pub action: ComputerAction,
    /// Whether the request asked to work around site or system security.
    pub bypass_security: bool,
    /// The label of the region the action touches, when it is known.
    pub target_label: Option<String>,
}

/// Decide whether a computer action may be performed.
///
/// The checks run in a fixed order so that the refusal reason is stable: provider
/// availability, screen lock, human ownership, security bypass, trigger-specific
/// capture refusal, consent, and finally scope.
pub fn authorize_computer_action(
    request: &ComputerActionRequest,
    consent: Option<&ComputerObservationConsent>,
    state: &ComputerUseState,
    now_ms: i64,
) -> Result<AuthorizedComputerAction, ComputerUseRefusal> {
    if state.capabilities.availability != ComputerProviderAvailability::Available {
        return Err(ComputerUseRefusal::ProviderUnavailable);
    }
    if state.screen_locked {
        return Err(ComputerUseRefusal::ScreenLocked);
    }
    if state.control_owner == ComputerControlOwner::Human {
        return Err(ComputerUseRefusal::HumanOwnsInput);
    }
    if request.bypass_security {
        return Err(ComputerUseRefusal::SecurityBypassRefused);
    }
    if request.trigger == ComputerUseTrigger::QuickEntryOpened && request.action.is_capture() {
        return Err(ComputerUseRefusal::QuickEntryCaptureRefused);
    }
    if let Some(label) = &request.target_label {
        if let Some(kind) = crate::browser_consent::classify_sensitive_region(label) {
            if request.action.is_capture() || kind == SensitiveRegionKind::PasswordField {
                return Err(ComputerUseRefusal::SensitiveRegion);
            }
        }
    }
    let consent = consent.ok_or(ComputerUseRefusal::ConsentMissing)?;
    if consent.revoked_at_ms.is_some() {
        return Err(ComputerUseRefusal::ConsentRevoked);
    }
    if now_ms >= consent.expires_at_ms {
        return Err(ComputerUseRefusal::ConsentExpired);
    }

    let scope = consent
        .scopes
        .iter()
        .find(|scope| match (scope.application_id.as_deref(), request.action.application_id()) {
            (None, _) => true,
            (Some(scoped), Some(target)) => scoped == target,
            (Some(_), None) => false,
        })
        .ok_or(ComputerUseRefusal::ApplicationNotInScope)?;
    if let Some(window_id) = &scope.window_id {
        match request.action.window_id() {
            Some(target) if target == window_id => {}
            _ => return Err(ComputerUseRefusal::WindowNotInScope),
        }
    }
    if request.action.is_capture() {
        if !scope.allow_screen_capture {
            return Err(ComputerUseRefusal::CaptureNotPermitted);
        }
    } else if !scope.allow_input_injection {
        return Err(ComputerUseRefusal::InputNotPermitted);
    }
    if scope.excluded_regions.iter().any(|region| {
        request
            .target_label
            .as_deref()
            .and_then(crate::browser_consent::classify_sensitive_region)
            == Some(region.kind)
    }) {
        return Err(ComputerUseRefusal::SensitiveRegion);
    }

    Ok(AuthorizedComputerAction {
        consent_id: consent.consent_id.clone(),
        scope_id: scope.scope_id.clone(),
        action: request.action.clone(),
        control_epoch: state.control_epoch,
    })
}

/// A consent that authorizes observation of exactly one scope.
pub fn single_scope_consent(
    workspace_id: &str,
    scope: ComputerObservationScope,
    now_ms: i64,
) -> ComputerObservationConsent {
    ComputerObservationConsent {
        consent_id: "consent-1".to_owned(),
        workspace_id: workspace_id.to_owned(),
        scopes: vec![scope],
        granted_at_ms: now_ms,
        expires_at_ms: now_ms + 60_000,
        revoked_at_ms: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scope() -> ComputerObservationScope {
        ComputerObservationScope {
            scope_id: "scope-1".to_owned(),
            application_id: Some("com.example.editor".to_owned()),
            window_id: Some("window-1".to_owned()),
            allow_screen_capture: true,
            allow_input_injection: true,
            excluded_regions: vec![],
        }
    }

    fn state() -> ComputerUseState {
        let mut capabilities = probe_computer_provider();
        capabilities.availability = ComputerProviderAvailability::Available;
        capabilities.screen_capture_available = true;
        capabilities.input_injection_available = true;
        ComputerUseState {
            screen_locked: false,
            control_owner: ComputerControlOwner::Agent,
            control_epoch: 3,
            capabilities,
        }
    }

    fn action() -> ComputerAction {
        ComputerAction::Click {
            application_id: "com.example.editor".to_owned(),
            window_id: "window-1".to_owned(),
            x: 10,
            y: 20,
        }
    }

    fn request() -> ComputerActionRequest {
        ComputerActionRequest {
            trigger: ComputerUseTrigger::TaskRequest,
            action: action(),
            bypass_security: false,
            target_label: None,
        }
    }

    #[test]
    fn the_local_provider_probe_reports_a_truthful_availability() {
        let capabilities = probe_computer_provider();
        assert_ne!(
            capabilities.availability,
            ComputerProviderAvailability::Available
        );
        assert!(!capabilities.screen_capture_available);
        assert!(!capabilities.input_injection_available);
        assert!(!capabilities.reasons.is_empty());
    }

    #[test]
    fn no_computer_action_is_allowed_without_a_qualified_provider() {
        let mut state = state();
        state.capabilities.availability = ComputerProviderAvailability::UnsupportedPlatform;
        let consent = single_scope_consent("workspace-1", scope(), 1_000);
        assert_eq!(
            authorize_computer_action(&request(), Some(&consent), &state, 1_100).unwrap_err(),
            ComputerUseRefusal::ProviderUnavailable
        );
    }

    #[test]
    fn opening_quick_entry_never_captures_the_screen() {
        let consent = single_scope_consent("workspace-1", scope(), 1_000);
        let mut request = request();
        request.trigger = ComputerUseTrigger::QuickEntryOpened;
        request.action = ComputerAction::CaptureScreen;
        assert_eq!(
            authorize_computer_action(&request, Some(&consent), &state(), 1_100).unwrap_err(),
            ComputerUseRefusal::QuickEntryCaptureRefused
        );
    }

    #[test]
    fn a_locked_screen_refuses_every_action() {
        let consent = single_scope_consent("workspace-1", scope(), 1_000);
        let mut state = state();
        state.screen_locked = true;
        assert_eq!(
            authorize_computer_action(&request(), Some(&consent), &state, 1_100).unwrap_err(),
            ComputerUseRefusal::ScreenLocked
        );
    }

    #[test]
    fn an_action_after_human_takeover_is_refused() {
        let consent = single_scope_consent("workspace-1", scope(), 1_000);
        let mut state = state();
        state.control_owner = ComputerControlOwner::Human;
        assert_eq!(
            authorize_computer_action(&request(), Some(&consent), &state, 1_100).unwrap_err(),
            ComputerUseRefusal::HumanOwnsInput
        );
    }

    #[test]
    fn a_security_bypass_request_is_always_refused() {
        let consent = single_scope_consent("workspace-1", scope(), 1_000);
        let mut request = request();
        request.bypass_security = true;
        assert_eq!(
            authorize_computer_action(&request, Some(&consent), &state(), 1_100).unwrap_err(),
            ComputerUseRefusal::SecurityBypassRefused
        );
    }

    #[test]
    fn observation_requires_an_explicit_unrevoked_unexpired_consent() {
        let mut state = state();
        let consent = single_scope_consent("workspace-1", scope(), 1_000);

        assert_eq!(
            authorize_computer_action(&request(), None, &state, 1_100).unwrap_err(),
            ComputerUseRefusal::ConsentMissing
        );

        let mut revoked = consent.clone();
        revoked.revoked_at_ms = Some(1_050);
        assert_eq!(
            authorize_computer_action(&request(), Some(&revoked), &state, 1_100).unwrap_err(),
            ComputerUseRefusal::ConsentRevoked
        );

        assert_eq!(
            authorize_computer_action(&request(), Some(&consent), &state, 1_000 + 60_000)
                .unwrap_err(),
            ComputerUseRefusal::ConsentExpired
        );

        state.control_epoch = 4;
        let authorized =
            authorize_computer_action(&request(), Some(&consent), &state, 1_100).expect("authorized");
        assert_eq!(authorized.consent_id, "consent-1");
        assert_eq!(authorized.scope_id, "scope-1");
        assert_eq!(authorized.control_epoch, 4);
    }

    #[test]
    fn a_wrong_window_is_out_of_scope() {
        let consent = single_scope_consent("workspace-1", scope(), 1_000);
        let mut request = request();
        request.action = ComputerAction::Click {
            application_id: "com.example.editor".to_owned(),
            window_id: "window-other".to_owned(),
            x: 1,
            y: 1,
        };
        assert_eq!(
            authorize_computer_action(&request, Some(&consent), &state(), 1_100).unwrap_err(),
            ComputerUseRefusal::WindowNotInScope
        );
    }

    #[test]
    fn a_capture_outside_the_permitted_capture_scope_is_refused() {
        let mut capture_scope = scope();
        capture_scope.allow_screen_capture = false;
        let consent = single_scope_consent("workspace-1", capture_scope, 1_000);
        let mut request = request();
        request.action = ComputerAction::CaptureWindow {
            application_id: "com.example.editor".to_owned(),
            window_id: "window-1".to_owned(),
        };
        assert_eq!(
            authorize_computer_action(&request, Some(&consent), &state(), 1_100).unwrap_err(),
            ComputerUseRefusal::CaptureNotPermitted
        );
    }

    #[test]
    fn a_sensitive_region_is_never_captured() {
        let consent = single_scope_consent("workspace-1", scope(), 1_000);
        let mut request = request();
        request.action = ComputerAction::CaptureWindow {
            application_id: "com.example.editor".to_owned(),
            window_id: "window-1".to_owned(),
        };
        request.target_label = Some("Password".to_owned());
        assert_eq!(
            authorize_computer_action(&request, Some(&consent), &state(), 1_100).unwrap_err(),
            ComputerUseRefusal::SensitiveRegion
        );
    }
}
