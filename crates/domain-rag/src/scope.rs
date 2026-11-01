//! Core-owned retrieval scope.
//!
//! The scope is decided by LiteCowork Core after Workspace ownership, ContextDocument
//! status, and Resource revision checks. A retrieval provider receives an already
//! authorized scope and may narrow it, but it can never widen it. Query text is
//! untrusted input and never expands the scope.

use serde::{Deserialize, Serialize};

/// One Resource the caller is authorized to read, optionally pinned to an exact
/// immutable revision and content digest.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct AuthorizedResourceRef {
    pub resource_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_revision_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_digest: Option<String>,
}

impl AuthorizedResourceRef {
    pub fn validate(&self) -> Result<(), String> {
        if self.resource_id.trim().is_empty() || self.resource_id.len() > 200 {
            return Err("authorized resource id is invalid".to_owned());
        }
        if let Some(revision_id) = &self.resource_revision_id {
            if revision_id.trim().is_empty() || revision_id.len() > 200 {
                return Err("authorized resource revision id is invalid".to_owned());
            }
        }
        if let Some(digest) = &self.content_digest {
            if !storage_core::is_sha256_digest(digest) {
                return Err("authorized resource content digest is invalid".to_owned());
            }
        }
        Ok(())
    }
}

/// The authorized boundary of one retrieval request.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RetrievalScope {
    /// Every ACTIVE Resource of the caller's Workspace. Core constructs this variant
    /// only after it has authorized the caller for the selected Workspace.
    Workspace,
    /// Only the listed Resources. A pin restricts the request to that exact revision.
    Resources(Vec<AuthorizedResourceRef>),
}

impl RetrievalScope {
    pub fn validate(&self) -> Result<(), String> {
        match self {
            Self::Workspace => Ok(()),
            Self::Resources(resources) => {
                if resources.len() > 200 {
                    return Err("authorized resource scope is too large".to_owned());
                }
                for resource in resources {
                    resource.validate()?;
                }
                Ok(())
            }
        }
    }

    /// Returns true when the exact revision may be used for this scope. A pinned
    /// revision is the only revision that is allowed for that Resource.
    pub fn allows(&self, resource_id: &str, resource_revision_id: &str) -> bool {
        match self {
            Self::Workspace => true,
            Self::Resources(resources) => resources.iter().any(|resource| {
                resource.resource_id == resource_id
                    && match &resource.resource_revision_id {
                        Some(pinned) => pinned == resource_revision_id,
                        None => true,
                    }
            })
        }
    }

    /// True when any listed Resource is pinned to an exact revision.
    pub fn has_revision_pins(&self) -> bool {
        match self {
            Self::Workspace => false,
            Self::Resources(resources) => resources
                .iter()
                .any(|resource| resource.resource_revision_id.is_some()),
        }
    }

    /// The exact pinned revision for a Resource, when the scope pins one.
    pub fn pinned_revision(&self, resource_id: &str) -> Option<&str> {
        match self {
            Self::Workspace => None,
            Self::Resources(resources) => resources
                .iter()
                .find(|resource| resource.resource_id == resource_id)
                .and_then(|resource| resource.resource_revision_id.as_deref()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AuthorizedResourceRef, RetrievalScope};

    fn pinned(resource_id: &str, revision_id: &str) -> AuthorizedResourceRef {
        AuthorizedResourceRef {
            resource_id: resource_id.to_owned(),
            resource_revision_id: Some(revision_id.to_owned()),
            content_digest: None,
        }
    }

    #[test]
    fn workspace_scope_allows_every_resource_in_the_authorized_workspace() {
        let scope = RetrievalScope::Workspace;
        assert!(scope.allows("resource-a", "revision-a"));
        assert!(!scope.has_revision_pins());
    }

    #[test]
    fn an_unlisted_resource_is_never_allowed() {
        let scope = RetrievalScope::Resources(vec![pinned("resource-a", "revision-a")]);
        assert!(scope.allows("resource-a", "revision-a"));
        assert!(!scope.allows("resource-a", "revision-b"));
        assert!(!scope.allows("resource-b", "revision-a"));
    }

    #[test]
    fn an_unpinned_listed_resource_allows_any_authorized_revision() {
        let scope = RetrievalScope::Resources(vec![AuthorizedResourceRef {
            resource_id: "resource-a".to_owned(),
            resource_revision_id: None,
            content_digest: None,
        }]);
        assert!(scope.allows("resource-a", "revision-a"));
        assert!(scope.allows("resource-a", "revision-b"));
        assert!(!scope.has_revision_pins());
    }

    #[test]
    fn invalid_scope_entries_are_rejected() {
        let scope = RetrievalScope::Resources(vec![AuthorizedResourceRef {
            resource_id: " ".to_owned(),
            resource_revision_id: None,
            content_digest: None,
        }]);
        assert!(scope.validate().is_err());

        let scope = RetrievalScope::Resources(vec![AuthorizedResourceRef {
            resource_id: "resource-a".to_owned(),
            resource_revision_id: None,
            content_digest: Some("sha256:not-a-digest".to_owned()),
        }]);
        assert!(scope.validate().is_err());
    }
}
