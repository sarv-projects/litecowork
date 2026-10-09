//! Bounded, read-only OpenCode Server profile observation.
//!
//! This operation starts only the owned local `opencode serve` process, reads the
//! documented provider status/catalog routes, projects a strict non-secret DTO,
//! then stops the direct child. It never starts a session or performs inference.
//! OpenCode descendants are not contained by this probe, so the resulting offer is
//! deliberately incompatible with Task/worker admission until that lifecycle is
//! qualified separately.

use std::{path::Path, time::Duration};

use serde::Serialize;
use serde_json::Value;

use super::{OpenCodeEnvironment, OpenCodeProcessState, OpenCodeServer, OpenCodeServerConfig};

const MAX_PROVIDERS: usize = 32;
const MAX_MODELS_TOTAL: usize = 256;
const MAX_MODELS_PER_PROVIDER: usize = 64;
const MAX_ID_BYTES: usize = 128;
const MAX_DISPLAY_NAME_BYTES: usize = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum OpenCodeProbeReadiness {
    Complete,
    Partial,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum OpenCodeObservation {
    Observed,
    Unknown,
    NotProbed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum OpenCodeProbeFailure {
    InvalidAdmission,
    HostUnavailable,
    ProviderStatusUnavailable,
    CatalogUnavailable,
    HostStopUnobserved,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct OpenCodeModelOption {
    pub id: String,
    pub display_name: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct OpenCodeProviderOption {
    pub id: String,
    pub display_name: Option<String>,
    pub models: Vec<OpenCodeModelOption>,
}

#[derive(Debug, Serialize)]
pub(crate) struct OpenCodeProfileProbe {
    pub agent_id: &'static str,
    pub protocol: &'static str,
    pub probe_readiness: OpenCodeProbeReadiness,
    pub failure: Option<OpenCodeProbeFailure>,
    /// This means `/provider` returned a valid `connected` list. It is not an
    /// authentication, entitlement, or inference-success claim.
    pub reported_provider_status: OpenCodeObservation,
    pub reported_connected_provider_ids: Vec<String>,
    pub reported_connected_provider_ids_truncated: bool,
    /// Display-only metadata from `/config/providers`; it is not a universal
    /// model registry and does not imply that OpenCode can select an override.
    pub reported_model_catalog: OpenCodeObservation,
    pub configured_providers: Vec<OpenCodeProviderOption>,
    pub catalog_truncated: bool,
    pub authentication_observation: &'static str,
    pub session_model_selection: &'static str,
    pub inference_access_verified: bool,
    pub host_process_stopped: bool,
    pub writer_quiescence_proven: bool,
    /// Always false until session lifecycle, Environment isolation, and native
    /// capability mediation are implemented and qualified.
    pub profile_admission_supported: bool,
}

impl OpenCodeProfileProbe {
    fn initial() -> Self {
        Self {
            agent_id: "opencode",
            protocol: "OPENCODE_SERVER",
            probe_readiness: OpenCodeProbeReadiness::Failed,
            failure: None,
            reported_provider_status: OpenCodeObservation::NotProbed,
            reported_connected_provider_ids: Vec::new(),
            reported_connected_provider_ids_truncated: false,
            reported_model_catalog: OpenCodeObservation::NotProbed,
            configured_providers: Vec::new(),
            catalog_truncated: false,
            authentication_observation: "UNKNOWN",
            session_model_selection: "NOT_QUALIFIED",
            inference_access_verified: false,
            host_process_stopped: false,
            writer_quiescence_proven: false,
            profile_admission_supported: false,
        }
    }
}

/// Runs one bounded read-only observation. The caller must already have
/// authenticated the Workspace owner, verified current Runtime enrollment, and
/// resolved an admitted Runtime-local executable. It returns no raw native JSON.
pub(crate) fn probe_opencode_profile(
    executable: &Path,
    cwd: &Path,
    environment: OpenCodeEnvironment,
) -> OpenCodeProfileProbe {
    let mut result = OpenCodeProfileProbe::initial();
    if !executable.is_absolute() || !cwd.is_absolute() {
        result.failure = Some(OpenCodeProbeFailure::InvalidAdmission);
        return result;
    }

    let mut config = OpenCodeServerConfig::bounded(executable, cwd).with_environment(environment);
    config.startup_timeout = Duration::from_secs(8);
    config.request_timeout = Duration::from_secs(5);
    let mut server = match OpenCodeServer::spawn(config) {
        Ok(server) => server,
        Err(_) => {
            result.failure = Some(OpenCodeProbeFailure::HostUnavailable);
            return result;
        }
    };

    match server.read_provider_status() {
        Ok(native) => match connected_provider_ids(&native) {
            Some((ids, truncated)) => {
                result.reported_provider_status = OpenCodeObservation::Observed;
                result.reported_connected_provider_ids = ids;
                result.reported_connected_provider_ids_truncated = truncated;
            }
            None => result.failure = Some(OpenCodeProbeFailure::ProviderStatusUnavailable),
        },
        Err(_) => result.failure = Some(OpenCodeProbeFailure::ProviderStatusUnavailable),
    }

    match server.read_configured_provider_catalog() {
        Ok(native) => match configured_catalog(&native) {
            Some((providers, truncated)) => {
                result.reported_model_catalog = OpenCodeObservation::Observed;
                result.configured_providers = providers;
                result.catalog_truncated = truncated;
            }
            None => {
                if result.failure.is_none() {
                    result.failure = Some(OpenCodeProbeFailure::CatalogUnavailable);
                }
            }
        },
        Err(_) => {
            if result.failure.is_none() {
                result.failure = Some(OpenCodeProbeFailure::CatalogUnavailable);
            }
        }
    }

    result.host_process_stopped = server.stop() == OpenCodeProcessState::Exited;
    if !result.host_process_stopped {
        result.failure = Some(OpenCodeProbeFailure::HostStopUnobserved);
    }

    let complete = result.reported_provider_status == OpenCodeObservation::Observed
        && result.reported_model_catalog == OpenCodeObservation::Observed
        && result.host_process_stopped;
    result.probe_readiness = if complete {
        OpenCodeProbeReadiness::Complete
    } else if result.reported_provider_status != OpenCodeObservation::NotProbed
        || result.reported_model_catalog != OpenCodeObservation::NotProbed
    {
        OpenCodeProbeReadiness::Partial
    } else {
        OpenCodeProbeReadiness::Failed
    };
    result
}

fn connected_provider_ids(value: &Value) -> Option<(Vec<String>, bool)> {
    let connected = value.get("connected")?.as_array()?;
    let mut truncated = connected.len() > MAX_PROVIDERS;
    let mut result = Vec::with_capacity(connected.len().min(MAX_PROVIDERS));
    for entry in connected.iter().take(MAX_PROVIDERS) {
        if let Some(id) = entry.as_str().and_then(safe_identifier) {
            result.push(id);
        } else {
            truncated = true;
        }
    }
    result.sort();
    result.dedup();
    Some((result, truncated))
}

fn configured_catalog(value: &Value) -> Option<(Vec<OpenCodeProviderOption>, bool)> {
    let providers = value.get("providers")?.as_array()?;
    let mut truncated = providers.len() > MAX_PROVIDERS;
    let mut projected = Vec::with_capacity(providers.len().min(MAX_PROVIDERS));
    let mut total_models = 0;

    for provider in providers.iter().take(MAX_PROVIDERS) {
        let Some(provider_id) = provider
            .get("id")
            .and_then(Value::as_str)
            .and_then(safe_identifier)
        else {
            truncated = true;
            continue;
        };
        let display_name = provider.get("name").and_then(safe_display_name);
        if provider.get("name").is_some() && display_name.is_none() {
            truncated = true;
        }
        let Some(models) = provider.get("models").and_then(Value::as_object) else {
            truncated = true;
            continue;
        };
        if models.len() > MAX_MODELS_PER_PROVIDER {
            truncated = true;
        }

        let mut model_options = Vec::with_capacity(
            models
                .len()
                .min(MAX_MODELS_PER_PROVIDER)
                .min(MAX_MODELS_TOTAL.saturating_sub(total_models)),
        );
        for (model_key, model) in models.iter().take(MAX_MODELS_PER_PROVIDER) {
            if total_models >= MAX_MODELS_TOTAL {
                truncated = true;
                break;
            }
            let Some(id) = safe_identifier(model_key) else {
                truncated = true;
                continue;
            };
            let display_name = model.get("name").and_then(safe_display_name);
            if model.get("name").is_some() && display_name.is_none() {
                truncated = true;
            }
            total_models += 1;
            model_options.push(OpenCodeModelOption { id, display_name });
        }
        model_options.sort_by(|left, right| left.id.cmp(&right.id));
        projected.push(OpenCodeProviderOption {
            id: provider_id,
            display_name,
            models: model_options,
        });
    }
    projected.sort_by(|left, right| left.id.cmp(&right.id));
    Some((projected, truncated))
}

fn safe_identifier(value: &str) -> Option<String> {
    if value.is_empty()
        || value.len() > MAX_ID_BYTES
        || value.contains("://")
        || value.to_ascii_lowercase().starts_with("www.")
        || !value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':' | b'/')
        })
    {
        return None;
    }
    Some(value.to_owned())
}

fn safe_display_name(value: &Value) -> Option<String> {
    let value = value.as_str()?;
    if value.is_empty()
        || value.len() > MAX_DISPLAY_NAME_BYTES
        || value.chars().any(char::is_control)
        || value.contains("://")
        || value.to_ascii_lowercase().starts_with("www.")
    {
        return None;
    }
    Some(value.to_owned())
}

pub(crate) fn opencode_native_environment() -> OpenCodeEnvironment {
    OpenCodeEnvironment {
        home: std::env::var_os("HOME"),
        user_profile: std::env::var_os("USERPROFILE"),
        app_data: std::env::var_os("APPDATA"),
        local_app_data: std::env::var_os("LOCALAPPDATA"),
        xdg_config_home: std::env::var_os("XDG_CONFIG_HOME"),
        xdg_data_home: std::env::var_os("XDG_DATA_HOME"),
        xdg_cache_home: std::env::var_os("XDG_CACHE_HOME"),
        xdg_state_home: std::env::var_os("XDG_STATE_HOME"),
        path: std::env::var_os("PATH"),
        shell: std::env::var_os("SHELL"),
        comspec: std::env::var_os("COMSPEC"),
        pathext: std::env::var_os("PATHEXT"),
        windir: std::env::var_os("WINDIR"),
        system_root: std::env::var_os("SYSTEMROOT"),
        temporary_directory: Some(std::env::temp_dir().into_os_string()),
        lang: std::env::var_os("LANG"),
        lc_all: std::env::var_os("LC_ALL"),
        term: std::env::var_os("TERM"),
    }
}
