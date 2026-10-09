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

use super::{
    OpenCodeEnvironment, OpenCodeError, OpenCodeProcessState, OpenCodeServer, OpenCodeServerConfig,
};

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
    UnsupportedVersion,
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
    pub protocol_version: Option<String>,
    pub probe_readiness: OpenCodeProbeReadiness,
    pub failure: Option<OpenCodeProbeFailure>,
    /// This means the V2 provider catalog route returned a valid data array. It
    /// is not a connected-provider, authentication, entitlement, or inference
    /// success claim.
    pub reported_provider_status: OpenCodeObservation,
    pub reported_connected_provider_ids: Vec<String>,
    pub reported_connected_provider_ids_truncated: bool,
    /// Provider identifiers reported by the V2 catalog. This is not a
    /// connectivity, authentication, or entitlement observation.
    pub reported_provider_ids: Vec<String>,
    pub reported_provider_ids_truncated: bool,
    /// Display-only metadata from `/api/model`; it is not a universal model
    /// registry and does not imply qualified session model selection.
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
            protocol_version: None,
            probe_readiness: OpenCodeProbeReadiness::Failed,
            failure: None,
            reported_provider_status: OpenCodeObservation::NotProbed,
            reported_connected_provider_ids: Vec::new(),
            reported_connected_provider_ids_truncated: false,
            reported_provider_ids: Vec::new(),
            reported_provider_ids_truncated: false,
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

    let Some(version) = read_opencode_version(executable) else {
        result.failure = Some(OpenCodeProbeFailure::UnsupportedVersion);
        return result;
    };
    result.protocol_version = Some(version.clone());
    if !is_supported_opencode_version(&version) {
        result.failure = Some(OpenCodeProbeFailure::UnsupportedVersion);
        return result;
    }

    let mut config = OpenCodeServerConfig::bounded(executable, cwd).with_environment(environment);
    config.protocol = super::OpenCodeServerProtocol::V2_0_26;
    config.startup_timeout = Duration::from_secs(8);
    config.request_timeout = Duration::from_secs(5);
    let mut server = match OpenCodeServer::spawn(config) {
        Ok(server) => server,
        Err(_) => {
            result.failure = Some(OpenCodeProbeFailure::HostUnavailable);
            return result;
        }
    };

    observe_v2_provider_catalog(&mut result, server.read_provider_status());
    observe_v2_model_catalog(&mut result, server.read_configured_provider_catalog());

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

fn read_opencode_version(executable: &Path) -> Option<String> {
    use std::{
        io::Read,
        process::{Command, Stdio},
        sync::mpsc,
        thread,
        time::Instant,
    };
    let mut child = Command::new(executable)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .env_clear()
        .spawn()
        .ok()?;
    let mut stdout = child.stdout.take()?;
    let (tx, rx) = mpsc::sync_channel(1);
    let _reader = thread::spawn(move || {
        let mut bytes = Vec::with_capacity(32);
        let _ = stdout.by_ref().take(129).read_to_end(&mut bytes);
        let _ = tx.send(bytes);
    });
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => break,
            Ok(Some(_)) | Err(_) => return None,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(10)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
    let bytes = rx.recv_timeout(Duration::from_millis(100)).ok()?;
    if bytes.len() > 128 {
        return None;
    }
    let text = std::str::from_utf8(&bytes).ok()?;
    parse_opencode_version_output(text)
}

fn parse_opencode_version_output(output: &str) -> Option<String> {
    let trimmed = output.trim();
    let value = trimmed.strip_prefix("opencode ").unwrap_or(trimmed);
    let value = value.strip_prefix('v').unwrap_or(value);
    let mut segments = value.split('.');
    let valid = (0..3).all(|_| {
        segments.next().is_some_and(|segment| {
            !segment.is_empty() && segment.bytes().all(|byte| byte.is_ascii_digit())
        })
    }) && segments.next().is_none();
    valid.then(|| value.to_owned())
}

fn is_supported_opencode_version(version: &str) -> bool {
    // This is the exact version whose V2 route behavior was observed and
    // qualified. Do not assume later releases preserve experimental routes.
    version == "2.0.26"
}

fn observe_v2_provider_catalog(
    result: &mut OpenCodeProfileProbe,
    response: Result<Value, OpenCodeError>,
) {
    let projected = response.ok().and_then(|value| {
        let entries = value.get("data")?.as_array()?;
        let mut truncated = entries.len() > MAX_PROVIDERS;
        let mut ids = entries
            .iter()
            .take(MAX_PROVIDERS)
            .filter_map(|entry| {
                let Some(id) = entry
                    .get("id")
                    .and_then(Value::as_str)
                    .and_then(safe_identifier)
                else {
                    truncated = true;
                    return None;
                };
                Some(id)
            })
            .collect::<Vec<_>>();
        ids.sort();
        ids.dedup();
        Some((ids, truncated))
    });
    match projected {
        Some((ids, truncated)) => {
            result.reported_provider_status = OpenCodeObservation::Observed;
            result.reported_provider_ids = ids;
            result.reported_provider_ids_truncated = truncated;
        }
        None => {
            result.reported_provider_status = OpenCodeObservation::Unknown;
            result
                .failure
                .get_or_insert(OpenCodeProbeFailure::ProviderStatusUnavailable);
        }
    }
}

fn observe_v2_model_catalog(
    result: &mut OpenCodeProfileProbe,
    response: Result<Value, OpenCodeError>,
) {
    let projected = response.ok().and_then(|value| {
        let entries = value.get("data")?.as_array()?;
        let mut truncated = entries.len() > MAX_MODELS_TOTAL;
        let mut grouped = std::collections::BTreeMap::<String, Vec<OpenCodeModelOption>>::new();
        for entry in entries.iter().take(MAX_MODELS_TOTAL) {
            let (Some(provider), Some(id)) = (
                entry
                    .get("providerID")
                    .and_then(Value::as_str)
                    .and_then(safe_identifier),
                entry
                    .get("modelID")
                    .and_then(Value::as_str)
                    .and_then(safe_identifier),
            ) else {
                truncated = true;
                continue;
            };
            if !result
                .reported_provider_ids
                .iter()
                .any(|observed| observed == &provider)
            {
                truncated = true;
                continue;
            }
            let display_name = entry.get("name").and_then(safe_display_name);
            if entry.get("name").is_some() && display_name.is_none() {
                truncated = true;
            }
            let models = grouped.entry(provider).or_default();
            if models.len() < MAX_MODELS_PER_PROVIDER {
                models.push(OpenCodeModelOption { id, display_name });
            } else {
                truncated = true;
            }
        }
        let providers = grouped
            .into_iter()
            .map(|(id, mut models)| {
                models.sort_by(|a, b| a.id.cmp(&b.id));
                OpenCodeProviderOption {
                    id,
                    display_name: None,
                    models,
                }
            })
            .collect::<Vec<_>>();
        Some((providers, truncated))
    });
    match projected {
        Some((providers, truncated)) => {
            result.reported_model_catalog = OpenCodeObservation::Observed;
            result.configured_providers = providers;
            result.catalog_truncated = truncated;
        }
        None => {
            result.reported_model_catalog = OpenCodeObservation::Unknown;
            result
                .failure
                .get_or_insert(OpenCodeProbeFailure::CatalogUnavailable);
        }
    }
}

fn safe_identifier(value: &str) -> Option<String> {
    if value.is_empty()
        || value.len() > MAX_ID_BYTES
        || looks_sensitive(value)
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
        || looks_sensitive(value)
        || value.contains("://")
        || value.to_ascii_lowercase().starts_with("www.")
    {
        return None;
    }
    Some(value.to_owned())
}

fn looks_sensitive(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.contains('@')
        || lower.contains("bearer ")
        || lower.contains("authorization:")
        || lower.contains("api_key")
        || lower.contains("api-key")
        || lower.contains("api key")
        || lower.contains("token=")
        || lower.contains("password=")
        || lower.contains("secret=")
        || lower.contains("sk-")
        || lower.contains("ghp_")
        || lower.contains("gho_")
        || lower.contains("ghu_")
        || lower.contains("ghs_")
        || lower.contains("ghr_")
        || lower.contains("xoxb-")
        || lower.contains("xoxp-")
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v2_provider_and_model_envelopes_project_only_bounded_display_fields() {
        let providers = serde_json::json!({
            "location": {"directory":"/private/project"},
            "data": [{"id":"openai","name":"OpenAI","disabled":false,
                "settings":{"apiKey":"private"},"headers":{"Authorization":"private"},
                "package":"provider-package"}]
        });
        let models = serde_json::json!({
            "location": {"directory":"/private/project"},
            "data": [{"id":"openai/gpt-safe","providerID":"openai","modelID":"gpt-safe",
                "name":"GPT Safe","headers":{"api-key":"private"},"settings":{"token":"private"},
                "enabled":true}]
        });
        let mut result = OpenCodeProfileProbe::initial();
        observe_v2_provider_catalog(&mut result, Ok(providers));
        observe_v2_model_catalog(&mut result, Ok(models));

        assert_eq!(
            result.reported_provider_status,
            OpenCodeObservation::Observed
        );
        assert_eq!(result.reported_model_catalog, OpenCodeObservation::Observed);
        assert_eq!(result.reported_connected_provider_ids, Vec::<String>::new());
        assert_eq!(result.configured_providers[0].id, "openai");
        assert_eq!(result.configured_providers[0].models[0].id, "gpt-safe");
        let serialized = serde_json::to_string(&result).unwrap();
        for secret in [
            "/private/project",
            "private",
            "Authorization",
            "provider-package",
        ] {
            assert!(!serialized.contains(secret));
        }
    }

    #[test]
    fn v2_malformed_envelopes_and_html_fallback_fail_closed() {
        let mut result = OpenCodeProfileProbe::initial();
        observe_v2_provider_catalog(&mut result, Ok(serde_json::json!({"providers":[]})));
        observe_v2_model_catalog(&mut result, Ok(serde_json::json!({"data":"not-an-array"})));
        assert_eq!(
            result.reported_provider_status,
            OpenCodeObservation::Unknown
        );
        assert_eq!(result.reported_model_catalog, OpenCodeObservation::Unknown);
        assert!(result.configured_providers.is_empty());
        observe_v2_model_catalog(&mut result, Err(OpenCodeError::InvalidResponse));
        assert_eq!(result.reported_model_catalog, OpenCodeObservation::Unknown);
    }

    #[test]
    fn exact_version_qualification_rejects_mismatch_without_inference() {
        assert!(is_supported_opencode_version("2.0.26"));
        for unsupported in ["2.0.25", "2.0.27", "1.1.0", "unknown"] {
            assert!(!is_supported_opencode_version(unsupported));
        }
        let result = OpenCodeProfileProbe::initial();
        assert_eq!(result.authentication_observation, "UNKNOWN");
        assert!(!result.inference_access_verified);
        assert!(!result.profile_admission_supported);
        assert_eq!(result.session_model_selection, "NOT_QUALIFIED");
    }

    #[test]
    fn version_output_parser_accepts_only_bounded_semver_line() {
        assert_eq!(
            parse_opencode_version_output("opencode v2.0.26\n"),
            Some("2.0.26".to_owned())
        );
        assert_eq!(
            parse_opencode_version_output("2.0.27"),
            Some("2.0.27".to_owned())
        );
        for malformed in ["2.0", "v2.0.26-beta", "2.0.26\nsecret output", "unknown"] {
            assert_eq!(
                parse_opencode_version_output(malformed),
                None,
                "{malformed}"
            );
        }
    }

    #[test]
    fn v2_provider_catalog_never_promotes_provider_ids_to_connected_ids() {
        let mut result = OpenCodeProfileProbe::initial();
        observe_v2_provider_catalog(
            &mut result,
            Ok(serde_json::json!({
                "data": [{"id":"openai","name":"OpenAI","disabled":false}]
            })),
        );
        assert_eq!(result.reported_provider_ids, ["openai"]);
        assert!(result.reported_connected_provider_ids.is_empty());
        assert_eq!(result.authentication_observation, "UNKNOWN");
        assert!(!result.inference_access_verified);
    }

    #[test]
    fn provider_and_model_catalog_limits_are_reported_as_truncated() {
        let provider_rows: Vec<_> = (0..MAX_PROVIDERS + 1)
            .map(|index| serde_json::json!({"id": format!("provider-{index:02}")}))
            .collect();
        let mut provider_result = OpenCodeProfileProbe::initial();
        observe_v2_provider_catalog(
            &mut provider_result,
            Ok(serde_json::json!({"data": provider_rows})),
        );
        assert_eq!(provider_result.reported_provider_ids.len(), MAX_PROVIDERS);
        assert!(provider_result.reported_provider_ids_truncated);

        let one_provider_models: Vec<_> = (0..MAX_MODELS_PER_PROVIDER + 1)
            .map(|index| {
                serde_json::json!({
                    "providerID":"provider-0", "modelID":format!("model-{index:02}"),
                    "name":format!("Model {index}")
                })
            })
            .collect();
        let mut one_provider_result = OpenCodeProfileProbe::initial();
        one_provider_result.reported_provider_ids = vec!["provider-0".to_owned()];
        observe_v2_model_catalog(
            &mut one_provider_result,
            Ok(serde_json::json!({"data": one_provider_models})),
        );
        assert_eq!(
            one_provider_result.configured_providers[0].models.len(),
            MAX_MODELS_PER_PROVIDER
        );
        assert!(one_provider_result.catalog_truncated);

        let many_providers: Vec<_> = (0..5)
            .map(|provider_index| format!("provider-{provider_index}"))
            .collect();
        let many_models: Vec<_> = many_providers
            .iter()
            .flat_map(|provider| {
                (0..MAX_MODELS_PER_PROVIDER).map(move |index| {
                    serde_json::json!({
                        "providerID":provider, "modelID":format!("model-{index:02}"),
                        "name":format!("Model {index}")
                    })
                })
            })
            .chain(std::iter::once(serde_json::json!({
                "providerID":"provider-4", "modelID":"model-over-total", "name":"Overflow"
            })))
            .collect();
        let mut total_result = OpenCodeProfileProbe::initial();
        total_result.reported_provider_ids = many_providers;
        observe_v2_model_catalog(
            &mut total_result,
            Ok(serde_json::json!({"data":many_models})),
        );
        assert_eq!(
            total_result
                .configured_providers
                .iter()
                .map(|provider| provider.models.len())
                .sum::<usize>(),
            MAX_MODELS_TOTAL
        );
        assert!(total_result.catalog_truncated);
    }

    #[test]
    fn identifiers_with_account_or_credential_markers_are_rejected() {
        for identifier in ["user@example.test", "sk-proj-secret", "xoxb-private"] {
            assert_eq!(safe_identifier(identifier), None, "{identifier}");
        }
    }

    #[test]
    fn provider_and_model_display_names_reject_obvious_secret_material() {
        for name in [
            "Bearer sk-proj-0123456789abcdef0123456789abcdef",
            "api_key=0123456789abcdef",
            "user@example.test",
            "https://provider.example",
        ] {
            assert_eq!(
                safe_display_name(&Value::String(name.to_owned())),
                None,
                "{name}"
            );
        }
        assert_eq!(
            safe_display_name(&Value::String("OpenAI-compatible Local".to_owned())),
            Some("OpenAI-compatible Local".to_owned())
        );
    }
}
