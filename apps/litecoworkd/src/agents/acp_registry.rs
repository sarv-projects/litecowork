use serde::Serialize;
use std::collections::BTreeSet;

const MAX_DOCUMENT_BYTES: usize = 2 * 1024 * 1024;
const MAX_AGENT_COUNT: usize = 512;
const MAX_DISTRIBUTION_COUNT: usize = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RegistryError {
    TooLarge,
    InvalidDocument,
    DuplicateAgent,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ParsedRegistry {
    pub source_revision: String,
    pub items: Vec<RegistryAgent>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct RegistryAgent {
    pub registry_agent_id: String,
    pub display_name: String,
    pub description: Option<String>,
    pub homepage: Option<String>,
    pub distributions: Vec<RegistryDistribution>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct RegistryDistribution {
    pub distribution_id: String,
    pub kind: &'static str,
    pub package_ref: String,
    pub version_constraint: Option<String>,
    pub os: Vec<&'static str>,
    pub architecture: Vec<&'static str>,
}

/// Parse the pinned ACP Registry document format into descriptive metadata.
/// Executable commands, arguments, and environment maps are intentionally ignored.
pub(crate) fn parse_registry(input: &[u8]) -> Result<ParsedRegistry, RegistryError> {
    if input.len() > MAX_DOCUMENT_BYTES {
        return Err(RegistryError::TooLarge);
    }
    let document: serde_json::Value =
        serde_json::from_slice(input).map_err(|_| RegistryError::InvalidDocument)?;
    let root = document.as_object().ok_or(RegistryError::InvalidDocument)?;
    let source_revision = bounded_text(root.get("version"), 256)
        .filter(|value| !value.is_empty())
        .ok_or(RegistryError::InvalidDocument)?;
    let agents = root
        .get("agents")
        .and_then(serde_json::Value::as_array)
        .ok_or(RegistryError::InvalidDocument)?;
    if agents.len() > MAX_AGENT_COUNT {
        return Err(RegistryError::InvalidDocument);
    }

    let mut seen = BTreeSet::new();
    let mut items = Vec::with_capacity(agents.len());
    for value in agents {
        let agent = value.as_object().ok_or(RegistryError::InvalidDocument)?;
        let registry_agent_id = bounded_text(agent.get("id"), 128)
            .filter(|id| valid_agent_id(id))
            .ok_or(RegistryError::InvalidDocument)?;
        if !seen.insert(registry_agent_id.clone()) {
            return Err(RegistryError::DuplicateAgent);
        }
        let display_name = bounded_text(agent.get("name"), 128)
            .filter(|name| !name.trim().is_empty())
            .ok_or(RegistryError::InvalidDocument)?;
        let stable_version = bounded_text(agent.get("version"), 128)
            .filter(|version| valid_version(version))
            .ok_or(RegistryError::InvalidDocument)?;
        if registry_agent_id != "dimcode"
            && !bounded_text(agent.get("license_url"), 2048)
                .is_some_and(|uri| valid_https_uri(&uri))
        {
            return Err(RegistryError::InvalidDocument);
        }
        let description = optional_text(agent.get("description"), 1024);
        let homepage = ["website", "repository"]
            .into_iter()
            .filter_map(|key| bounded_text(agent.get(key), 2048))
            .find(|uri| valid_https_uri(uri));
        let distribution = agent
            .get("distribution")
            .and_then(serde_json::Value::as_object)
            .ok_or(RegistryError::InvalidDocument)?;
        if distribution
            .keys()
            .any(|key| !matches!(key.as_str(), "binary" | "npx" | "uvx"))
        {
            return Err(RegistryError::InvalidDocument);
        }
        let mut distributions = Vec::new();
        project_package_distribution(
            distribution.get("npx"),
            "NPM",
            "npx",
            &stable_version,
            &mut distributions,
        );
        project_package_distribution(
            distribution.get("uvx"),
            "PYPI",
            "uvx",
            &stable_version,
            &mut distributions,
        );
        project_binary_distribution(distribution.get("binary"), &mut distributions);
        if distributions.is_empty() || distributions.len() > MAX_DISTRIBUTION_COUNT {
            return Err(RegistryError::InvalidDocument);
        }
        items.push(RegistryAgent {
            registry_agent_id,
            display_name,
            description,
            homepage,
            distributions,
        });
    }
    Ok(ParsedRegistry {
        source_revision,
        items,
    })
}

fn project_package_distribution(
    value: Option<&serde_json::Value>,
    kind: &'static str,
    id_prefix: &str,
    version: &str,
    output: &mut Vec<RegistryDistribution>,
) {
    let Some(package) = value
        .and_then(serde_json::Value::as_object)
        .and_then(|object| bounded_text(object.get("package"), 512))
        .filter(|package| valid_package_ref(package))
    else {
        return;
    };
    output.push(RegistryDistribution {
        distribution_id: format!("{id_prefix}:{version}"),
        kind,
        package_ref: package,
        version_constraint: Some(version.to_owned()),
        os: vec!["WINDOWS", "LINUX", "MACOS"],
        architecture: vec!["X86_64", "AARCH64"],
    });
}

fn project_binary_distribution(
    value: Option<&serde_json::Value>,
    output: &mut Vec<RegistryDistribution>,
) {
    let Some(targets) = value.and_then(serde_json::Value::as_object) else {
        return;
    };
    for (target, metadata) in targets {
        let Some((os, architecture)) = target_platform(target) else {
            continue;
        };
        let Some(archive) = metadata
            .as_object()
            .and_then(|object| bounded_text(object.get("archive"), 512))
            .filter(|archive| valid_https_uri(archive))
        else {
            continue;
        };
        output.push(RegistryDistribution {
            distribution_id: format!("binary:{target}"),
            kind: "BINARY",
            package_ref: archive,
            version_constraint: None,
            os: vec![os],
            architecture: vec![architecture],
        });
        if output.len() > MAX_DISTRIBUTION_COUNT {
            return;
        }
    }
}

fn target_platform(value: &str) -> Option<(&'static str, &'static str)> {
    match value {
        "windows-x86_64" => Some(("WINDOWS", "X86_64")),
        "windows-aarch64" => Some(("WINDOWS", "AARCH64")),
        "linux-x86_64" => Some(("LINUX", "X86_64")),
        "linux-aarch64" => Some(("LINUX", "AARCH64")),
        "darwin-x86_64" => Some(("MACOS", "X86_64")),
        "darwin-aarch64" => Some(("MACOS", "AARCH64")),
        _ => None,
    }
}

fn bounded_text(value: Option<&serde_json::Value>, max_chars: usize) -> Option<String> {
    let text = value?.as_str()?;
    if text.chars().count() > max_chars || text.chars().any(char::is_control) {
        return None;
    }
    Some(text.to_owned())
}

fn optional_text(value: Option<&serde_json::Value>, max_chars: usize) -> Option<String> {
    bounded_text(value, max_chars).filter(|text| !text.trim().is_empty())
}

fn valid_agent_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.as_bytes()[0].is_ascii_lowercase()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn valid_version(value: &str) -> bool {
    let mut parts = value.split('.');
    let valid_number = |part: Option<&str>| {
        part.is_some_and(|number| {
            !number.is_empty()
                && number.bytes().all(|byte| byte.is_ascii_digit())
                && (number == "0" || !number.starts_with('0'))
        })
    };
    valid_number(parts.next())
        && valid_number(parts.next())
        && valid_number(parts.next())
        && parts.next().is_none()
}

fn valid_package_ref(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= 512
        && !value
            .bytes()
            .any(|byte| byte.is_ascii_control() || byte.is_ascii_whitespace())
}

fn valid_https_uri(value: &str) -> bool {
    if value
        .bytes()
        .any(|byte| byte.is_ascii_whitespace() || byte.is_ascii_control() || byte == b'\\')
    {
        return false;
    }
    let Some(authority) = value.strip_prefix("https://") else {
        return false;
    };
    let authority = authority.split(['/', '?', '#']).next().unwrap_or_default();
    !authority.is_empty()
        && !authority.contains('@')
        && authority.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b':' | b'[' | b']')
        })
}

#[cfg(test)]
mod tests {
    use super::{RegistryError, parse_registry};

    #[test]
    fn projects_only_safe_registry_metadata() {
        let input = br#"{"version":"1.0.0","agents":[{"id":"codex-acp","name":"Codex","version":"1.2.3","description":"ACP bridge","license_url":"https://example.test/license","distribution":{"npx":{"package":"@agentclientprotocol/codex-acp@1.2.3","args":["--unsafe"],"env":{"TOKEN":"secret"}}}}]}"#;
        let snapshot = parse_registry(input).expect("valid bounded snapshot");
        assert_eq!(snapshot.source_revision, "1.0.0");
        assert_eq!(snapshot.items.len(), 1);
        assert_eq!(snapshot.items[0].registry_agent_id, "codex-acp");
        assert_eq!(snapshot.items[0].display_name, "Codex");
        assert_eq!(snapshot.items[0].distributions.len(), 1);
        assert_eq!(
            snapshot.items[0].distributions[0].package_ref,
            "@agentclientprotocol/codex-acp@1.2.3"
        );
        let projected = serde_json::to_string(&snapshot.items[0]).unwrap();
        assert!(!projected.contains("--unsafe"));
        assert!(!projected.contains("TOKEN"));
        assert!(!projected.contains("secret"));
    }

    #[test]
    fn rejects_duplicate_ids_and_oversized_documents() {
        let duplicate = br#"{"version":"1.0.0","agents":[{"id":"agent","name":"A","version":"1.0.0","description":"A","license_url":"https://example.test/license","distribution":{"npx":{"package":"agent"}}},{"id":"agent","name":"B","version":"1.0.0","description":"B","license_url":"https://example.test/license","distribution":{"npx":{"package":"agent-b"}}}]}"#;
        assert_eq!(
            parse_registry(duplicate),
            Err(RegistryError::DuplicateAgent)
        );
        assert_eq!(
            parse_registry(&vec![b' '; 2 * 1024 * 1024 + 1]),
            Err(RegistryError::TooLarge)
        );
    }

    #[test]
    fn rejects_malformed_entries_without_a_supported_distribution() {
        let no_license = br#"{"version":"1.0.0","agents":[{"id":"agent","name":"A","version":"1.0.0","description":"A","distribution":{"npx":{"package":"agent"}}}]}"#;
        assert_eq!(
            parse_registry(no_license),
            Err(RegistryError::InvalidDocument)
        );

        let command_only = br#"{"version":"1.0.0","agents":[{"id":"agent","name":"A","version":"1.0.0","description":"A","license_url":"https://example.test/license","distribution":{"binary":{"linux-x86_64":{"archive":"https://example.test/agent.zip","cmd":"evil-command","args":["--run"],"env":{"TOKEN":"secret"}}}}}]}"#;
        let parsed = parse_registry(command_only).expect("safe archive description is retained");
        assert_eq!(
            parsed.items[0].distributions[0].package_ref,
            "https://example.test/agent.zip"
        );
        let projection = serde_json::to_string(&parsed.items[0]).unwrap();
        assert!(!projection.contains("evil-command"));
        assert!(!projection.contains("--run"));
        assert!(!projection.contains("TOKEN"));
        assert!(!projection.contains("secret"));

        let malformed_target = br#"{"version":"1.0.0","agents":[{"id":"agent","name":"A","version":"1.0.0","description":"A","license_url":"https://example.test/license","distribution":{"binary":{"linux-x86_64":{"archive":"https://user@example.test/a.zip","cmd":"agent"}}}}]}"#;
        assert_eq!(
            parse_registry(malformed_target),
            Err(RegistryError::InvalidDocument)
        );

        let whitespace_url = br#"{"version":"1.0.0","agents":[{"id":"agent","name":"A","version":"1.0.0","description":"A","license_url":"https://example.test/license","distribution":{"binary":{"linux-x86_64":{"archive":"https://example.test/a zip.zip","cmd":"agent"}}}}]}"#;
        assert_eq!(
            parse_registry(whitespace_url),
            Err(RegistryError::InvalidDocument)
        );

        let unknown_distribution = br#"{"version":"1.0.0","agents":[{"id":"agent","name":"A","version":"1.0.0","description":"A","license_url":"https://example.test/license","distribution":{"shell":{"package":"agent"}}}]}"#;
        assert_eq!(
            parse_registry(unknown_distribution),
            Err(RegistryError::InvalidDocument)
        );
    }

    #[test]
    fn rejects_archive_references_over_the_operator_contract_limit() {
        let archive = format!("https://example.test/{}", "a".repeat(512));
        let input = serde_json::json!({
            "version": "1.0.0",
            "agents": [{
                "id": "agent",
                "name": "A",
                "version": "1.0.0",
                "license_url": "https://example.test/license",
                "distribution": {
                    "binary": { "linux-x86_64": { "archive": archive } }
                }
            }]
        });

        assert_eq!(
            parse_registry(&serde_json::to_vec(&input).unwrap()),
            Err(RegistryError::InvalidDocument)
        );
    }
}
