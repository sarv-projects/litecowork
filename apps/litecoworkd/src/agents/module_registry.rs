use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum OperatingSystem {
    Linux,
    Windows,
    Macos,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum OperatingArchitecture {
    X86_64,
    Aarch64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AgentModuleManifest {
    pub module_id: String,
    pub module_version: String,
    pub descriptor_api_version: u32,
    pub registry_agent_ids: Vec<String>,
    pub display_name: String,
    pub distribution_kinds: Vec<String>,
    pub supported_os: Vec<OperatingSystem>,
    pub supported_architectures: Vec<OperatingArchitecture>,
    pub lifecycle_features: AgentModuleLifecycleFeatures,
}

/// Trusted module-declared lifecycle actions. Registry distribution metadata and
/// installation observations never imply that a module implements one of these
/// operations.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct AgentModuleLifecycleFeatures {
    pub discover_installation: bool,
    pub install: bool,
    pub update: bool,
    pub uninstall: bool,
    pub native_auth: bool,
    pub secure_credential_slots: bool,
    pub open_native_config: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ModuleRegistryError {
    InvalidManifest,
    InvalidLookup,
    DuplicateModule,
    AmbiguousMatch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ModuleSupportAssessment {
    NoAdapter,
    NoCompatibleDescriptor,
    UnsupportedRuntime,
    Supported,
}

#[derive(Default)]
pub(crate) struct AgentModuleRegistry {
    modules_by_id: BTreeMap<String, AgentModuleManifest>,
}

impl AgentModuleRegistry {
    pub(crate) fn register(
        &mut self,
        manifest: AgentModuleManifest,
    ) -> Result<(), ModuleRegistryError> {
        validate_manifest(&manifest)?;
        if self.modules_by_id.contains_key(&manifest.module_id) {
            return Err(ModuleRegistryError::DuplicateModule);
        }
        self.modules_by_id
            .insert(manifest.module_id.clone(), manifest);
        Ok(())
    }

    pub(crate) fn match_registry_agent(
        &self,
        registry_agent_id: &str,
        os: OperatingSystem,
        architecture: OperatingArchitecture,
        host_descriptor_api_version: u32,
    ) -> Result<Option<&AgentModuleManifest>, ModuleRegistryError> {
        if !valid_registry_agent_id(registry_agent_id) {
            return Err(ModuleRegistryError::InvalidLookup);
        }
        if host_descriptor_api_version == 0 {
            return Ok(None);
        }
        let highest = self
            .modules_by_id
            .values()
            .filter(|manifest| {
                manifest
                    .registry_agent_ids
                    .iter()
                    .any(|id| id == registry_agent_id)
                    && manifest.supported_os.contains(&os)
                    && manifest.supported_architectures.contains(&architecture)
                    && manifest.descriptor_api_version <= host_descriptor_api_version
            })
            .map(|manifest| manifest.descriptor_api_version)
            .max();
        let Some(highest) = highest else {
            return Ok(None);
        };
        let mut candidates = self.modules_by_id.values().filter(|manifest| {
            manifest.descriptor_api_version == highest
                && manifest
                    .registry_agent_ids
                    .iter()
                    .any(|id| id == registry_agent_id)
                && manifest.supported_os.contains(&os)
                && manifest.supported_architectures.contains(&architecture)
        });
        let selected = candidates.next();
        if candidates.next().is_some() {
            return Err(ModuleRegistryError::AmbiguousMatch);
        }
        Ok(selected)
    }

    /// Describes module support without collapsing a missing adapter into a runtime
    /// incompatibility. A registry record alone never yields `Supported`.
    pub(crate) fn assess_registry_agent(
        &self,
        registry_agent_id: &str,
        os: OperatingSystem,
        architecture: OperatingArchitecture,
        host_descriptor_api_version: u32,
    ) -> Result<ModuleSupportAssessment, ModuleRegistryError> {
        if !valid_registry_agent_id(registry_agent_id) {
            return Err(ModuleRegistryError::InvalidLookup);
        }
        let claiming = self.modules_by_id.values().filter(|manifest| {
            manifest
                .registry_agent_ids
                .iter()
                .any(|id| id == registry_agent_id)
        });
        let mut found_claim = false;
        let mut found_compatible_descriptor = false;
        let mut found_runtime_target = false;
        for manifest in claiming {
            found_claim = true;
            if manifest.descriptor_api_version <= host_descriptor_api_version {
                found_compatible_descriptor = true;
                if manifest.supported_os.contains(&os)
                    && manifest.supported_architectures.contains(&architecture)
                {
                    found_runtime_target = true;
                }
            }
        }
        if !found_claim {
            return Ok(ModuleSupportAssessment::NoAdapter);
        }
        if !found_compatible_descriptor {
            return Ok(ModuleSupportAssessment::NoCompatibleDescriptor);
        }
        if !found_runtime_target {
            return Ok(ModuleSupportAssessment::UnsupportedRuntime);
        }
        self.match_registry_agent(
            registry_agent_id,
            os,
            architecture,
            host_descriptor_api_version,
        )?
        .map(|_| ModuleSupportAssessment::Supported)
        .ok_or(ModuleRegistryError::InvalidManifest)
    }

    pub(crate) fn get(&self, module_id: &str) -> Option<&AgentModuleManifest> {
        self.modules_by_id.get(module_id)
    }

    pub(crate) fn list_supported(&self) -> Vec<AgentModuleManifest> {
        self.modules_by_id.values().cloned().collect()
    }
}

fn validate_manifest(manifest: &AgentModuleManifest) -> Result<(), ModuleRegistryError> {
    let unique = |items: &[String]| {
        items
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            == items.len()
    };
    const DISTRIBUTIONS: &[&str] = &[
        "NPM", "PYPI", "BINARY", "HOMEBREW", "WINGET", "SCOOP", "CARGO", "MANUAL", "OTHER",
    ];
    if !valid_module_id(&manifest.module_id)
        || manifest.module_version.len() > 128
        || !valid_semver(&manifest.module_version)
        || manifest.descriptor_api_version == 0
        || manifest.registry_agent_ids.is_empty()
        || manifest.registry_agent_ids.len() > 64
        || !unique(&manifest.registry_agent_ids)
        || manifest
            .registry_agent_ids
            .iter()
            .any(|id| !valid_registry_agent_id(id))
        || manifest.display_name.trim().is_empty()
        || manifest.display_name.chars().count() > 128
        || manifest.display_name.chars().any(char::is_control)
        || manifest.distribution_kinds.len() > 16
        || !unique(&manifest.distribution_kinds)
        || manifest
            .distribution_kinds
            .iter()
            .any(|kind| !DISTRIBUTIONS.contains(&kind.as_str()))
        || manifest.supported_os.is_empty()
        || manifest.supported_os.len() > 3
        || manifest
            .supported_os
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != manifest.supported_os.len()
        || manifest.supported_architectures.is_empty()
        || manifest.supported_architectures.len() > 2
        || manifest
            .supported_architectures
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != manifest.supported_architectures.len()
    {
        return Err(ModuleRegistryError::InvalidManifest);
    }
    Ok(())
}

fn valid_module_id(value: &str) -> bool {
    valid_identifier(
        value,
        128,
        b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789._/-",
    ) && !value
        .split('/')
        .any(|segment| segment.is_empty() || matches!(segment, "." | ".."))
}

fn valid_registry_agent_id(value: &str) -> bool {
    valid_identifier(
        value,
        256,
        b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789._/@-",
    ) && !value
        .split('/')
        .any(|segment| segment.is_empty() || matches!(segment, "." | ".."))
}

fn valid_identifier(value: &str, max_len: usize, allowed: &[u8]) -> bool {
    !value.is_empty() && value.len() <= max_len && value.bytes().all(|byte| allowed.contains(&byte))
}

pub(super) fn valid_semver(value: &str) -> bool {
    let (without_build, build) = match value.split_once('+') {
        Some((left, right)) if !right.is_empty() && !right.contains('+') => (left, Some(right)),
        Some(_) => return false,
        None => (value, None),
    };
    let (core, prerelease) = match without_build.split_once('-') {
        Some((left, right)) if !right.is_empty() => (left, Some(right)),
        Some(_) => return false,
        None => (without_build, None),
    };
    if core.is_empty() {
        return false;
    }
    let segments: Vec<_> = core.split('.').collect();
    let valid_core = segments.len() == 3
        && segments.iter().all(|segment| {
            !segment.is_empty()
                && segment.bytes().all(|byte| byte.is_ascii_digit())
                && (segment == &"0" || !segment.starts_with('0'))
        });
    let valid_suffix = |suffix: &str, reject_numeric_leading_zero: bool| {
        !suffix.is_empty()
            && suffix.split('.').all(|identifier| {
                !identifier.is_empty()
                    && identifier
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
                    && !(reject_numeric_leading_zero
                        && identifier.bytes().all(|byte| byte.is_ascii_digit())
                        && identifier.len() > 1
                        && identifier.starts_with('0'))
            })
    };
    valid_core
        && prerelease.is_none_or(|suffix| valid_suffix(suffix, true))
        && build.is_none_or(|suffix| valid_suffix(suffix, false))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(module_id: &str, os: &[OperatingSystem], api: u32) -> AgentModuleManifest {
        AgentModuleManifest {
            module_id: module_id.to_owned(),
            module_version: "1.0.0".to_owned(),
            descriptor_api_version: api,
            registry_agent_ids: vec!["acme/agent".to_owned()],
            display_name: "Test Agent".to_owned(),
            distribution_kinds: vec!["NPM".to_owned()],
            supported_os: os.to_vec(),
            supported_architectures: vec![OperatingArchitecture::X86_64],
            lifecycle_features: AgentModuleLifecycleFeatures {
                discover_installation: true,
                install: false,
                update: false,
                uninstall: false,
                native_auth: false,
                secure_credential_slots: false,
                open_native_config: false,
            },
        }
    }

    #[test]
    fn module_manifest_never_infers_unsupported_lifecycle_operations() {
        let mut registry = AgentModuleRegistry::default();
        registry
            .register(manifest(
                "module/discovery-only",
                &[OperatingSystem::Linux],
                1,
            ))
            .unwrap();

        let selected = registry
            .match_registry_agent(
                "acme/agent",
                OperatingSystem::Linux,
                OperatingArchitecture::X86_64,
                1,
            )
            .unwrap()
            .expect("compatible module registered");
        assert!(selected.lifecycle_features.discover_installation);
        assert!(!selected.lifecycle_features.install);
        assert!(!selected.lifecycle_features.update);
        assert!(!selected.lifecycle_features.uninstall);
        assert!(!selected.lifecycle_features.native_auth);
        assert!(!selected.lifecycle_features.secure_credential_slots);
        assert!(!selected.lifecycle_features.open_native_config);
    }

    #[test]
    fn registry_selects_only_an_exactly_compatible_module() {
        let mut registry = AgentModuleRegistry::default();
        registry
            .register(manifest("module/linux-v1", &[OperatingSystem::Linux], 1))
            .unwrap();
        registry
            .register(manifest("module/linux-v2", &[OperatingSystem::Linux], 2))
            .unwrap();
        registry
            .register(manifest(
                "module/windows-v2",
                &[OperatingSystem::Windows],
                2,
            ))
            .unwrap();

        let selected = registry
            .match_registry_agent(
                "acme/agent",
                OperatingSystem::Linux,
                OperatingArchitecture::X86_64,
                2,
            )
            .unwrap();
        assert_eq!(
            selected.map(|manifest| manifest.module_id.as_str()),
            Some("module/linux-v2")
        );
        assert_eq!(
            registry
                .match_registry_agent(
                    "acme/other",
                    OperatingSystem::Linux,
                    OperatingArchitecture::X86_64,
                    2,
                )
                .unwrap(),
            None
        );
        assert_eq!(
            registry
                .match_registry_agent(
                    "acme/agent",
                    OperatingSystem::Macos,
                    OperatingArchitecture::X86_64,
                    2,
                )
                .unwrap(),
            None
        );
        assert_eq!(
            registry
                .match_registry_agent(
                    "acme/agent",
                    OperatingSystem::Linux,
                    OperatingArchitecture::X86_64,
                    3,
                )
                .unwrap()
                .map(|manifest| manifest.module_id.as_str()),
            Some("module/linux-v2")
        );
    }

    #[test]
    fn registry_never_matches_a_module_for_an_unsupported_architecture() {
        let mut registry = AgentModuleRegistry::default();
        registry
            .register(manifest("module/x86", &[OperatingSystem::Linux], 1))
            .unwrap();

        assert_eq!(
            registry
                .match_registry_agent(
                    "acme/agent",
                    OperatingSystem::Linux,
                    OperatingArchitecture::Aarch64,
                    1,
                )
                .unwrap(),
            None
        );
        assert_eq!(
            registry
                .match_registry_agent(
                    "acme/agent",
                    OperatingSystem::Linux,
                    OperatingArchitecture::X86_64,
                    1,
                )
                .unwrap()
                .map(|manifest| manifest.module_id.as_str()),
            Some("module/x86")
        );
    }

    #[test]
    fn registry_explains_runtime_incompatibility_separately_from_missing_adapter() {
        let mut registry = AgentModuleRegistry::default();
        registry
            .register(manifest("module/one", &[OperatingSystem::Linux], 2))
            .unwrap();

        assert_eq!(
            registry
                .assess_registry_agent(
                    "acme/agent",
                    OperatingSystem::Windows,
                    OperatingArchitecture::X86_64,
                    2,
                )
                .unwrap(),
            ModuleSupportAssessment::UnsupportedRuntime
        );
        assert_eq!(
            registry
                .assess_registry_agent(
                    "acme/other",
                    OperatingSystem::Windows,
                    OperatingArchitecture::X86_64,
                    2,
                )
                .unwrap(),
            ModuleSupportAssessment::NoAdapter
        );
        assert_eq!(
            registry
                .assess_registry_agent(
                    "acme/agent",
                    OperatingSystem::Linux,
                    OperatingArchitecture::X86_64,
                    1,
                )
                .unwrap(),
            ModuleSupportAssessment::NoCompatibleDescriptor
        );
        assert_eq!(
            registry
                .assess_registry_agent(
                    "acme/agent",
                    OperatingSystem::Linux,
                    OperatingArchitecture::X86_64,
                    2,
                )
                .unwrap(),
            ModuleSupportAssessment::Supported
        );
    }

    #[test]
    fn duplicate_claims_for_same_compatible_identity_fail_closed() {
        let mut registry = AgentModuleRegistry::default();
        registry
            .register(manifest("module/one", &[OperatingSystem::Linux], 1))
            .unwrap();
        registry
            .register(manifest("module/two", &[OperatingSystem::Linux], 1))
            .unwrap();
        assert_eq!(registry.get("module/one").unwrap().module_version, "1.0.0");
        assert_eq!(
            registry
                .list_supported()
                .iter()
                .map(|module| module.module_id.as_str())
                .collect::<Vec<_>>(),
            ["module/one", "module/two"]
        );

        assert_eq!(
            registry.match_registry_agent(
                "acme/agent",
                OperatingSystem::Linux,
                OperatingArchitecture::X86_64,
                1,
            ),
            Err(ModuleRegistryError::AmbiguousMatch),
        );
    }

    #[test]
    fn registration_rejects_duplicate_module_and_malformed_manifest() {
        let mut registry = AgentModuleRegistry::default();
        registry
            .register(manifest("module/one", &[OperatingSystem::Linux], 1))
            .unwrap();
        assert_eq!(
            registry.register(manifest("module/one", &[OperatingSystem::Linux], 2)),
            Err(ModuleRegistryError::DuplicateModule),
        );

        let mut invalid = manifest(" ", &[OperatingSystem::Linux], 1);
        assert_eq!(
            registry.register(invalid.clone()),
            Err(ModuleRegistryError::InvalidManifest)
        );
        invalid.module_id = "module/three".to_owned();
        invalid.registry_agent_ids.push("acme/agent".to_owned());
        assert_eq!(
            registry.register(invalid),
            Err(ModuleRegistryError::InvalidManifest)
        );
    }

    #[test]
    fn registration_rejects_module_ids_with_empty_or_parent_path_segments() {
        let mut registry = AgentModuleRegistry::default();
        for module_id in ["module//agent", "module/../agent", "module/./agent"] {
            assert_eq!(
                registry.register(manifest(module_id, &[OperatingSystem::Linux], 1)),
                Err(ModuleRegistryError::InvalidManifest),
                "module identity must not contain unsafe path segments: {module_id}"
            );
        }
        assert!(registry.list_supported().is_empty());
    }

    #[test]
    fn module_versions_use_semver_and_reject_noncanonical_numeric_parts() {
        let mut registry = AgentModuleRegistry::default();
        let mut valid = manifest("module/preview", &[OperatingSystem::Linux], 1);
        valid.module_version = "1.2.3-rc.1+build.42".to_owned();
        assert_eq!(registry.register(valid), Ok(()));

        let mut invalid = manifest("module/leading-zero", &[OperatingSystem::Linux], 1);
        invalid.module_version = "01.2.3".to_owned();
        assert_eq!(
            registry.register(invalid),
            Err(ModuleRegistryError::InvalidManifest)
        );
    }

    #[test]
    fn module_version_is_bounded_to_the_agent_control_contract_limit() {
        let mut candidate = manifest("module/oversized-version", &[OperatingSystem::Linux], 1);
        candidate.module_version = format!("1.0.0+{}", "a".repeat(123));
        let mut registry = AgentModuleRegistry::default();
        assert_eq!(
            registry.register(candidate),
            Err(ModuleRegistryError::InvalidManifest)
        );
        assert!(registry.list_supported().is_empty());
    }

    #[test]
    fn registry_registration_never_executes_distribution_metadata() {
        let mut candidate = manifest("module/one", &[OperatingSystem::Linux], 1);
        candidate.distribution_kinds = vec!["../../run-this".to_owned()];
        let mut registry = AgentModuleRegistry::default();
        assert_eq!(
            registry.register(candidate),
            Err(ModuleRegistryError::InvalidManifest)
        );
        assert!(registry.list_supported().is_empty());
    }

    #[test]
    fn unsupported_host_api_and_malformed_lookup_do_not_match() {
        let mut registry = AgentModuleRegistry::default();
        registry
            .register(manifest("module/one", &[OperatingSystem::Linux], 1))
            .unwrap();
        assert_eq!(
            registry
                .match_registry_agent(
                    "acme/agent",
                    OperatingSystem::Linux,
                    OperatingArchitecture::X86_64,
                    0,
                )
                .unwrap(),
            None
        );
        assert_eq!(
            registry.match_registry_agent(
                "../agent",
                OperatingSystem::Linux,
                OperatingArchitecture::X86_64,
                2,
            ),
            Err(ModuleRegistryError::InvalidLookup)
        );
    }
}
