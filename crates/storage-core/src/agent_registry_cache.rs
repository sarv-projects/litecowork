use crate::StoreError;
use serde_json::Value;
use std::io::{self, Write};

pub const AGENT_REGISTRY_CACHE_MAX_ENTRIES: usize = 512;
pub const AGENT_REGISTRY_CACHE_MAX_SNAPSHOT_BYTES: usize = 2 * 1024 * 1024;

/// Sanitized descriptive metadata projected from an untrusted ACP Registry.
/// It is discovery cache only and never grants installation or execution authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentRegistryCacheEntry {
    pub registry_agent_id: String,
    pub metadata: Value,
}

/// One source's last completely validated registry snapshot. Expired snapshots remain
/// readable so callers can render them as stale during an outage.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentRegistryCacheSnapshot {
    pub source_registry: String,
    pub source_revision: Option<String>,
    pub observed_at: String,
    pub expires_at: Option<String>,
    pub entries: Vec<AgentRegistryCacheEntry>,
}

pub trait AgentRegistryCacheStore: Send + Sync {
    fn read_agent_registry_snapshot(
        &self,
        source_registry: &str,
    ) -> Result<Option<AgentRegistryCacheSnapshot>, StoreError>;

    /// Atomically replace one source's cache after the whole snapshot has been validated.
    fn replace_agent_registry_snapshot(
        &self,
        snapshot: AgentRegistryCacheSnapshot,
    ) -> Result<(), StoreError>;
}

/// Validate size and identity constraints before a provider's snapshot can enter storage.
/// Provider-specific schema validation and metadata sanitization remain in AgentRegistryService.
pub fn validate_agent_registry_cache_snapshot(
    snapshot: &AgentRegistryCacheSnapshot,
) -> Result<(), String> {
    let bounded = |value: &str, max: usize| {
        !value.is_empty() && value.len() <= max && !value.chars().any(char::is_control)
    };
    if !bounded(&snapshot.source_registry, 256)
        || snapshot
            .source_revision
            .as_deref()
            .is_some_and(|revision| revision.len() > 256 || revision.chars().any(char::is_control))
        || !bounded(&snapshot.observed_at, 64)
        || snapshot
            .expires_at
            .as_deref()
            .is_some_and(|timestamp| !bounded(timestamp, 64))
        || snapshot.entries.len() > AGENT_REGISTRY_CACHE_MAX_ENTRIES
    {
        return Err("snapshot metadata is outside the bounded cache contract".to_owned());
    }

    let mut seen = std::collections::BTreeSet::new();
    let mut bytes = 0usize;
    for entry in &snapshot.entries {
        if !bounded(&entry.registry_agent_id, 128)
            || !entry.registry_agent_id.is_ascii()
            || !entry.metadata.is_object()
            || !seen.insert(&entry.registry_agent_id)
        {
            return Err("entry identity or metadata is invalid".to_owned());
        }
        let remaining = AGENT_REGISTRY_CACHE_MAX_SNAPSHOT_BYTES.saturating_sub(bytes);
        let mut counter = BoundedCounter { written: 0, remaining };
        serde_json::to_writer(&mut counter, &entry.metadata)
            .map_err(|_| "snapshot exceeds the bounded cache size".to_owned())?;
        bytes = bytes
            .checked_add(counter.written)
            .ok_or_else(|| "snapshot size overflow".to_owned())?;
        if bytes > AGENT_REGISTRY_CACHE_MAX_SNAPSHOT_BYTES {
            return Err("snapshot exceeds the bounded cache size".to_owned());
        }
    }
    Ok(())
}

struct BoundedCounter {
    written: usize,
    remaining: usize,
}

impl Write for BoundedCounter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if buffer.len() > self.remaining.saturating_sub(self.written) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "bounded registry snapshot size exceeded",
            ));
        }
        self.written += buffer.len();
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn snapshot(entries: Vec<AgentRegistryCacheEntry>) -> AgentRegistryCacheSnapshot {
        AgentRegistryCacheSnapshot {
            source_registry: "https://registry.example.test/index.json".to_owned(),
            source_revision: Some("r1".to_owned()),
            observed_at: "2026-10-19T10:00:00Z".to_owned(),
            expires_at: Some("2026-10-19T10:15:00Z".to_owned()),
            entries,
        }
    }

    #[test]
    fn registry_cache_requires_unique_bounded_ids_and_object_metadata() {
        let valid = AgentRegistryCacheEntry {
            registry_agent_id: "agent-one".to_owned(),
            metadata: json!({"name":"Agent One"}),
        };
        assert!(validate_agent_registry_cache_snapshot(&snapshot(vec![valid.clone()])).is_ok());

        let mut duplicate = valid.clone();
        duplicate.metadata = json!({"name":"other"});
        assert!(
            validate_agent_registry_cache_snapshot(&snapshot(vec![valid.clone(), duplicate]))
                .is_err()
        );

        let mut non_object = valid;
        non_object.metadata = json!(["not", "an", "object"]);
        assert!(validate_agent_registry_cache_snapshot(&snapshot(vec![non_object])).is_err());
    }

    #[test]
    fn registry_cache_snapshot_has_a_hard_byte_and_entry_bound() {
        let huge = AgentRegistryCacheEntry {
            registry_agent_id: "agent-one".to_owned(),
            metadata: json!({"description":"x".repeat(AGENT_REGISTRY_CACHE_MAX_SNAPSHOT_BYTES + 1)}),
        };
        assert!(validate_agent_registry_cache_snapshot(&snapshot(vec![huge])).is_err());

        let many = (0..=AGENT_REGISTRY_CACHE_MAX_ENTRIES)
            .map(|index| AgentRegistryCacheEntry {
                registry_agent_id: format!("agent-{index}"),
                metadata: json!({}),
            })
            .collect();
        assert!(validate_agent_registry_cache_snapshot(&snapshot(many)).is_err());
    }
}
