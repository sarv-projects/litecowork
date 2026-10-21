//! Honest provider capability probing for the Linux local Environment provider.
//!
//! `docs/ENVIRONMENTS.md` requires that `probe` reports explicit feature flags and
//! limits, and that unknown support is *unavailable*, not assumed. This module is the
//! probe for the Linux substrate: it reports which host mechanisms exist, and it never
//! claims enforcement.
//!
//! A probe result is not an attestation. Only an observed launch can produce one, and
//! an attestation built from probe data alone must be refused at admission.

use std::path::{Path, PathBuf};

/// Candidate locations of the Bubblewrap binary.
pub const BUBBLEWRAP_CANDIDATES: [&str; 4] = [
    "/usr/bin/bwrap",
    "/bin/bwrap",
    "/usr/local/bin/bwrap",
    "/usr/libexec/bwrap",
];

/// One Bubblewrap candidate location.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BubblewrapCandidate {
    pub path: PathBuf,
    pub present: bool,
    /// The binary is present and is owned by root and not group/world writable.
    pub trusted: bool,
}

/// What the host exposes for the local Environment provider.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalProviderProbe {
    pub operating_system: String,
    pub operating_system_build: String,
    pub bubblewrap_candidates: Vec<BubblewrapCandidate>,
    /// The trusted Bubblewrap binary, when one exists.
    pub trusted_bubblewrap: Option<PathBuf>,
    /// Whether a delegated cgroup v2 hierarchy is present.
    pub cgroup_v2: bool,
    /// The cgroup path this process is in.
    pub cgroup_path: Option<String>,
    /// Whether a systemd system manager is running.
    pub systemd_available: bool,
    /// Whether unprivileged user namespaces can be created.
    pub user_namespaces_available: bool,
    /// Every unmet requirement, stated as a human-readable reason.
    pub reasons: Vec<String>,
}

impl LocalProviderProbe {
    /// Whether the probe found every host mechanism the local provider needs.
    ///
    /// This says nothing about whether the mechanisms enforce anything; it only says
    /// that the host exposes them.
    pub fn host_mechanisms_present(&self) -> bool {
        self.trusted_bubblewrap.is_some()
            && self.cgroup_v2
            && self.systemd_available
            && self.user_namespaces_available
    }
}

/// Probe the host for the local Environment provider requirements.
pub fn probe_local_provider() -> LocalProviderProbe {
    let operating_system = std::env::consts::OS.to_owned();
    let operating_system_build = operating_system_build();
    let mut reasons = Vec::new();

    let mut bubblewrap_candidates = Vec::new();
    let mut trusted_bubblewrap = None;
    for candidate in BUBBLEWRAP_CANDIDATES {
        let path = PathBuf::from(candidate);
        let present = path.is_file();
        let trusted = present && bubblewrap_binary_is_trusted(&path);
        if present && !trusted {
            reasons.push(format!(
                "{candidate} is present but is not a root-owned non-group/world-writable binary"
            ));
        }
        if trusted && trusted_bubblewrap.is_none() {
            trusted_bubblewrap = Some(path.clone());
        }
        bubblewrap_candidates.push(BubblewrapCandidate {
            path,
            present,
            trusted,
        });
    }
    if trusted_bubblewrap.is_none() {
        reasons.push("no trusted bubblewrap binary is available on this host".to_owned());
    }

    let cgroup_v2 = cgroup_v2_available();
    if !cgroup_v2 {
        reasons.push("this host does not expose a cgroup v2 hierarchy".to_owned());
    }
    let cgroup_path = current_cgroup_path();
    let systemd_available = systemd_available();
    if !systemd_available {
        reasons.push("no systemd system manager is running on this host".to_owned());
    }
    let user_namespaces_available = user_namespaces_available();
    if !user_namespaces_available {
        reasons.push(
            "this host does not permit unprivileged user namespaces".to_owned(),
        );
    }

    LocalProviderProbe {
        operating_system,
        operating_system_build,
        bubblewrap_candidates,
        trusted_bubblewrap,
        cgroup_v2,
        cgroup_path,
        systemd_available,
        user_namespaces_available,
        reasons,
    }
}

/// The operating system build identifier.
pub fn operating_system_build() -> String {
    #[cfg(target_os = "linux")]
    {
        if let Ok(build) = std::fs::read_to_string("/proc/sys/kernel/osrelease") {
            let trimmed = build.trim();
            if !trimmed.is_empty() {
                return trimmed.to_owned();
            }
        }
    }
    "unavailable".to_owned()
}

/// Whether the host exposes a delegated cgroup v2 hierarchy.
pub fn cgroup_v2_available() -> bool {
    Path::new("/sys/fs/cgroup/cgroup.controllers").is_file()
}

/// The cgroup path this process is in.
pub fn current_cgroup_path() -> Option<String> {
    std::fs::read_to_string("/proc/self/cgroup")
        .ok()
        .map(|content| content.trim().to_owned())
        .filter(|content| !content.is_empty())
}

/// Whether a systemd system manager is running.
pub fn systemd_available() -> bool {
    Path::new("/run/systemd/system").is_dir()
}

/// Whether unprivileged user namespaces can be created on this host.
pub fn user_namespaces_available() -> bool {
    if let Ok(clone) = std::fs::read_to_string("/proc/sys/kernel/unprivileged_userns_clone") {
        if clone.trim() == "0" {
            return false;
        }
    }
    if let Ok(max) = std::fs::read_to_string("/proc/sys/user/max_user_namespaces") {
        if max.trim() == "0" {
            return false;
        }
    }
    true
}

/// Whether the Bubblewrap binary at `path` is trusted for a sandbox launch.
///
/// A trusted binary is owned by root and is not group- or world-writable. A binary
/// that a local user could replace would defeat the whole namespace boundary. The
/// path is canonicalized first so that the probe and the launch path agree on what
/// "trusted" means for a symlinked candidate.
#[cfg(target_os = "linux")]
pub fn bubblewrap_binary_is_trusted(path: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    let Ok(canonical) = std::fs::canonicalize(path) else {
        return false;
    };
    let Ok(metadata) = std::fs::metadata(&canonical) else {
        return false;
    };
    if !metadata.is_file() || metadata.uid() != 0 {
        return false;
    }
    metadata.mode() & 0o022 == 0
}

/// Whether the Bubblewrap binary at `path` is trusted for a sandbox launch.
#[cfg(not(target_os = "linux"))]
pub fn bubblewrap_binary_is_trusted(_path: &Path) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_probe_never_trusts_a_missing_binary() {
        let probe = probe_local_provider();
        for candidate in &probe.bubblewrap_candidates {
            if !candidate.present {
                assert!(!candidate.trusted, "{:?}", candidate.path);
            }
            if candidate.trusted {
                assert!(candidate.present, "{:?}", candidate.path);
            }
        }
    }

    #[test]
    fn a_reported_trusted_binary_is_one_of_the_trusted_candidates() {
        let probe = probe_local_provider();
        match &probe.trusted_bubblewrap {
            Some(path) => assert!(
                probe
                    .bubblewrap_candidates
                    .iter()
                    .any(|candidate| candidate.trusted && candidate.path == *path),
                "{path:?}"
            ),
            None => assert!(
                !probe
                    .bubblewrap_candidates
                    .iter()
                    .any(|candidate| candidate.trusted)
            ),
        }
    }

    #[test]
    fn the_probe_states_a_reason_for_every_missing_requirement() {
        let probe = probe_local_provider();
        if probe.trusted_bubblewrap.is_none() {
            assert!(
                probe
                    .reasons
                    .iter()
                    .any(|reason| reason.contains("bubblewrap")),
                "{:?}",
                probe.reasons
            );
        }
        if !probe.cgroup_v2 {
            assert!(probe.reasons.iter().any(|reason| reason.contains("cgroup")));
        }
        if !probe.systemd_available {
            assert!(probe.reasons.iter().any(|reason| reason.contains("systemd")));
        }
        if !probe.user_namespaces_available {
            assert!(probe.reasons.iter().any(|reason| reason.contains("namespace")));
        }
    }

    #[test]
    fn host_mechanism_presence_requires_every_mechanism() {
        let probe = probe_local_provider();
        assert_eq!(
            probe.host_mechanisms_present(),
            probe.trusted_bubblewrap.is_some()
                && probe.cgroup_v2
                && probe.systemd_available
                && probe.user_namespaces_available
        );
    }

    #[test]
    fn an_untrusted_binary_is_refused() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let path = temporary.path().join("bwrap");
        std::fs::write(&path, b"not a real binary").expect("write");
        assert!(!bubblewrap_binary_is_trusted(&path));
        assert!(!bubblewrap_binary_is_trusted(
            &temporary.path().join("missing")
        ));
    }

    #[test]
    fn a_symlinked_candidate_is_judged_by_its_target() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let target = temporary.path().join("target");
        std::fs::write(&target, b"not a real binary").expect("write");
        let link = temporary.path().join("bwrap");
        std::os::unix::fs::symlink(&target, &link).expect("symlink");
        // The link itself is user-writable, and its target is not root-owned, so the
        // candidate is refused for the same reason the launch path would refuse it.
        assert!(!bubblewrap_binary_is_trusted(&link));
    }
}
