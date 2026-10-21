//! Read-only Discover catalog projection.
//!
//! Authority: [`docs/CAPABILITY-FABRIC.md`](../../../docs/CAPABILITY-FABRIC.md),
//! [`docs/CONNECTORS-WORKFLOWS.md`](../../../docs/CONNECTORS-WORKFLOWS.md), and
//! [`docs/V1-UI-ACTION-STATE-LEDGER.md`](../../../docs/V1-UI-ACTION-STATE-LEDGER.md).
//!
//! This adapter is a **projection**, not a provider gateway. It never contacts a
//! package manager, never starts a process, and never loads a provider tool
//! schema. It projects only what LiteCowork already owns: the Workspace's own
//! Skill definitions.
//!
//! The LiteSPM package service is not reachable from here: its API,
//! authentication, and package model are intentionally deferred in
//! `docs/CAPABILITY-FABRIC.md`. The projection therefore reports the package
//! catalog as *not qualified* instead of inventing endpoints or pretending a
//! package was discovered.
//!
//! The Runtime's observed capability hosts are a separate observation, not a
//! catalog row. A host instance is an expiring readiness observation owned by
//! the Runtime; it is neither installable nor a package the owner browses.
//! Mixing the two would collapse "installed", "offered", and "available" into
//! one row, which the capability fabric explicitly forbids. The hosts stay
//! available through the Runtime capability-host route.

use super::*;
use serde::{Deserialize, Serialize};
use storage_core::{
    DiscoverAuthState, DiscoverAvailability, DiscoverCatalogEntry,
    DiscoverCatalogEntryDetail, DiscoverCompatibility, DiscoverEntryKind, DiscoverEntrySource,
    DiscoverInstallState, RuntimeCapabilityHostPageRequest, RuntimeCapabilityRef, SkillKind,
    SkillRecord, SkillStatus,
};

use crate::skill::SqliteSkillStore;

/// The reason code reported whenever the LiteSPM package service is required
/// but its published contract is unavailable.
pub const PACKAGE_SERVICE_NOT_QUALIFIED: &str = "LITESPOM_PACKAGE_SERVICE_NOT_QUALIFIED";

/// The maximum number of Runtime capability-host observations one page carries.
pub const DISCOVER_MAX_RUNTIME_CAPABILITIES: usize = 50;

/// The truthfulness of the owning provider behind a catalog search.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DiscoverProviderStatus {
    /// Always `false` until the LiteSPM package contract is published and
    /// qualified. The catalog never pretends a package was discovered.
    pub package_service_qualified: bool,
    pub reason_code: String,
    pub message: String,
}

/// A bounded page of Discover catalog entries.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DiscoverCatalogPage {
    /// The installable/owned catalog stream. A catalog row is never authority.
    pub items: Vec<DiscoverCatalogEntry>,
    pub next_cursor: Option<(String, String)>,
    /// The owning provider's truthfulness. The package catalog is not qualified.
    pub provider_status: DiscoverProviderStatus,
    /// The Runtime's observed capability hosts. Observations, not catalog rows.
    pub runtime_capabilities: Vec<DiscoverCatalogEntry>,
}

impl Default for DiscoverProviderStatus {
    fn default() -> Self {
        Self {
            package_service_qualified: false,
            reason_code: PACKAGE_SERVICE_NOT_QUALIFIED.to_owned(),
            message: "Package discovery is unavailable until the LiteSPM package contract is published and qualified.".to_owned(),
        }
    }
}

/// The read-only Discover catalog projection.
#[derive(Clone)]
pub struct SqliteDiscoverStore {
    store: SqliteWorkspaceStore,
}

impl SqliteDiscoverStore {
    pub fn new(store: SqliteWorkspaceStore) -> Self {
        Self { store }
    }

    /// Search the bounded Discover catalog of one Workspace.
    ///
    /// The projection never loads a provider tool schema and never spawns a
    /// process, so a large configured inventory stays cheap to page through.
    /// The query and kind filters run while the page is built, so a filtered
    /// page neither skips a matching entry nor repeats one.
    pub fn search_catalog(
        &self,
        principal_id: &str,
        workspace_id: &str,
        runtime_id: &str,
        runtime_incarnation_id: &str,
        query: Option<&str>,
        kind: Option<DiscoverEntryKind>,
        now: &str,
        after: Option<(String, String)>,
        limit: usize,
    ) -> Result<DiscoverCatalogPage, StoreError> {
        if !(1..=200).contains(&limit) {
            return Err(StoreError::Invalid(
                "Discover page limit is invalid".to_owned(),
            ));
        }
        let normalized_query = query.map(normalize_query).filter(|value| !value.is_empty());
        let filter_kind = kind;
        let filter_query = normalized_query.clone();
        let skills = SqliteSkillStore::read_only(self.store.clone()).list_skills_matching(
            principal_id,
            workspace_id,
            after,
            limit,
            move |skill| {
                let entry = skill_catalog_entry(skill);
                if let Some(kind) = filter_kind {
                    if entry.kind != kind {
                        return false;
                    }
                }
                match filter_query.as_deref() {
                    Some(query) => {
                        normalize_query(&entry.display_name).contains(query)
                            || normalize_query(&entry.summary).contains(query)
                            || normalize_query(&entry.item_ref).contains(query)
                    }
                    None => true,
                }
            },
        )?;
        let items: Vec<DiscoverCatalogEntry> = skills.items.iter().map(skill_catalog_entry).collect();
        let runtime_capabilities = self.runtime_capability_entries(
            principal_id,
            workspace_id,
            runtime_id,
            runtime_incarnation_id,
            now,
        );
        Ok(DiscoverCatalogPage {
            items,
            next_cursor: skills.next,
            provider_status: DiscoverProviderStatus::default(),
            runtime_capabilities,
        })
    }

    /// Describe one catalog entry.
    ///
    /// The detail never contains a provider handle, a credential, or a raw
    /// provider payload. The package service is not qualified, so no package
    /// detail is available.
    pub fn describe_entry(
        &self,
        principal_id: &str,
        workspace_id: &str,
        runtime_id: &str,
        runtime_incarnation_id: &str,
        item_ref: &str,
        now: &str,
    ) -> Result<Option<DiscoverCatalogEntryDetail>, StoreError> {
        if let Some(skill_id) = item_ref.strip_prefix("skill:") {
            let skill = SqliteSkillStore::read_only(self.store.clone())
                .get_skill(principal_id, workspace_id, skill_id)?;
            let Some(skill) = skill else {
                return Ok(None);
            };
            return Ok(Some(DiscoverCatalogEntryDetail {
                entry: skill_catalog_entry(&skill),
                capabilities: Vec::new(),
                provider_note: Some(
                    "Package publication stays with LiteSPM and is not qualified yet.".to_owned(),
                ),
            }));
        }
        let Some(host) = self
            .runtime_capability_entries(
                principal_id,
                workspace_id,
                runtime_id,
                runtime_incarnation_id,
                now,
            )
            .into_iter()
            .find(|entry| entry.item_ref == item_ref)
        else {
            return Ok(None);
        };
        Ok(Some(DiscoverCatalogEntryDetail {
            entry: host,
            capabilities: Vec::new(),
            provider_note: Some(
                "This is a Runtime observation of an existing capability host, not an installable catalog row.".to_owned(),
            ),
        }))
    }

    /// The Runtime's bounded capability-host observations.
    ///
    /// A host that cannot be observed is simply absent: the projection never
    /// invents a health or readiness value it did not observe.
    fn runtime_capability_entries(
        &self,
        principal_id: &str,
        workspace_id: &str,
        runtime_id: &str,
        runtime_incarnation_id: &str,
        now: &str,
    ) -> Vec<DiscoverCatalogEntry> {
        self.store
            .list_runtime_capability_hosts(RuntimeCapabilityHostPageRequest {
                owner_principal_id: principal_id.to_owned(),
                workspace_id: workspace_id.to_owned(),
                runtime_id: runtime_id.to_owned(),
                expected_incarnation_id: Some(runtime_incarnation_id.to_owned()),
                now: now.to_owned(),
                after: None,
                limit: DISCOVER_MAX_RUNTIME_CAPABILITIES,
            })
            .unwrap_or_default()
            .iter()
            .map(host_catalog_entry)
            .collect()
    }
}

/// Project one LiteCowork-owned Skill definition into a catalog entry.
fn skill_catalog_entry(skill: &SkillRecord) -> DiscoverCatalogEntry {
    let revision = skill.current_revision_record();
    let display_name = revision
        .map(|revision| revision.title.clone())
        .unwrap_or_else(|| skill.skill_id.clone());
    let summary = revision
        .map(|revision| revision.summary.clone())
        .unwrap_or_else(|| "Skill definition".to_owned());
    let mut entry = DiscoverCatalogEntry::new(
        format!("skill:{}", skill.skill_id),
        match skill.kind {
            SkillKind::LocalProcedure => DiscoverEntryKind::Skill,
            SkillKind::McpSkill => DiscoverEntryKind::Skill,
            SkillKind::PackageComponent => DiscoverEntryKind::Capability,
        },
        display_name,
        summary,
        DiscoverEntrySource::LiteCoworkLocal,
    );
    entry.description = revision.map(|revision| revision.summary.clone());
    entry.compatibility = DiscoverCompatibility::Compatible;
    entry.install_state = DiscoverInstallState::Installed;
    entry.auth_state = DiscoverAuthState::NotRequired;
    entry.publisher = Some("LiteCowork".to_owned());
    entry.version = revision.map(|revision| revision.revision.to_string());
    entry.digest = revision.map(|revision| revision.manifest_digest.clone());
    entry.grants_required = revision
        .map(|revision| revision.declared_capabilities.clone())
        .unwrap_or_default();
    match skill.status {
        SkillStatus::Active => {
            entry.availability = DiscoverAvailability::Available;
            entry.availability_reason = "READY".to_owned();
            entry.compatibility_reason = None;
        }
        SkillStatus::Draft => {
            entry.availability = DiscoverAvailability::Unsupported;
            entry.availability_reason = "SKILL_NOT_ACTIVE".to_owned();
            entry.compatibility = DiscoverCompatibility::Incompatible;
            entry.compatibility_reason = Some("SKILL_NOT_ACTIVE".to_owned());
        }
        SkillStatus::Suspended => {
            entry.availability = DiscoverAvailability::Unsupported;
            entry.availability_reason = "SKILL_SUSPENDED".to_owned();
            entry.compatibility = DiscoverCompatibility::Incompatible;
            entry.compatibility_reason = Some("SKILL_SUSPENDED".to_owned());
        }
        SkillStatus::Revoked => {
            entry.availability = DiscoverAvailability::Unsupported;
            entry.availability_reason = "SKILL_REVOKED".to_owned();
            entry.compatibility = DiscoverCompatibility::Incompatible;
            entry.compatibility_reason = Some("SKILL_REVOKED".to_owned());
        }
    }
    entry
}

/// Project one observed Runtime capability host into an observation entry.
///
/// The entry reports the owning source truthfully: an MCP Skill and a
/// LiteSPM package component are different owners, and an MCP Skill is a Skill
/// rather than a general capability.
fn host_catalog_entry(host: &storage_core::RuntimeCapabilityHostRecord) -> DiscoverCatalogEntry {
    let (display_name, digest, component, entry_source, entry_kind, item_ref) =
        match &host.capability_ref {
            RuntimeCapabilityRef::PackageComponent {
                capability_id,
                source: _,
                digest,
                component,
                ..
            } => (
                capability_id.clone(),
                digest.clone(),
                component.clone(),
                DiscoverEntrySource::LiteSpmPackage,
                DiscoverEntryKind::Capability,
                format!("capability:{}", host.host_instance_id),
            ),
            RuntimeCapabilityRef::McpSkill {
                capability_id,
                source: _,
                digest,
                component,
            } => (
                capability_id.clone(),
                digest.clone(),
                component.clone(),
                DiscoverEntrySource::McpServer,
                DiscoverEntryKind::Skill,
                format!("mcp-skill:{}", host.host_instance_id),
            ),
        };
    let mut entry = DiscoverCatalogEntry::new(
        item_ref,
        entry_kind,
        display_name,
        format!("{} capability offered by this Runtime", component),
        entry_source,
    );
    entry.description = Some(component);
    entry.digest = Some(digest);
    entry.install_state = match host.state.as_str() {
        "ACTIVE" | "HEALTHY" => DiscoverInstallState::Installed,
        _ => DiscoverInstallState::Unknown,
    };
    entry.auth_state = DiscoverAuthState::Unknown;
    entry.compatibility = if host.health == "HEALTHY" {
        DiscoverCompatibility::Compatible
    } else {
        DiscoverCompatibility::Unknown
    };
    entry.availability = match host.health.as_str() {
        "HEALTHY" => DiscoverAvailability::Available,
        _ => DiscoverAvailability::Error,
    };
    entry.availability_reason = match host.health.as_str() {
        "HEALTHY" => "READY".to_owned(),
        "DEGRADED" => "PROVIDER_DEGRADED".to_owned(),
        _ => "PROVIDER_HEALTH_UNKNOWN".to_owned(),
    };
    entry
}

fn normalize_query(value: &str) -> String {
    value.trim().to_lowercase()
}
