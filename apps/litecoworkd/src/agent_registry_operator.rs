//! Bounded, descriptive ACP Registry cache exposed through authenticated local Operator routes.
//! Registry documents never provide executable commands or create agent authority.

use crate::agents::acp_registry::{ParsedRegistry, RegistryAgent, parse_registry};
use axum::{
    Json, Router,
    extract::{FromRef, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::time::Duration;

const REGISTRY_URL: &str = "https://cdn.agentclientprotocol.com/registry/v1/latest/registry.json";
const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
const CACHE_TTL: Duration = Duration::from_secs(15 * 60);
const MAX_PAGE_SIZE: usize = 100;
const MAX_CACHED_DISTRIBUTIONS: usize = 32;

static ERROR_SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

#[derive(Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AgentRegistrySnapshot {
    source_registry: String,
    source_revision: Option<String>,
    observed_at: String,
    expires_at: Option<String>,
    stale: bool,
    items: Vec<AgentRegistryEntry>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AgentRegistryEntry {
    registry_agent_id: String,
    display_name: String,
    description: Option<String>,
    homepage: Option<String>,
    source_registry: String,
    source_revision: Option<String>,
    distributions: Vec<AgentDistribution>,
    adapter_support: String,
    metadata_observed_at: String,
    cache_expires_at: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct AgentDistribution {
    distribution_id: String,
    kind: String,
    package_ref: String,
    version_constraint: Option<String>,
    os: Vec<String>,
    architecture: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct AgentRegistryPage {
    items: Vec<AgentRegistryEntry>,
    next_cursor: Option<String>,
    observed_at: String,
    stale: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RegistryQuery {
    q: Option<String>,
    cursor: Option<String>,
    limit: Option<usize>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RegistryCursor {
    version: u8,
    snapshot_digest: String,
    source_revision: Option<String>,
    query: Option<String>,
    offset: usize,
}

#[derive(Clone)]
struct RegistryApiState {
    store: super::SqliteWorkspaceStore,
}

impl FromRef<super::ApiState> for RegistryApiState {
    fn from_ref(state: &super::ApiState) -> Self {
        Self {
            store: state.store.clone(),
        }
    }
}

pub(crate) fn routes() -> Router<super::ApiState> {
    Router::new()
        .route("/v1/agent-registry", get(list_registry))
        .route("/v1/agent-registry/refresh", post(refresh_registry))
}

async fn list_registry(
    State(state): State<RegistryApiState>,
    Query(query): Query<RegistryQuery>,
) -> Response {
    let query_text = match normalize_query(query.q) {
        Ok(query) => query,
        Err(response) => return response,
    };
    let limit = query.limit.unwrap_or(50);
    if limit == 0 || limit > MAX_PAGE_SIZE {
        return registry_error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "Registry page size must be between 1 and 100",
        );
    }
    let snapshot = match cached_or_refresh(state).await {
        Ok(snapshot) => snapshot,
        Err(response) => return response,
    };
    let offset = match decode_cursor(query.cursor.as_deref(), &snapshot, query_text.as_deref()) {
        Ok(offset) => offset,
        Err(response) => return response,
    };
    let filtered: Vec<_> = snapshot
        .items
        .iter()
        .filter(|entry| matches_query(entry, query_text.as_deref()))
        .cloned()
        .collect();
    if offset > filtered.len() {
        return registry_error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "Registry cursor is outside the current result set",
        );
    }
    let end = offset.saturating_add(limit).min(filtered.len());
    let next_cursor = if end < filtered.len() {
        let digest = match snapshot_digest(&snapshot) {
            Ok(digest) => digest,
            Err(response) => return response,
        };
        match encode_cursor(&RegistryCursor {
            version: 1,
            snapshot_digest: digest,
            source_revision: snapshot.source_revision.clone(),
            query: query_text.clone(),
            offset: end,
        }) {
            Ok(cursor) => Some(cursor),
            Err(response) => return response,
        }
    } else {
        None
    };
    Json(AgentRegistryPage {
        items: filtered[offset..end].to_vec(),
        next_cursor,
        observed_at: snapshot.observed_at,
        stale: snapshot.stale,
    })
    .into_response()
}

async fn refresh_registry(State(state): State<RegistryApiState>) -> Response {
    match refresh_or_stale(state).await {
        Ok(snapshot) => Json(snapshot).into_response(),
        Err(response) => response,
    }
}

async fn cached_or_refresh(state: RegistryApiState) -> Result<AgentRegistrySnapshot, Response> {
    if let Some(cached) = read_cached_snapshot(state.store.clone()).await?
        && !cached.stale
    {
        return Ok(cached);
    }
    refresh_or_stale(state).await
}

async fn refresh_or_stale(state: RegistryApiState) -> Result<AgentRegistrySnapshot, Response> {
    match fetch_snapshot().await {
        Ok(snapshot) => {
            persist_snapshot(state.store.clone(), &snapshot).await?;
            Ok(snapshot)
        }
        Err(()) => read_cached_snapshot(state.store.clone()).await?.map_or_else(
            || {
                Err(registry_error(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "DEPENDENCY_UNAVAILABLE",
                    "The ACP agent registry could not be reached and no cached snapshot is available",
                ))
            },
            |mut snapshot| {
                snapshot.stale = true;
                Ok(snapshot)
            },
        ),
    }
}

async fn read_cached_snapshot(
    store: super::SqliteWorkspaceStore,
) -> Result<Option<AgentRegistrySnapshot>, Response> {
    use storage_core::AgentRegistryCacheStore;

    let snapshot =
        tokio::task::spawn_blocking(move || store.read_agent_registry_snapshot(REGISTRY_URL))
            .await
            .map_err(|_| {
                registry_error(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "DEPENDENCY_UNAVAILABLE",
                    "The ACP agent registry cache is unavailable",
                )
            })?
            .map_err(|_| {
                registry_error(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "DEPENDENCY_UNAVAILABLE",
                    "The ACP agent registry cache is unavailable",
                )
            })?;
    snapshot.map(project_cached_snapshot).transpose()
}

async fn persist_snapshot(
    store: super::SqliteWorkspaceStore,
    snapshot: &AgentRegistrySnapshot,
) -> Result<(), Response> {
    use storage_core::{
        AgentRegistryCacheEntry, AgentRegistryCacheSnapshot, AgentRegistryCacheStore,
    };

    let cached = AgentRegistryCacheSnapshot {
        source_registry: snapshot.source_registry.clone(),
        source_revision: snapshot.source_revision.clone(),
        observed_at: snapshot.observed_at.clone(),
        expires_at: snapshot.expires_at.clone(),
        entries: snapshot
            .items
            .iter()
            .map(|entry| AgentRegistryCacheEntry {
                registry_agent_id: entry.registry_agent_id.clone(),
                metadata: serde_json::json!({
                    "display_name": entry.display_name,
                    "description": entry.description,
                    "homepage": entry.homepage,
                    "source_registry": entry.source_registry,
                    "source_revision": entry.source_revision,
                    "distributions": entry.distributions,
                    "adapter_support": entry.adapter_support,
                    "metadata_observed_at": entry.metadata_observed_at,
                    "cache_expires_at": entry.cache_expires_at,
                }),
            })
            .collect(),
    };
    tokio::task::spawn_blocking(move || store.replace_agent_registry_snapshot(cached))
        .await
        .map_err(|_| {
            registry_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "DEPENDENCY_UNAVAILABLE",
                "The ACP agent registry cache could not be saved",
            )
        })?
        .map_err(|_| {
            registry_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "DEPENDENCY_UNAVAILABLE",
                "The ACP agent registry cache could not be saved",
            )
        })
}

fn project_cached_snapshot(
    cached: storage_core::AgentRegistryCacheSnapshot,
) -> Result<AgentRegistrySnapshot, Response> {
    use serde::Deserialize;

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct CachedMetadata {
        display_name: String,
        description: Option<String>,
        homepage: Option<String>,
        source_registry: String,
        source_revision: Option<String>,
        distributions: Vec<AgentDistribution>,
        #[serde(rename = "adapter_support")]
        _adapter_support: String,
        metadata_observed_at: String,
        cache_expires_at: Option<String>,
    }

    if cached.source_registry != REGISTRY_URL {
        return Err(invalid_cached_snapshot());
    }
    let mut items = Vec::with_capacity(cached.entries.len());
    for entry in cached.entries {
        if entry.registry_agent_id.is_empty()
            || entry.registry_agent_id.len() > 128
            || !entry.registry_agent_id.is_ascii()
            || !entry
                .registry_agent_id
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        {
            return Err(invalid_cached_snapshot());
        }
        let metadata: CachedMetadata =
            serde_json::from_value(entry.metadata).map_err(|_| invalid_cached_snapshot())?;
        if metadata.source_registry != REGISTRY_URL
            || metadata.source_revision != cached.source_revision
            || metadata.display_name.trim().is_empty()
            || metadata.display_name.chars().count() > 128
            || metadata
                .description
                .as_ref()
                .is_some_and(|value| value.chars().count() > 1024)
            || metadata
                .homepage
                .as_ref()
                .is_some_and(|value| value.len() > 2048)
            || metadata.distributions.len() > MAX_CACHED_DISTRIBUTIONS
            || metadata.metadata_observed_at.len() > 64
            || metadata
                .cache_expires_at
                .as_ref()
                .is_some_and(|value| value.len() > 64)
            || metadata.distributions.iter().any(|distribution| {
                distribution.distribution_id.is_empty()
                    || distribution.distribution_id.len() > 128
                    || distribution.distribution_id.chars().any(char::is_control)
                    || !matches!(distribution.kind.as_str(), "NPM" | "PYPI" | "BINARY")
                    || distribution.package_ref.is_empty()
                    || distribution.package_ref.len() > 512
                    || distribution.package_ref.chars().any(char::is_control)
                    || distribution
                        .version_constraint
                        .as_ref()
                        .is_some_and(|value| {
                            value.len() > 128 || value.chars().any(char::is_control)
                        })
                    || distribution.os.len() > 3
                    || distribution
                        .os
                        .iter()
                        .any(|value| !matches!(value.as_str(), "WINDOWS" | "LINUX" | "MACOS"))
                    || distribution.architecture.len() > 2
                    || distribution
                        .architecture
                        .iter()
                        .any(|value| !matches!(value.as_str(), "X86_64" | "AARCH64"))
            })
        {
            return Err(invalid_cached_snapshot());
        }
        // The cache is informational. Never let stored metadata manufacture a
        // claim of adapter support or executable authority.
        items.push(AgentRegistryEntry {
            registry_agent_id: entry.registry_agent_id,
            display_name: metadata.display_name,
            description: metadata.description,
            homepage: metadata.homepage,
            source_registry: metadata.source_registry,
            source_revision: metadata.source_revision,
            distributions: metadata.distributions,
            adapter_support: "NO_ADAPTER".to_owned(),
            metadata_observed_at: metadata.metadata_observed_at,
            cache_expires_at: metadata.cache_expires_at,
        });
    }
    let stale = cache_expired(cached.expires_at.as_deref());
    Ok(AgentRegistrySnapshot {
        source_registry: cached.source_registry,
        source_revision: cached.source_revision,
        observed_at: cached.observed_at,
        expires_at: cached.expires_at,
        stale,
        items,
    })
}

fn invalid_cached_snapshot() -> Response {
    registry_error(
        StatusCode::SERVICE_UNAVAILABLE,
        "DEPENDENCY_UNAVAILABLE",
        "The cached ACP agent registry snapshot is invalid",
    )
}

fn cache_expired(expires_at: Option<&str>) -> bool {
    let Some(expires_at) = expires_at else {
        return true;
    };
    let Ok(expiry) =
        time::OffsetDateTime::parse(expires_at, &time::format_description::well_known::Rfc3339)
    else {
        return true;
    };
    time::OffsetDateTime::now_utc() >= expiry
}

async fn fetch_snapshot() -> Result<AgentRegistrySnapshot, ()> {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(3))
        .timeout(Duration::from_secs(8))
        .build()
        .map_err(|_| ())?;
    let response = client.get(REGISTRY_URL).send().await.map_err(|_| ())?;
    if !response.status().is_success()
        || response
            .content_length()
            .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
    {
        return Err(());
    }
    let mut stream = response.bytes_stream();
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| ())?;
        if bytes.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
            return Err(());
        }
        bytes.extend_from_slice(&chunk);
    }
    let parsed = parse_registry(&bytes).map_err(|_| ())?;
    Ok(project_snapshot(parsed))
}

fn project_snapshot(parsed: ParsedRegistry) -> AgentRegistrySnapshot {
    let now = time::OffsetDateTime::now_utc();
    let observed_at = format_time(now);
    let expires_at = format_time(now + time::Duration::seconds(CACHE_TTL.as_secs() as i64));
    let items = parsed
        .items
        .into_iter()
        .map(|agent| project_agent(agent, &parsed.source_revision, &observed_at, &expires_at))
        .collect();
    AgentRegistrySnapshot {
        source_registry: REGISTRY_URL.to_owned(),
        source_revision: Some(parsed.source_revision),
        observed_at,
        expires_at: Some(expires_at),
        stale: false,
        items,
    }
}

fn project_agent(
    agent: RegistryAgent,
    source_revision: &str,
    observed_at: &str,
    expires_at: &str,
) -> AgentRegistryEntry {
    AgentRegistryEntry {
        registry_agent_id: agent.registry_agent_id,
        display_name: agent.display_name,
        description: agent.description,
        homepage: agent.homepage,
        source_registry: REGISTRY_URL.to_owned(),
        source_revision: Some(source_revision.to_owned()),
        distributions: agent
            .distributions
            .into_iter()
            .map(|distribution| AgentDistribution {
                distribution_id: distribution.distribution_id,
                kind: distribution.kind.to_owned(),
                package_ref: distribution.package_ref,
                version_constraint: distribution.version_constraint,
                os: distribution.os.into_iter().map(str::to_owned).collect(),
                architecture: distribution
                    .architecture
                    .into_iter()
                    .map(str::to_owned)
                    .collect(),
            })
            .collect(),
        // No production AgentModule descriptors are registered yet. ACP presence is never
        // promoted to support, installation, authentication, or execution readiness.
        adapter_support: "NO_ADAPTER".to_owned(),
        metadata_observed_at: observed_at.to_owned(),
        cache_expires_at: Some(expires_at.to_owned()),
    }
}

fn format_time(value: time::OffsetDateTime) -> String {
    value
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_owned())
}

fn normalize_query(query: Option<String>) -> Result<Option<String>, Response> {
    let Some(query) = query else {
        return Ok(None);
    };
    let query = query.trim();
    if query.chars().count() > 128 || query.chars().any(char::is_control) {
        return Err(registry_error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "Registry search text is invalid",
        ));
    }
    Ok((!query.is_empty()).then(|| query.to_lowercase()))
}

fn matches_query(entry: &AgentRegistryEntry, query: Option<&str>) -> bool {
    let Some(query) = query else {
        return true;
    };
    let query = query.to_lowercase();
    entry.registry_agent_id.to_lowercase().contains(&query)
        || entry.display_name.to_lowercase().contains(&query)
        || entry
            .description
            .as_deref()
            .is_some_and(|description| description.to_lowercase().contains(&query))
}

fn encode_cursor(cursor: &RegistryCursor) -> Result<String, Response> {
    let json = serde_json::to_vec(cursor).map_err(|_| {
        registry_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL",
            "The ACP agent registry cursor could not be encoded",
        )
    })?;
    Ok(URL_SAFE_NO_PAD.encode(json))
}

fn snapshot_digest(snapshot: &AgentRegistrySnapshot) -> Result<String, Response> {
    use sha2::Digest;

    #[derive(Serialize)]
    struct SnapshotIdentity<'a> {
        source_registry: &'a str,
        source_revision: &'a Option<String>,
        observed_at: &'a str,
        expires_at: &'a Option<String>,
        items: &'a [AgentRegistryEntry],
    }
    // `stale` is a time-derived serving state, not part of the durable snapshot
    // identity. A cursor remains valid if the same cached snapshot expires while
    // the UI paginates during an upstream outage.
    let identity = SnapshotIdentity {
        source_registry: &snapshot.source_registry,
        source_revision: &snapshot.source_revision,
        observed_at: &snapshot.observed_at,
        expires_at: &snapshot.expires_at,
        items: &snapshot.items,
    };
    let bytes = serde_json::to_vec(&identity).map_err(|_| {
        registry_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL",
            "The ACP agent registry snapshot could not be encoded",
        )
    })?;
    Ok(format!("sha256:{:x}", sha2::Sha256::digest(bytes)))
}

fn decode_cursor(
    cursor: Option<&str>,
    snapshot: &AgentRegistrySnapshot,
    query: Option<&str>,
) -> Result<usize, Response> {
    let Some(cursor) = cursor else {
        return Ok(0);
    };
    if cursor.len() > 1024 {
        return Err(invalid_cursor());
    }
    let decoded = URL_SAFE_NO_PAD
        .decode(cursor)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<RegistryCursor>(&bytes).ok())
        .ok_or_else(invalid_cursor)?;
    if decoded.version != 1
        || decoded.source_revision != snapshot.source_revision
        || decoded.query.as_deref() != query
        || !matches!(snapshot_digest(snapshot), Ok(digest) if digest == decoded.snapshot_digest)
    {
        return Err(invalid_cursor());
    }
    Ok(decoded.offset)
}

fn invalid_cursor() -> Response {
    registry_error(
        StatusCode::BAD_REQUEST,
        "INVALID_ARGUMENT",
        "Registry cursor is invalid or belongs to another snapshot or search",
    )
}

fn registry_error(status: StatusCode, code: &'static str, message: &'static str) -> Response {
    let sequence = ERROR_SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let correlation_id = format!("cor_registry_{sequence}");
    (
        status,
        [(
            axum::http::header::HeaderName::from_static("x-correlation-id"),
            axum::http::HeaderValue::from_str(&correlation_id)
                .unwrap_or_else(|_| axum::http::HeaderValue::from_static("cor_registry_error")),
        )],
        Json(serde_json::json!({
            "code": code,
            "message": message,
            "retryable": status.is_server_error(),
            "correlation_id": correlation_id
        })),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, name: &str) -> AgentRegistryEntry {
        AgentRegistryEntry {
            registry_agent_id: id.to_owned(),
            display_name: name.to_owned(),
            description: None,
            homepage: None,
            source_registry: REGISTRY_URL.to_owned(),
            source_revision: Some("rev-1".to_owned()),
            distributions: vec![],
            adapter_support: "NO_ADAPTER".to_owned(),
            metadata_observed_at: "2026-10-17T00:00:00Z".to_owned(),
            cache_expires_at: Some("2026-10-17T00:15:00Z".to_owned()),
        }
    }

    fn snapshot() -> AgentRegistrySnapshot {
        AgentRegistrySnapshot {
            source_registry: REGISTRY_URL.to_owned(),
            source_revision: Some("rev-1".to_owned()),
            observed_at: "2026-10-17T00:00:00Z".to_owned(),
            expires_at: Some("2026-10-17T00:15:00Z".to_owned()),
            stale: false,
            items: vec![entry("alpha", "Alpha"), entry("beta", "Beta")],
        }
    }

    #[test]
    fn cursor_is_bound_to_registry_revision_and_search() {
        let encoded = encode_cursor(&RegistryCursor {
            version: 1,
            snapshot_digest: snapshot_digest(&snapshot()).unwrap(),
            source_revision: Some("rev-1".to_owned()),
            query: Some("beta".to_owned()),
            offset: 10,
        })
        .unwrap();
        assert_eq!(
            decode_cursor(Some(&encoded), &snapshot(), Some("beta")).unwrap(),
            10
        );
        assert!(decode_cursor(Some(&encoded), &snapshot(), Some("alpha")).is_err());
        let mut stale = snapshot();
        stale.stale = true;
        assert_eq!(
            decode_cursor(Some(&encoded), &stale, Some("beta")).unwrap(),
            10,
            "time-derived stale state does not invalidate the cached snapshot cursor"
        );
        let mut changed = snapshot();
        changed.source_revision = Some("rev-2".to_owned());
        assert!(decode_cursor(Some(&encoded), &changed, Some("beta")).is_err());
        let mut changed_metadata = snapshot();
        changed_metadata.items[0].display_name = "Changed content at same revision".to_owned();
        assert!(decode_cursor(Some(&encoded), &changed_metadata, Some("beta")).is_err());
    }

    #[test]
    fn query_matching_is_case_insensitive_and_searches_safe_metadata_only() {
        let agent = entry("claude-code", "Claude Code");
        assert!(matches_query(&agent, Some("CLAUDE")));
        assert!(!matches_query(&agent, Some("secret")));
    }

    #[test]
    fn registry_presence_never_claims_adapter_support() {
        let projected = project_agent(
            RegistryAgent {
                registry_agent_id: "example-agent".to_owned(),
                display_name: "Example".to_owned(),
                description: None,
                homepage: None,
                distributions: vec![],
            },
            "rev-1",
            "2026-10-17T00:00:00Z",
            "2026-10-17T00:15:00Z",
        );
        assert_eq!(projected.adapter_support, "NO_ADAPTER");
    }

    #[test]
    fn persistent_expired_cache_remains_displayable_but_unqualified() {
        let cached = storage_core::AgentRegistryCacheSnapshot {
            source_registry: REGISTRY_URL.to_owned(),
            source_revision: Some("rev-1".to_owned()),
            observed_at: "2026-10-17T00:00:00Z".to_owned(),
            expires_at: Some("1970-01-01T00:00:00Z".to_owned()),
            entries: vec![storage_core::AgentRegistryCacheEntry {
                registry_agent_id: "codex-acp".to_owned(),
                metadata: serde_json::json!({
                    "display_name": "Codex",
                    "description": null,
                    "homepage": null,
                    "source_registry": REGISTRY_URL,
                    "source_revision": "rev-1",
                    "distributions": [],
                    "adapter_support": "SUPPORTED",
                    "metadata_observed_at": "2026-10-17T00:00:00Z",
                    "cache_expires_at": "1970-01-01T00:00:00Z"
                }),
            }],
        };

        let snapshot = project_cached_snapshot(cached).expect("valid persisted snapshot");
        assert!(snapshot.stale);
        assert_eq!(snapshot.items.len(), 1);
        assert_eq!(snapshot.items[0].display_name, "Codex");
        assert_eq!(snapshot.items[0].adapter_support, "NO_ADAPTER");
    }

    #[test]
    fn malformed_persistent_cache_is_rejected_as_a_whole() {
        let cached = storage_core::AgentRegistryCacheSnapshot {
            source_registry: REGISTRY_URL.to_owned(),
            source_revision: Some("rev-1".to_owned()),
            observed_at: "2026-10-17T00:00:00Z".to_owned(),
            expires_at: None,
            entries: vec![storage_core::AgentRegistryCacheEntry {
                registry_agent_id: "../agent".to_owned(),
                metadata: serde_json::json!({}),
            }],
        };
        assert!(project_cached_snapshot(cached).is_err());
    }
}
