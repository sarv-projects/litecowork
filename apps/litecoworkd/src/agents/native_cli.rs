use super::{AgentInstallation, AuthenticationReadiness, InstallationState, SessionReadiness};

#[derive(Clone, Copy)]
pub(super) struct NativeCliCandidate {
    agent_id: &'static str,
    display_name: &'static str,
    executable: &'static str,
    version_args: &'static [&'static str],
    version_prefix: &'static str,
    trailing_period: bool,
    timeout_seconds: u64,
    script_interpreters: &'static [&'static str],
}

pub(super) const CLAUDE_CODE: NativeCliCandidate = NativeCliCandidate {
    agent_id: "claude-code",
    display_name: "Claude Code",
    executable: "claude",
    version_args: &["--version"],
    version_prefix: "",
    trailing_period: false,
    timeout_seconds: 2,
    script_interpreters: &["node"],
};

pub(super) const GEMINI_CLI: NativeCliCandidate = NativeCliCandidate {
    agent_id: "gemini-cli",
    display_name: "Gemini CLI",
    executable: "gemini",
    version_args: &["--version"],
    version_prefix: "",
    trailing_period: false,
    timeout_seconds: 2,
    script_interpreters: &["node"],
};

pub(super) const QWEN_CODE: NativeCliCandidate = NativeCliCandidate {
    agent_id: "qwen-code",
    display_name: "Qwen Code",
    executable: "qwen",
    version_args: &["--version"],
    version_prefix: "",
    trailing_period: false,
    timeout_seconds: 2,
    script_interpreters: &["node"],
};

pub(super) const CURSOR_CLI: NativeCliCandidate = NativeCliCandidate {
    agent_id: "cursor-cli",
    display_name: "Cursor CLI",
    executable: "cursor-agent",
    version_args: &["--version"],
    version_prefix: "",
    trailing_period: false,
    timeout_seconds: 2,
    script_interpreters: &["node"],
};

pub(super) const KIRO_CLI: NativeCliCandidate = NativeCliCandidate {
    agent_id: "kiro-cli",
    display_name: "Kiro CLI",
    executable: "kiro-cli",
    version_args: &["--version"],
    version_prefix: "",
    trailing_period: false,
    timeout_seconds: 2,
    script_interpreters: &[],
};

pub(super) const GOOSE: NativeCliCandidate = NativeCliCandidate {
    agent_id: "goose",
    display_name: "Goose",
    executable: "goose",
    version_args: &["--version"],
    version_prefix: "",
    trailing_period: false,
    timeout_seconds: 2,
    script_interpreters: &[],
};

pub(super) const MISTRAL_VIBE: NativeCliCandidate = NativeCliCandidate {
    agent_id: "mistral-vibe",
    display_name: "Mistral Vibe",
    executable: "vibe",
    version_args: &["--version"],
    version_prefix: "",
    trailing_period: false,
    timeout_seconds: 2,
    script_interpreters: &["python", "python3"],
};

pub(super) const JUNIE_CLI: NativeCliCandidate = NativeCliCandidate {
    agent_id: "jetbrains-junie-cli",
    display_name: "JetBrains Junie CLI",
    executable: "junie",
    version_args: &["--version"],
    version_prefix: "",
    trailing_period: false,
    timeout_seconds: 2,
    script_interpreters: &[],
};

pub(super) const GITHUB_COPILOT_CLI: NativeCliCandidate = NativeCliCandidate {
    agent_id: "github-copilot-cli",
    display_name: "GitHub Copilot CLI",
    executable: "copilot",
    version_args: &["--version"],
    version_prefix: "GitHub Copilot CLI ",
    trailing_period: true,
    // A cold self-extracting installation took ~6.7s in the Linux inventory
    // environment, so keep this probe bounded without falsely hiding it.
    timeout_seconds: 10,
    script_interpreters: &["node"],
};

pub(super) const KIMI_CODE_CLI: NativeCliCandidate = NativeCliCandidate {
    agent_id: "kimi-code-cli",
    display_name: "Kimi Code CLI",
    executable: "kimi",
    version_args: &["--version"],
    version_prefix: "",
    trailing_period: false,
    timeout_seconds: 2,
    script_interpreters: &["python", "python3"],
};

pub(super) const FACTORY_DROID: NativeCliCandidate = NativeCliCandidate {
    agent_id: "factory-droid",
    display_name: "Factory Droid",
    executable: "droid",
    version_args: &["-v"],
    version_prefix: "",
    trailing_period: false,
    timeout_seconds: 2,
    script_interpreters: &[],
};

pub(super) const AUGGIE_CLI: NativeCliCandidate = NativeCliCandidate {
    agent_id: "auggie-cli",
    display_name: "Auggie CLI",
    executable: "auggie",
    version_args: &["--version"],
    version_prefix: "",
    trailing_period: false,
    timeout_seconds: 2,
    script_interpreters: &["node"],
};

pub(super) const INVENTORY_CANDIDATES: [NativeCliCandidate; 12] = [
    CLAUDE_CODE,
    GEMINI_CLI,
    QWEN_CODE,
    CURSOR_CLI,
    KIRO_CLI,
    GOOSE,
    MISTRAL_VIBE,
    JUNIE_CLI,
    GITHUB_COPILOT_CLI,
    KIMI_CODE_CLI,
    FACTORY_DROID,
    AUGGIE_CLI,
];
const MAX_CONCURRENT_VERSION_PROBES: usize = 4;

fn map_bounded<T: Send>(
    item_count: usize,
    max_workers: usize,
    work: impl Fn(usize) -> T + Sync,
) -> Vec<T> {
    use std::sync::atomic::{AtomicUsize, Ordering};

    if item_count == 0 {
        return Vec::new();
    }

    let worker_count = max_workers.max(1).min(item_count);
    let next_item = AtomicUsize::new(0);
    std::thread::scope(|scope| {
        let (sender, receiver) = std::sync::mpsc::channel();
        // Keep workers pulling from one queue so a slow version command does not
        // hold later candidates behind a whole-batch join barrier.
        for _ in 0..worker_count {
            let sender = sender.clone();
            let work = &work;
            let next_item = &next_item;
            let _worker_handle = scope.spawn(move || {
                loop {
                    let index = next_item.fetch_add(1, Ordering::Relaxed);
                    if index >= item_count {
                        break;
                    }
                    if sender.send((index, work(index))).is_err() {
                        break;
                    }
                }
            });
        }
        drop(sender);

        let mut results: Vec<Option<T>> = (0..item_count).map(|_| None).collect();
        for (index, result) in receiver {
            results[index] = Some(result);
        }
        results
            .into_iter()
            .map(|result| result.expect("each bounded worker input returns one result"))
            .collect()
    })
}

pub(super) fn discover(candidate: NativeCliCandidate) -> AgentInstallation {
    let (installation, version) = super::discover_version_with_interpreters(
        candidate.executable,
        candidate.version_args,
        candidate.version_prefix,
        candidate.trailing_period,
        std::time::Duration::from_secs(candidate.timeout_seconds),
        candidate.script_interpreters,
    );
    AgentInstallation {
        agent_id: candidate.agent_id,
        display_name: candidate.display_name,
        protocol_candidate: "CLI",
        installation,
        version,
        authentication: AuthenticationReadiness::Unknown,
        session_readiness: SessionReadiness::NotProbed,
    }
}

pub(super) fn unavailable(candidate: NativeCliCandidate) -> AgentInstallation {
    AgentInstallation {
        agent_id: candidate.agent_id,
        display_name: candidate.display_name,
        protocol_candidate: "CLI",
        installation: InstallationState::VersionUnavailable,
        version: None,
        authentication: AuthenticationReadiness::Unknown,
        session_readiness: SessionReadiness::NotProbed,
    }
}

pub(super) fn discover_all() -> Vec<AgentInstallation> {
    map_bounded(
        INVENTORY_CANDIDATES.len(),
        MAX_CONCURRENT_VERSION_PROBES,
        |index| {
            let candidate = INVENTORY_CANDIDATES[index];
            std::panic::catch_unwind(|| discover(candidate))
                .unwrap_or_else(|_| unavailable(candidate))
        },
    )
}

#[cfg(test)]
mod tests {
    use super::{
        AUGGIE_CLI, FACTORY_DROID, GITHUB_COPILOT_CLI, INVENTORY_CANDIDATES, KIMI_CODE_CLI,
    };
    use crate::agents::{AuthenticationReadiness, SessionReadiness};

    #[test]
    fn native_cli_inventory_has_a_bounded_unique_candidate_for_each_new_portfolio_agent() {
        assert_eq!(INVENTORY_CANDIDATES.len(), 12);
        let ids: Vec<_> = INVENTORY_CANDIDATES
            .iter()
            .map(|candidate| candidate.agent_id)
            .collect();
        assert_eq!(
            ids,
            [
                "claude-code",
                "gemini-cli",
                "qwen-code",
                "cursor-cli",
                "kiro-cli",
                "goose",
                "mistral-vibe",
                "jetbrains-junie-cli",
                "github-copilot-cli",
                "kimi-code-cli",
                "factory-droid",
                "auggie-cli",
            ]
        );
        assert_eq!(
            [
                GITHUB_COPILOT_CLI.executable,
                KIMI_CODE_CLI.executable,
                FACTORY_DROID.executable,
                AUGGIE_CLI.executable,
            ],
            ["copilot", "kimi", "droid", "auggie"]
        );
        assert!(
            INVENTORY_CANDIDATES.iter().all(|candidate| {
                candidate.timeout_seconds > 0 && candidate.timeout_seconds <= 10
            })
        );
    }

    #[test]
    fn vendor_specific_version_flags_and_copilot_prefix_are_pinned() {
        assert_eq!(GITHUB_COPILOT_CLI.version_args, ["--version"]);
        assert_eq!(GITHUB_COPILOT_CLI.version_prefix, "GitHub Copilot CLI ");
        assert!(GITHUB_COPILOT_CLI.trailing_period);
        assert_eq!(KIMI_CODE_CLI.version_args, ["--version"]);
        assert_eq!(FACTORY_DROID.version_args, ["-v"]);
        assert_eq!(AUGGIE_CLI.version_args, ["--version"]);
    }

    #[cfg(windows)]
    #[test]
    fn windows_inventory_falls_back_to_npm_cmd_shims_without_shell_arguments() {
        use crate::agents::{InstallationState, discover_version_with_args};
        use std::{fs, time::Duration};

        let directory = tempfile::tempdir().expect("temporary CLI directory");
        let executable_stem = directory.path().join("fixture-agent");
        let command_file = executable_stem.with_extension("cmd");
        fs::write(&command_file, "@echo off\r\necho 1.2.3\r\n")
            .expect("write deterministic version shim");

        let (installation, version) = discover_version_with_args(
            &executable_stem.to_string_lossy(),
            &["--version"],
            "",
            false,
            Duration::from_secs(2),
        );

        assert!(matches!(installation, InstallationState::Installed));
        assert_eq!(version.as_deref(), Some("1.2.3"));
    }

    #[test]
    fn inventory_scheduler_keeps_other_probes_moving_while_one_probe_is_slow() {
        use std::sync::{Mutex, mpsc};
        use std::time::Duration;

        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let release_rx = Mutex::new(release_rx);
        let inventory = std::thread::spawn(move || {
            super::map_bounded(3, 2, |index| {
                if index == 0 {
                    release_rx
                        .lock()
                        .expect("release receiver lock")
                        .recv()
                        .expect("release slow probe");
                }
                if index == 2 {
                    let _ = started_tx.send(());
                }
                index
            })
        });

        let later_probe_started = started_rx.recv_timeout(Duration::from_millis(250)).is_ok();
        let _ = release_tx.send(());
        assert_eq!(
            inventory.join().expect("inventory workers complete"),
            [0, 1, 2]
        );
        assert!(
            later_probe_started,
            "a slow native CLI must not hold later inventory probes behind a batch barrier"
        );
    }

    #[test]
    fn inventory_scheduler_bounds_parallelism_and_preserves_candidate_order() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::time::Duration;

        let active = AtomicUsize::new(0);
        let peak = AtomicUsize::new(0);
        let inventory = super::map_bounded(12, 4, |index| {
            let now_active = active.fetch_add(1, Ordering::SeqCst) + 1;
            peak.fetch_max(now_active, Ordering::SeqCst);
            std::thread::sleep(Duration::from_millis(5));
            active.fetch_sub(1, Ordering::SeqCst);
            index
        });

        assert_eq!(inventory, (0..12).collect::<Vec<_>>());
        assert!(peak.load(Ordering::SeqCst) <= 4);
        assert!(peak.load(Ordering::SeqCst) > 1);
    }

    #[test]
    fn runtime_inventory_lists_all_fifteen_candidates_without_claiming_auth_or_sessions() {
        let inventory = super::super::discover_local_installations();
        assert_eq!(inventory.len(), 15);
        for candidate in INVENTORY_CANDIDATES {
            let rows: Vec<_> = inventory
                .iter()
                .filter(|row| row.agent_id == candidate.agent_id)
                .collect();
            assert_eq!(
                rows.len(),
                1,
                "{} has exactly one inventory row",
                candidate.agent_id
            );
            assert!(matches!(
                rows[0].authentication,
                AuthenticationReadiness::Unknown
            ));
            assert!(matches!(
                rows[0].session_readiness,
                SessionReadiness::NotProbed
            ));
        }
        for agent_id in ["codex-cli", "opencode", "cline-cli"] {
            assert_eq!(
                inventory
                    .iter()
                    .filter(|row| row.agent_id == agent_id)
                    .count(),
                1,
                "{agent_id} has exactly one inventory row"
            );
        }
    }
}
