//! Discover catalog projection and ConnectorPackage descriptor validation.
//!
//! Authority:
//! - [`docs/CAPABILITY-FABRIC.md`](../../../docs/CAPABILITY-FABRIC.md) — LiteSPM owns
//!   package discovery/lifecycle; LiteCowork owns Task-scoped use; an offer is not
//!   proof of installation, health, an account connection, or permission.
//! - [`docs/CONNECTORS-WORKFLOWS.md`](../../../docs/CONNECTORS-WORKFLOWS.md) — the
//!   `ConnectorPackageManifest`, auth recipes, actions, and triggers.
//! - [`docs/V1-UI-ACTION-STATE-LEDGER.md`](../../../docs/V1-UI-ACTION-STATE-LEDGER.md) —
//!   the Discover surface states `AVAILABLE`, `NEEDS_AUTH`, `UNSUPPORTED`, `ERROR`.
//!
//! This module is a **projection**, not a provider gateway. It never contacts a
//! package manager, never spawns a process, and never loads a provider tool
//! schema. Every entry it produces is explicitly non-authoritative: a catalog
//! listing is not trusted merely because it exists.

use serde::{Deserialize, Serialize};

use crate::is_sha256_digest;

/// The reason code used when the LiteSPM package service is not available.
pub const PACKAGE_SERVICE_UNAVAILABLE: &str = "PACKAGE_SERVICE_UNAVAILABLE";
/// The reason code used when a catalog entry is not compatible with this Runtime.
pub const INCOMPATIBLE_ENTRY: &str = "INCOMPATIBLE_ENTRY";
/// The reason code used when an entry needs the owner's authentication first.
pub const AUTH_REQUIRED: &str = "AUTH_REQUIRED";
/// The reason code used when the entry's install state cannot be observed.
pub const INSTALL_STATE_UNKNOWN: &str = "INSTALL_STATE_UNKNOWN";
/// The reason code used when the entry is ready.
pub const ENTRY_READY: &str = "READY";

/// Maximum number of auth recipes one package descriptor may carry.
pub const CONNECTOR_MAX_AUTH_RECIPES: usize = 32;
/// Maximum number of actions one package descriptor may carry.
pub const CONNECTOR_MAX_ACTIONS: usize = 512;
/// Maximum number of triggers one package descriptor may carry.
pub const CONNECTOR_MAX_TRIGGERS: usize = 128;
/// Maximum byte length of one catalog entry summary.
pub const DISCOVER_SUMMARY_MAX_BYTES: usize = 2_000;

/// The kind of entry in the Discover catalog.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DiscoverEntryKind {
    /// An application connector.
    App,
    /// A general capability offered by a qualified provider.
    Capability,
    /// A Skill.
    Skill,
    /// A compatible agent.
    Agent,
}

impl DiscoverEntryKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::App => "APP",
            Self::Capability => "CAPABILITY",
            Self::Skill => "SKILL",
            Self::Agent => "AGENT",
        }
    }
}

/// The user-visible availability of one catalog entry.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DiscoverAvailability {
    /// The entry is usable in this Workspace right now.
    Available,
    /// The owner must authenticate before the entry becomes usable.
    NeedsAuth,
    /// The entry is not supported by this Runtime or Workspace.
    Unsupported,
    /// The entry's state could not be observed.
    Error,
    /// The owning provider is not qualified, so the entry cannot be used.
    NotQualified,
}

impl DiscoverAvailability {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Available => "AVAILABLE",
            Self::NeedsAuth => "NEEDS_AUTH",
            Self::Unsupported => "UNSUPPORTED",
            Self::Error => "ERROR",
            Self::NotQualified => "NOT_QUALIFIED",
        }
    }
}

/// The compatibility decision for one catalog entry.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DiscoverCompatibility {
    Compatible,
    Incompatible,
    Unknown,
}

impl DiscoverCompatibility {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Compatible => "COMPATIBLE",
            Self::Incompatible => "INCOMPATIBLE",
            Self::Unknown => "UNKNOWN",
        }
    }
}

/// The observed installation state of one catalog entry.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DiscoverInstallState {
    NotInstalled,
    Installed,
    InstallPending,
    Unknown,
}

impl DiscoverInstallState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotInstalled => "NOT_INSTALLED",
            Self::Installed => "INSTALLED",
            Self::InstallPending => "INSTALL_PENDING",
            Self::Unknown => "UNKNOWN",
        }
    }
}

/// The observed authentication state of one catalog entry.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DiscoverAuthState {
    NotRequired,
    Required,
    Authenticated,
    Unknown,
}

impl DiscoverAuthState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotRequired => "NOT_REQUIRED",
            Self::Required => "REQUIRED",
            Self::Authenticated => "AUTHENTICATED",
            Self::Unknown => "UNKNOWN",
        }
    }
}

/// Which owner published one catalog entry.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DiscoverEntrySource {
    /// A LiteSPM-managed package.
    LiteSpmPackage,
    /// An MCP server observed by a qualified Runtime.
    McpServer,
    /// A LiteCowork-owned local record.
    LiteCoworkLocal,
}

impl DiscoverEntrySource {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LiteSpmPackage => "LITE_SPM_PACKAGE",
            Self::McpServer => "MCP_SERVER",
            Self::LiteCoworkLocal => "LITECOWORK_LOCAL",
        }
    }
}

/// One bounded, user-facing Discover catalog entry.
///
/// The entry is a projection. It never carries a credential, never carries a
/// provider handle, and never asserts that the entry is installed, authenticated,
/// assigned, or authorized.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DiscoverCatalogEntry {
    pub item_ref: String,
    pub kind: DiscoverEntryKind,
    pub display_name: String,
    pub summary: String,
    pub description: Option<String>,
    pub compatibility: DiscoverCompatibility,
    pub compatibility_reason: Option<String>,
    pub availability: DiscoverAvailability,
    pub availability_reason: String,
    pub install_state: DiscoverInstallState,
    pub auth_state: DiscoverAuthState,
    pub source: DiscoverEntrySource,
    pub publisher: Option<String>,
    pub version: Option<String>,
    pub digest: Option<String>,
    /// Names of the grants the entry would need. Names only, never authority.
    pub grants_required: Vec<String>,
    /// Always `false`. A catalog entry is never an authorization.
    pub authoritative: bool,
}

impl DiscoverCatalogEntry {
    /// Build an entry. The `authoritative` flag is always `false`: install and
    /// discovery metadata never becomes authority.
    pub fn new(
        item_ref: String,
        kind: DiscoverEntryKind,
        display_name: String,
        summary: String,
        source: DiscoverEntrySource,
    ) -> Self {
        Self {
            item_ref,
            kind,
            display_name,
            summary,
            description: None,
            compatibility: DiscoverCompatibility::Unknown,
            compatibility_reason: None,
            availability: DiscoverAvailability::Error,
            availability_reason: INSTALL_STATE_UNKNOWN.to_owned(),
            install_state: DiscoverInstallState::Unknown,
            auth_state: DiscoverAuthState::Unknown,
            source,
            publisher: None,
            version: None,
            digest: None,
            grants_required: Vec::new(),
            authoritative: false,
        }
    }

    /// True when the owner must act before the entry becomes usable.
    pub fn requires_user_action(&self) -> bool {
        self.availability == DiscoverAvailability::NeedsAuth
    }

    /// True when the UI must disable this entry.
    pub fn is_disabled(&self) -> bool {
        !matches!(self.availability, DiscoverAvailability::Available)
    }

    /// Validate the entry's own shape.
    pub fn validate(&self) -> Result<(), DiscoverError> {
        if self.item_ref.trim().is_empty() || self.item_ref.len() > 256 {
            return Err(DiscoverError::InvalidEntry);
        }
        if self.display_name.trim().is_empty() || self.display_name.len() > 256 {
            return Err(DiscoverError::InvalidEntry);
        }
        if self.summary.len() > DISCOVER_SUMMARY_MAX_BYTES {
            return Err(DiscoverError::InvalidEntry);
        }
        if self.authoritative {
            // A catalog entry that claims authority is always rejected.
            return Err(DiscoverError::EntryClaimsAuthority);
        }
        if let Some(digest) = self.digest.as_deref() {
            if !is_sha256_digest(digest) {
                return Err(DiscoverError::InvalidEntry);
            }
        }
        Ok(())
    }
}

/// Every deterministic rejection the Discover projection can produce.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiscoverError {
    InvalidEntry,
    EntryClaimsAuthority,
    InvalidManifest,
    TooManyAuthRecipes,
    TooManyActions,
    TooManyTriggers,
    DuplicateKey,
    InvalidDigest,
    InvalidOperationClass,
    InvalidAuthRecipeKind,
    InvalidScopeRisk,
    InvalidRedirectPolicy,
    InvalidIdempotency,
    InconsistentAuthRecipe,
}

impl DiscoverError {
    pub const fn reason_code(self) -> &'static str {
        match self {
            Self::InvalidEntry => "INVALID_DISCOVER_ENTRY",
            Self::EntryClaimsAuthority => "DISCOVER_ENTRY_CLAIMS_AUTHORITY",
            Self::InvalidManifest => "INVALID_CONNECTOR_MANIFEST",
            Self::TooManyAuthRecipes => "TOO_MANY_AUTH_RECIPES",
            Self::TooManyActions => "TOO_MANY_ACTIONS",
            Self::TooManyTriggers => "TOO_MANY_TRIGGERS",
            Self::DuplicateKey => "DUPLICATE_DESCRIPTOR_KEY",
            Self::InvalidDigest => "INVALID_DIGEST",
            Self::InvalidOperationClass => "INVALID_OPERATION_CLASS",
            Self::InvalidAuthRecipeKind => "INVALID_AUTH_RECIPE_KIND",
            Self::InvalidScopeRisk => "INVALID_SCOPE_RISK",
            Self::InvalidRedirectPolicy => "INVALID_REDIRECT_POLICY",
            Self::InvalidIdempotency => "INVALID_IDEMPOTENCY_CLASS",
            Self::InconsistentAuthRecipe => "INCONSISTENT_AUTH_RECIPE",
        }
    }
}

impl std::fmt::Display for DiscoverError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.reason_code())
    }
}

impl std::error::Error for DiscoverError {}

/// Derive the user-visible availability of one catalog entry.
///
/// The order matters and fails closed: an unqualified provider outranks every
/// other state, an incompatible entry is never offered, and an unknown install
/// state is an error rather than a silent "available".
pub fn derive_discover_availability(
    provider_qualified: bool,
    compatibility: DiscoverCompatibility,
    install_state: DiscoverInstallState,
    auth_state: DiscoverAuthState,
) -> (DiscoverAvailability, String) {
    if !provider_qualified {
        return (
            DiscoverAvailability::NotQualified,
            PACKAGE_SERVICE_UNAVAILABLE.to_owned(),
        );
    }
    if compatibility == DiscoverCompatibility::Incompatible {
        return (
            DiscoverAvailability::Unsupported,
            INCOMPATIBLE_ENTRY.to_owned(),
        );
    }
    if install_state == DiscoverInstallState::Unknown {
        return (
            DiscoverAvailability::Error,
            INSTALL_STATE_UNKNOWN.to_owned(),
        );
    }
    if auth_state == DiscoverAuthState::Required {
        return (DiscoverAvailability::NeedsAuth, AUTH_REQUIRED.to_owned());
    }
    if auth_state == DiscoverAuthState::Unknown {
        return (
            DiscoverAvailability::Error,
            INSTALL_STATE_UNKNOWN.to_owned(),
        );
    }
    (DiscoverAvailability::Available, ENTRY_READY.to_owned())
}

// ---------------------------------------------------------------------------
// ConnectorPackage descriptor
// ---------------------------------------------------------------------------

/// The provider authentication protocol one recipe describes.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AuthRecipeKind {
    PublicOauthPkceLoopback,
    PublicOauthPkceCustomUri,
    DeviceCode,
    ConfidentialOauth,
    ApiKey,
    ServiceAccount,
    NativeSession,
    NoAuth,
}

impl AuthRecipeKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PublicOauthPkceLoopback => "PUBLIC_OAUTH_PKCE_LOOPBACK",
            Self::PublicOauthPkceCustomUri => "PUBLIC_OAUTH_PKCE_CUSTOM_URI",
            Self::DeviceCode => "DEVICE_CODE",
            Self::ConfidentialOauth => "CONFIDENTIAL_OAUTH",
            Self::ApiKey => "API_KEY",
            Self::ServiceAccount => "SERVICE_ACCOUNT",
            Self::NativeSession => "NATIVE_SESSION",
            Self::NoAuth => "NO_AUTH",
        }
    }

    /// Public OAuth recipes must use PKCE.
    pub const fn requires_pkce(self) -> bool {
        matches!(
            self,
            Self::PublicOauthPkceLoopback | Self::PublicOauthPkceCustomUri
        )
    }

    /// A recipe that can hold a client secret.
    pub const fn may_hold_client_secret(self) -> bool {
        matches!(self, Self::ConfidentialOauth | Self::ServiceAccount)
    }
}

/// The risk class of the scopes one recipe requests.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ScopeRisk {
    Read,
    Write,
    Sensitive,
    Restricted,
}

/// The redirect policy one recipe declares.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RedirectPolicy {
    LoopbackRandomPort,
    FixedHttps,
    CustomUri,
    None,
}

/// The provider authentication recipe one connector offers.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AuthRecipeDescriptor {
    pub recipe_id: String,
    pub kind: AuthRecipeKind,
    pub client_secret_required: bool,
    pub pkce_required: bool,
    pub redirect_policy: RedirectPolicy,
    pub default_scopes: Vec<String>,
    pub optional_scopes: Vec<String>,
    pub scope_risk: ScopeRisk,
    pub supports_refresh: bool,
    pub provider_constraints: Vec<String>,
}

/// The provider-declared class of one connector action.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ConnectorOperationClass {
    Read,
    Write,
    Send,
    Delete,
    PurchaseLike,
    Admin,
}

impl ConnectorOperationClass {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Read => "READ",
            Self::Write => "WRITE",
            Self::Send => "SEND",
            Self::Delete => "DELETE",
            Self::PurchaseLike => "PURCHASE_LIKE",
            Self::Admin => "ADMIN",
        }
    }

    /// True when the provider-declared class implies a consequential operation.
    ///
    /// This is the *declared* class only. TrustService keeps its own
    /// classification, and a declared `READ` label never downgrades a
    /// consequential action.
    pub const fn is_consequential(self) -> bool {
        !matches!(self, Self::Read)
    }
}

/// The provider-declared idempotency guarantee of one action.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ConnectorIdempotency {
    ProviderKey,
    ClientKey,
    ReadOnly,
    Unknown,
}

/// One connector action descriptor.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConnectorActionDescriptor {
    pub action_key: String,
    pub title: String,
    pub description: String,
    pub operation_class: ConnectorOperationClass,
    pub idempotency: ConnectorIdempotency,
    pub required_scopes: Vec<String>,
}

/// One connector trigger descriptor.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConnectorTriggerDescriptor {
    pub trigger_key: String,
    pub title: String,
    pub required_scopes: Vec<String>,
    pub stable_delivery_id: bool,
}

/// The bounded descriptor of one LiteSPM-managed connector package.
///
/// LiteSPM owns the package and the descriptor is the normalized, qualified
/// subset LiteCowork consumes. It is untrusted input until validation passes.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConnectorPackageManifest {
    pub package_id: String,
    /// Opaque, non-empty version label exactly as LiteSPM normalized it.
    pub package_version: String,
    pub publisher: String,
    pub package_digest: String,
    pub license_spdx: Option<String>,
    pub source_url: Option<String>,
    pub connector_id: String,
    pub display_name: String,
    pub category: String,
    pub auth_recipes: Vec<AuthRecipeDescriptor>,
    pub actions: Vec<ConnectorActionDescriptor>,
    pub triggers: Vec<ConnectorTriggerDescriptor>,
}

/// Validate one connector package descriptor.
///
/// A malformed or hostile descriptor is rejected rather than repaired, and a
/// declared `READ` class is never treated as proof that an action is harmless.
pub fn validate_connector_package_manifest(
    manifest: &ConnectorPackageManifest,
) -> Result<(), DiscoverError> {
    if manifest.package_id.trim().is_empty()
        || manifest.package_version.trim().is_empty()
        || manifest.publisher.trim().is_empty()
        || manifest.connector_id.trim().is_empty()
        || manifest.display_name.trim().is_empty()
        || manifest.category.trim().is_empty()
    {
        return Err(DiscoverError::InvalidManifest);
    }
    if !is_sha256_digest(&manifest.package_digest) {
        return Err(DiscoverError::InvalidDigest);
    }
    if manifest.auth_recipes.len() > CONNECTOR_MAX_AUTH_RECIPES {
        return Err(DiscoverError::TooManyAuthRecipes);
    }
    if manifest.actions.len() > CONNECTOR_MAX_ACTIONS {
        return Err(DiscoverError::TooManyActions);
    }
    if manifest.triggers.len() > CONNECTOR_MAX_TRIGGERS {
        return Err(DiscoverError::TooManyTriggers);
    }
    let mut recipe_ids: Vec<&str> = Vec::with_capacity(manifest.auth_recipes.len());
    for recipe in &manifest.auth_recipes {
        if recipe.recipe_id.trim().is_empty() {
            return Err(DiscoverError::InvalidManifest);
        }
        if recipe_ids.iter().any(|id| *id == recipe.recipe_id.as_str()) {
            return Err(DiscoverError::DuplicateKey);
        }
        recipe_ids.push(recipe.recipe_id.as_str());
        if recipe.kind.requires_pkce() != recipe.pkce_required {
            return Err(DiscoverError::InconsistentAuthRecipe);
        }
        if recipe.client_secret_required && !recipe.kind.may_hold_client_secret() {
            return Err(DiscoverError::InconsistentAuthRecipe);
        }
        if recipe.kind == AuthRecipeKind::NoAuth
            && (!recipe.default_scopes.is_empty() || !recipe.optional_scopes.is_empty())
        {
            return Err(DiscoverError::InconsistentAuthRecipe);
        }
    }
    let mut action_keys: Vec<&str> = Vec::with_capacity(manifest.actions.len());
    for action in &manifest.actions {
        if action.action_key.trim().is_empty() || action.title.trim().is_empty() {
            return Err(DiscoverError::InvalidManifest);
        }
        if action_keys.iter().any(|key| *key == action.action_key.as_str()) {
            return Err(DiscoverError::DuplicateKey);
        }
        action_keys.push(action.action_key.as_str());
    }
    let mut trigger_keys: Vec<&str> = Vec::with_capacity(manifest.triggers.len());
    for trigger in &manifest.triggers {
        if trigger.trigger_key.trim().is_empty() || trigger.title.trim().is_empty() {
            return Err(DiscoverError::InvalidManifest);
        }
        if trigger_keys.iter().any(|key| *key == trigger.trigger_key.as_str()) {
            return Err(DiscoverError::DuplicateKey);
        }
        trigger_keys.push(trigger.trigger_key.as_str());
    }
    Ok(())
}

/// Build the bounded catalog entry for one validated connector package.
///
/// The entry never claims installation, authentication, or authority. Its
/// availability still depends on the observed install/auth state, so a package
/// that is merely *known* is never reported as usable.
pub fn connector_catalog_entry(
    manifest: &ConnectorPackageManifest,
    compatibility: DiscoverCompatibility,
    install_state: DiscoverInstallState,
    auth_state: DiscoverAuthState,
    provider_qualified: bool,
) -> Result<DiscoverCatalogEntry, DiscoverError> {
    validate_connector_package_manifest(manifest)?;
    let (availability, availability_reason) =
        derive_discover_availability(provider_qualified, compatibility, install_state, auth_state);
    let mut grants_required: Vec<String> = Vec::new();
    for recipe in &manifest.auth_recipes {
        for scope in &recipe.default_scopes {
            if !grants_required.iter().any(|value| value == scope) {
                grants_required.push(scope.clone());
            }
        }
    }
    let mut entry = DiscoverCatalogEntry::new(
        format!("connector:{}", manifest.connector_id),
        DiscoverEntryKind::App,
        manifest.display_name.clone(),
        format!("{} connector", manifest.category),
        DiscoverEntrySource::LiteSpmPackage,
    );
    entry.description = Some(manifest.connector_id.clone());
    entry.compatibility = compatibility;
    entry.compatibility_reason = None;
    entry.availability = availability;
    entry.availability_reason = availability_reason;
    entry.install_state = install_state;
    entry.auth_state = auth_state;
    entry.publisher = Some(manifest.publisher.clone());
    entry.version = Some(manifest.package_version.clone());
    entry.digest = Some(manifest.package_digest.clone());
    entry.grants_required = grants_required;
    entry.validate()?;
    Ok(entry)
}

/// The bounded, non-secret summary of a provider tool schema.
///
/// Discover never ships every tool schema to the UI. A summary carries only the
/// operation name and class, so a catalog with thousands of entries stays small.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DiscoverCapabilitySummary {
    pub capability_id: String,
    pub display_name: String,
    pub operation_class: ConnectorOperationClass,
    pub required_scopes: Vec<String>,
}

/// Reduce a connector descriptor to the bounded summary Discover publishes.
pub fn connector_capability_summaries(
    manifest: &ConnectorPackageManifest,
) -> Vec<DiscoverCapabilitySummary> {
    manifest
        .actions
        .iter()
        .map(|action| DiscoverCapabilitySummary {
            capability_id: format!("{}:{}", manifest.connector_id, action.action_key),
            display_name: action.title.clone(),
            operation_class: action.operation_class,
            required_scopes: action.required_scopes.clone(),
        })
        .collect()
}

/// The bounded, non-secret detail of one catalog entry.
///
/// The detail never contains a tool schema, a provider handle, or a credential.
/// There is deliberately no field for a raw provider payload: the projection
/// never retains provider bytes it cannot bound.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DiscoverCatalogEntryDetail {
    pub entry: DiscoverCatalogEntry,
    /// Bounded operation summaries. Empty when the provider is not qualified.
    pub capabilities: Vec<DiscoverCapabilitySummary>,
    /// The provider's own error or limitation text, if any.
    pub provider_note: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest() -> ConnectorPackageManifest {
        ConnectorPackageManifest {
            package_id: "pkg-1".to_owned(),
            package_version: "2026.10.1".to_owned(),
            publisher: "acme".to_owned(),
            package_digest: format!("sha256:{}", "a".repeat(64)),
            license_spdx: Some("MIT".to_owned()),
            source_url: None,
            connector_id: "acme-invoices".to_owned(),
            display_name: "Acme Invoices".to_owned(),
            category: "finance".to_owned(),
            auth_recipes: vec![AuthRecipeDescriptor {
                recipe_id: "recipe-1".to_owned(),
                kind: AuthRecipeKind::PublicOauthPkceLoopback,
                client_secret_required: false,
                pkce_required: true,
                redirect_policy: RedirectPolicy::LoopbackRandomPort,
                default_scopes: vec!["invoices.read".to_owned()],
                optional_scopes: vec!["invoices.write".to_owned()],
                scope_risk: ScopeRisk::Read,
                supports_refresh: true,
                provider_constraints: Vec::new(),
            }],
            actions: vec![ConnectorActionDescriptor {
                action_key: "list".to_owned(),
                title: "List invoices".to_owned(),
                description: "List invoices".to_owned(),
                operation_class: ConnectorOperationClass::Read,
                idempotency: ConnectorIdempotency::ReadOnly,
                required_scopes: vec!["invoices.read".to_owned()],
            }],
            triggers: Vec::new(),
        }
    }

    #[test]
    fn a_well_formed_connector_descriptor_is_accepted() {
        assert_eq!(validate_connector_package_manifest(&manifest()), Ok(()));
    }

    #[test]
    fn a_descriptor_with_a_malformed_package_digest_is_rejected() {
        let mut manifest = manifest();
        manifest.package_digest = "not-a-digest".to_owned();
        assert_eq!(
            validate_connector_package_manifest(&manifest),
            Err(DiscoverError::InvalidDigest)
        );
    }

    #[test]
    fn a_descriptor_with_duplicate_action_keys_is_rejected() {
        let mut manifest = manifest();
        manifest.actions.push(manifest.actions[0].clone());
        assert_eq!(
            validate_connector_package_manifest(&manifest),
            Err(DiscoverError::DuplicateKey)
        );
    }

    #[test]
    fn a_public_oauth_recipe_without_pkce_is_rejected() {
        let mut manifest = manifest();
        manifest.auth_recipes[0].pkce_required = false;
        assert_eq!(
            validate_connector_package_manifest(&manifest),
            Err(DiscoverError::InconsistentAuthRecipe)
        );
    }

    #[test]
    fn a_recipe_that_requires_a_client_secret_it_cannot_hold_is_rejected() {
        let mut manifest = manifest();
        manifest.auth_recipes[0].client_secret_required = true;
        assert_eq!(
            validate_connector_package_manifest(&manifest),
            Err(DiscoverError::InconsistentAuthRecipe)
        );
    }

    #[test]
    fn a_declared_read_action_does_not_downgrade_a_write_action() {
        assert!(!ConnectorOperationClass::Read.is_consequential());
        assert!(ConnectorOperationClass::Write.is_consequential());
        assert!(ConnectorOperationClass::PurchaseLike.is_consequential());
        assert!(ConnectorOperationClass::Admin.is_consequential());
    }

    #[test]
    fn an_unqualified_package_service_is_reported_as_not_qualified() {
        let entry = connector_catalog_entry(
            &manifest(),
            DiscoverCompatibility::Compatible,
            DiscoverInstallState::Installed,
            DiscoverAuthState::Authenticated,
            false,
        )
        .expect("a validated descriptor produces an entry");
        assert_eq!(entry.availability, DiscoverAvailability::NotQualified);
        assert_eq!(entry.availability_reason, PACKAGE_SERVICE_UNAVAILABLE);
        assert!(entry.is_disabled());
    }

    #[test]
    fn an_incompatible_entry_is_unsupported_and_disabled() {
        let entry = connector_catalog_entry(
            &manifest(),
            DiscoverCompatibility::Incompatible,
            DiscoverInstallState::Installed,
            DiscoverAuthState::Authenticated,
            true,
        )
        .expect("a validated descriptor produces an entry");
        assert_eq!(entry.availability, DiscoverAvailability::Unsupported);
        assert!(entry.is_disabled());
    }

    #[test]
    fn an_entry_that_needs_authentication_reports_needs_auth() {
        let entry = connector_catalog_entry(
            &manifest(),
            DiscoverCompatibility::Compatible,
            DiscoverInstallState::Installed,
            DiscoverAuthState::Required,
            true,
        )
        .expect("a validated descriptor produces an entry");
        assert_eq!(entry.availability, DiscoverAvailability::NeedsAuth);
        assert!(entry.requires_user_action());
        assert!(entry.is_disabled());
    }

    #[test]
    fn an_entry_with_an_unknown_install_state_is_an_error_not_available() {
        let entry = connector_catalog_entry(
            &manifest(),
            DiscoverCompatibility::Compatible,
            DiscoverInstallState::Unknown,
            DiscoverAuthState::Authenticated,
            true,
        )
        .expect("a validated descriptor produces an entry");
        assert_eq!(entry.availability, DiscoverAvailability::Error);
    }

    #[test]
    fn a_catalog_entry_never_claims_authority() {
        let entry = connector_catalog_entry(
            &manifest(),
            DiscoverCompatibility::Compatible,
            DiscoverInstallState::Installed,
            DiscoverAuthState::Authenticated,
            true,
        )
        .expect("a validated descriptor produces an entry");
        assert!(!entry.authoritative);
        assert_eq!(entry.availability, DiscoverAvailability::Available);
        assert!(!entry.requires_user_action());
    }

    #[test]
    fn an_entry_that_claims_authority_is_rejected() {
        let mut entry = DiscoverCatalogEntry::new(
            "connector:acme-invoices".to_owned(),
            DiscoverEntryKind::App,
            "Acme Invoices".to_owned(),
            "finance connector".to_owned(),
            DiscoverEntrySource::LiteSpmPackage,
        );
        entry.authoritative = true;
        assert_eq!(
            entry.validate(),
            Err(DiscoverError::EntryClaimsAuthority)
        );
    }

    #[test]
    fn discover_publishes_only_bounded_operation_summaries() {
        let summaries = connector_capability_summaries(&manifest());
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].capability_id, "acme-invoices:list");
        assert_eq!(summaries[0].operation_class, ConnectorOperationClass::Read);
        assert!(summaries[0].required_scopes.contains(&"invoices.read".to_owned()));
    }

    #[test]
    fn a_package_entry_reports_the_lite_spm_package_source() {
        assert_eq!(
            DiscoverEntrySource::LiteSpmPackage.as_str(),
            "LITE_SPM_PACKAGE"
        );
        assert_eq!(DiscoverEntrySource::McpServer.as_str(), "MCP_SERVER");
        assert_eq!(
            DiscoverEntrySource::LiteCoworkLocal.as_str(),
            "LITECOWORK_LOCAL"
        );
    }

    #[test]
    fn a_catalog_detail_carries_no_raw_provider_payload() {
        let detail = DiscoverCatalogEntryDetail {
            entry: DiscoverCatalogEntry::new(
                "skill:skill-1".to_owned(),
                DiscoverEntryKind::Skill,
                "Invoice cleanup".to_owned(),
                "Clean up an invoice".to_owned(),
                DiscoverEntrySource::LiteCoworkLocal,
            ),
            capabilities: Vec::new(),
            provider_note: None,
        };
        let encoded = serde_json::to_string(&detail).expect("the detail encodes");
        assert!(!encoded.contains("raw_provider_payload"));
        assert!(!encoded.contains("provider_payload"));
    }
}
