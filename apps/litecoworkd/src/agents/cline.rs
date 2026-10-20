use super::{AgentInstallation, AuthenticationReadiness, InstallationState, SessionReadiness};

pub(super) fn discover() -> AgentInstallation {
    let (installation, version) = match discover_npm_package_version() {
        Some(version) => (InstallationState::Installed, Some(version)),
        None => {
            let (installation, version) = super::discover_version("cline", "");
            let version = version.and_then(|version| {
                let release = version
                    .split_once('+')
                    .map_or(version.as_str(), |(core, _)| core);
                is_plain_numeric_release(release).then(|| release.to_owned())
            });
            let installation = match installation {
                InstallationState::Installed if version.is_none() => {
                    InstallationState::VersionUnavailable
                }
                other => other,
            };
            (installation, version)
        }
    };
    AgentInstallation {
        agent_id: "cline-cli",
        display_name: "Cline CLI",
        protocol_candidate: "ACP",
        installation,
        version,
        authentication: AuthenticationReadiness::Unknown,
        session_readiness: SessionReadiness::NotProbed,
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
const MAX_PACKAGE_JSON_BYTES: u64 = 64 * 1024;
#[cfg(any(target_os = "linux", target_os = "macos"))]
const MAX_PACKAGE_ANCESTORS: usize = 6;

/// Reads only bounded, local npm package metadata. The CLI version command can
/// take many seconds on a cold Cline startup, so use its installed package
/// manifest when the executable resolves to a Cline npm package. This is
/// installation inventory only; it does not initialize ACP or inspect auth.
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn discover_npm_package_version() -> Option<String> {
    let path = std::env::var_os("PATH")?;
    let executable = std::env::split_paths(&path)
        .map(|directory| directory.join("cline"))
        .find(|candidate| is_executable_file(candidate))?;
    let executable = std::fs::canonicalize(executable).ok()?;

    for ancestor in executable.parent()?.ancestors().take(MAX_PACKAGE_ANCESTORS) {
        let manifest = ancestor.join("package.json");
        match std::fs::symlink_metadata(&manifest) {
            Ok(_) => return package_version(&manifest),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => return None,
        }
    }
    None
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn is_executable_file(path: &std::path::Path) -> bool {
    std::fs::metadata(path).is_ok_and(|metadata| metadata.is_file())
        && rustix::fs::accessat(
            rustix::fs::CWD,
            path,
            rustix::fs::Access::EXEC_OK,
            rustix::fs::AtFlags::EACCESS,
        )
        .is_ok()
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn package_version(path: &std::path::Path) -> Option<String> {
    use std::{fs::File, io::Read, os::fd::OwnedFd};

    // Reject ordinary symlinks and oversized files before opening. O_NOFOLLOW
    // closes the symlink race; O_NONBLOCK ensures a raced FIFO cannot hang
    // inventory. The read itself is capped even if the file grows afterward.
    let metadata = std::fs::symlink_metadata(path).ok()?;
    if !metadata.file_type().is_file() || metadata.len() > MAX_PACKAGE_JSON_BYTES {
        return None;
    }
    let fd: OwnedFd = rustix::fs::open(
        path,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::NONBLOCK
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .ok()?;
    let mut file = File::from(fd);
    let metadata = file.metadata().ok()?;
    if !metadata.is_file() || metadata.len() > MAX_PACKAGE_JSON_BYTES {
        return None;
    }

    let mut contents = Vec::with_capacity(metadata.len() as usize);
    file.by_ref()
        .take(MAX_PACKAGE_JSON_BYTES + 1)
        .read_to_end(&mut contents)
        .ok()?;
    if contents.len() as u64 > MAX_PACKAGE_JSON_BYTES {
        return None;
    }

    let manifest: serde_json::Value = serde_json::from_slice(&contents).ok()?;
    let object = manifest.as_object()?;
    if object.get("name")?.as_str()? != "cline" {
        return None;
    }
    let version = object.get("version")?.as_str()?;
    // Package metadata is untrusted and returned to the Operator as inventory.
    // Accept a plain numeric release only; arbitrary prerelease/build text may
    // contain account identifiers or other private data.
    is_plain_numeric_release(version).then(|| version.to_owned())
}

fn is_plain_numeric_release(version: &str) -> bool {
    super::is_version_shaped(version)
        && version
            .bytes()
            .all(|byte| byte.is_ascii_digit() || byte == b'.')
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn discover_npm_package_version() -> Option<String> {
    None
}

pub(super) fn unavailable() -> AgentInstallation {
    AgentInstallation {
        agent_id: "cline-cli",
        display_name: "Cline CLI",
        protocol_candidate: "ACP",
        installation: InstallationState::VersionUnavailable,
        version: None,
        authentication: AuthenticationReadiness::Unknown,
        session_readiness: SessionReadiness::NotProbed,
    }
}

#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
mod tests {
    use std::{
        fs,
        os::unix::fs::PermissionsExt,
        path::{Path, PathBuf},
        process::{Command, Output},
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::super::{
        AgentInstallation, AuthenticationReadiness, InstallationState, SessionReadiness,
        discover_local_installations,
    };
    use super::MAX_PACKAGE_JSON_BYTES;

    const CHILD_TEST_MODE: &str = "LITECOWORK_CLINE_INVENTORY_TEST_CHILD";
    const SECRET_ENV: &str = "LITECOWORK_CLINE_INVENTORY_TEST_SECRET";
    const TEST_SECRET: &str = "inventory-test-secret-must-not-be-forwarded";

    struct TemporaryDirectory(PathBuf);

    impl TemporaryDirectory {
        fn new() -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock is after Unix epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "litecowork-cline-inventory-{}-{unique}",
                std::process::id()
            ));
            fs::create_dir_all(&path).expect("create temporary Cline inventory directory");
            Self(path)
        }

        fn remove(self) {
            let path = self.0.clone();
            drop(self);
            assert!(
                !path.exists(),
                "temporary Cline inventory directory remains"
            );
        }
    }

    impl Drop for TemporaryDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn node_interpreter() -> PathBuf {
        std::env::split_paths(&std::env::var_os("PATH").expect("test process PATH"))
            .map(|directory| directory.join("node"))
            .find(|candidate| {
                fs::metadata(candidate).is_ok_and(|metadata| {
                    metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
                })
            })
            .expect("Cline's qualified node interpreter is installed for this test")
    }

    fn write_node_executable(path: &Path, body: &str) {
        let source = format!("#!{}\n{body}", node_interpreter().display());
        fs::write(path, source).expect("write fake Node-based Cline executable");
        let mut permissions = fs::metadata(path)
            .expect("read fake Node-based Cline executable metadata")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions)
            .expect("make fake Node-based Cline executable runnable");
    }

    fn write_fake_npm_package(
        temporary: &TemporaryDirectory,
        manifest: &[u8],
        marker: &Path,
    ) -> PathBuf {
        use std::os::unix::fs::symlink;

        let package = temporary.0.join("node_modules/cline");
        let executable = package.join("bin/cline");
        let path_directory = temporary.0.join("path");
        fs::create_dir_all(executable.parent().expect("executable parent"))
            .expect("create fake npm package bin directory");
        fs::create_dir_all(&path_directory).expect("create isolated PATH directory");
        fs::write(package.join("package.json"), manifest).expect("write package metadata");
        write_node_executable(
            &executable,
            &format!(
                "require('node:fs').writeFileSync({}, ''); console.log('9.8.7');\n",
                serde_json::to_string(&marker.to_string_lossy().to_string())
                    .expect("encode marker path")
            ),
        );
        symlink(&executable, path_directory.join("cline")).expect("link fake CLI into PATH");
        path_directory
    }

    fn cline_row() -> AgentInstallation {
        discover_local_installations()
            .into_iter()
            .find(|installation| installation.agent_id == "cline-cli")
            .expect("local inventory contains the Cline candidate")
    }

    fn run_child_case(_mode: &str) -> Option<AgentInstallation> {
        let Ok(child_mode) = std::env::var(CHILD_TEST_MODE) else {
            return None;
        };

        let installation = cline_row();
        match child_mode.as_str() {
            "version" => {
                assert!(matches!(
                    installation.installation,
                    InstallationState::Installed
                ));
                assert_eq!(installation.version.as_deref(), Some("3.0.72"));
            }
            "metadata" => {
                assert!(matches!(
                    installation.installation,
                    InstallationState::Installed
                ));
                assert_eq!(installation.version.as_deref(), Some("3.0.72"));
            }
            "metadata_fallback" => {
                assert!(matches!(
                    installation.installation,
                    InstallationState::Installed
                ));
                assert_eq!(installation.version.as_deref(), Some("9.8.7"));
            }
            "metadata_unusable" => {
                assert!(
                    matches!(
                        installation.installation,
                        InstallationState::VersionUnavailable
                    ),
                    "unexpected result for unusable metadata: {installation:?}"
                );
                assert_eq!(installation.version, None);
            }
            "cli_build_metadata" => {
                assert!(matches!(
                    installation.installation,
                    InstallationState::Installed
                ));
                assert_eq!(installation.version.as_deref(), Some("9.8.7"));
                assert!(
                    !serde_json::to_string(&installation)
                        .expect("serialize safe fallback row")
                        .contains("private-account")
                );
            }
            "malformed" | "timeout" | "cli_prerelease" => {
                assert!(matches!(
                    installation.installation,
                    InstallationState::VersionUnavailable
                ));
                assert_eq!(installation.version, None);
            }
            "missing" => {
                assert!(matches!(
                    installation.installation,
                    InstallationState::Missing
                ));
                assert_eq!(installation.version, None);
            }
            other => panic!("unknown inventory test child mode: {other}"),
        }

        assert_eq!(installation.agent_id, "cline-cli");
        assert_eq!(installation.display_name, "Cline CLI");
        assert_eq!(installation.protocol_candidate, "ACP");
        assert!(matches!(
            installation.authentication,
            AuthenticationReadiness::Unknown
        ));
        assert!(matches!(
            installation.session_readiness,
            SessionReadiness::NotProbed
        ));
        assert!(
            !serde_json::to_string(&installation)
                .expect("serialize safe inventory row")
                .contains(TEST_SECRET)
        );
        Some(installation)
    }

    fn run_isolated_inventory_case(test_name: &str, mode: &str, path: &std::ffi::OsStr) -> Output {
        let test_binary = std::env::current_exe().expect("current Rust test executable");
        Command::new(test_binary)
            .args(["--exact", test_name, "--nocapture"])
            .env_clear()
            .env("PATH", path)
            .env(CHILD_TEST_MODE, mode)
            .env(SECRET_ENV, TEST_SECRET)
            .output()
            .expect("run isolated Cline inventory test child")
    }

    fn assert_child_passed(output: &Output) {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            output.status.success(),
            "isolated inventory test failed: stdout={stdout:?}, stderr={stderr:?}"
        );
        assert!(
            stdout.contains("running 1 test"),
            "unexpected child output: {stdout:?}"
        );
        assert!(!stdout.contains(TEST_SECRET));
        assert!(!stderr.contains(TEST_SECRET));
        assert!(!stdout.contains("raw-native-output"));
        assert!(!stderr.contains("raw-native-output"));
        assert!(!stdout.contains("private-account"));
        assert!(!stderr.contains("private-account"));
    }

    #[test]
    fn cline_inventory_recognizes_exact_version_and_does_not_forward_secrets_or_raw_output() {
        if run_child_case("version").is_some() {
            return;
        }

        let temporary = TemporaryDirectory::new();
        write_node_executable(
            &temporary.0.join("cline"),
            &format!(
                "if (process.env[{secret_key:?}] === {secret:?}) {{ console.log('raw-native-output'); process.exit(71); }}\nconsole.log('3.0.72'); console.log('raw-native-output');\n",
                secret_key = SECRET_ENV,
                secret = TEST_SECRET,
            ),
        );
        let output = run_isolated_inventory_case(
            "agents::cline::tests::cline_inventory_recognizes_exact_version_and_does_not_forward_secrets_or_raw_output",
            "version",
            temporary.0.as_os_str(),
        );
        assert_child_passed(&output);
        temporary.remove();
    }

    #[test]
    fn cline_inventory_reads_bounded_npm_metadata_without_executing_the_cli() {
        if run_child_case("metadata").is_some() {
            return;
        }

        let temporary = TemporaryDirectory::new();
        let marker = temporary.0.join("cli-was-executed");
        let path = write_fake_npm_package(
            &temporary,
            br#"{"name":"cline","version":"3.0.72"}"#,
            &marker,
        );
        let output = run_isolated_inventory_case(
            "agents::cline::tests::cline_inventory_reads_bounded_npm_metadata_without_executing_the_cli",
            "metadata",
            path.as_os_str(),
        );
        assert_child_passed(&output);
        assert!(!marker.exists(), "metadata fast path executed the CLI");
        temporary.remove();
    }

    #[test]
    fn cline_inventory_skips_a_non_executable_path_entry_for_a_later_candidate() {
        if run_child_case("metadata").is_some() {
            return;
        }

        let temporary = TemporaryDirectory::new();
        let first_path = temporary.0.join("first-path-entry");
        fs::create_dir_all(&first_path).expect("create first PATH entry");
        let unusable = first_path.join("cline");
        fs::write(&unusable, "not executable").expect("write non-executable CLI candidate");
        let mut permissions = fs::metadata(&unusable)
            .expect("read non-executable candidate metadata")
            .permissions();
        permissions.set_mode(0o644);
        fs::set_permissions(&unusable, permissions).expect("make first candidate non-executable");

        let marker = temporary.0.join("later-cli-was-executed");
        let second_path = write_fake_npm_package(
            &temporary,
            br#"{"name":"cline","version":"3.0.72"}"#,
            &marker,
        );
        let joined_path = std::env::join_paths([first_path.as_os_str(), second_path.as_os_str()])
            .expect("combine isolated PATH entries");
        let output = run_isolated_inventory_case(
            "agents::cline::tests::cline_inventory_skips_a_non_executable_path_entry_for_a_later_candidate",
            "metadata",
            &joined_path,
        );
        assert_child_passed(&output);
        assert!(
            !marker.exists(),
            "metadata fast path executed the later CLI"
        );
        temporary.remove();
    }

    #[test]
    fn cline_inventory_does_not_attribute_an_inner_package_to_a_cline_ancestor() {
        if run_child_case("metadata_fallback").is_some() {
            return;
        }

        let temporary = TemporaryDirectory::new();
        let outer = temporary.0.join("node_modules/cline");
        let inner = outer.join("node_modules/other");
        let executable = inner.join("bin/cline");
        let path_directory = temporary.0.join("path");
        let marker = temporary.0.join("inner-cli-was-executed");
        fs::create_dir_all(executable.parent().expect("inner executable parent"))
            .expect("create nested npm package");
        fs::create_dir_all(&path_directory).expect("create isolated PATH directory");
        fs::write(
            outer.join("package.json"),
            br#"{"name":"cline","version":"3.0.72"}"#,
        )
        .expect("write outer Cline manifest");
        fs::write(
            inner.join("package.json"),
            br#"{"name":"other","version":"1.0.0"}"#,
        )
        .expect("write nearest non-Cline manifest");
        write_node_executable(
            &executable,
            &format!(
                "require('node:fs').writeFileSync({}, ''); console.log('9.8.7');\n",
                serde_json::to_string(&marker.to_string_lossy().to_string())
                    .expect("encode marker path")
            ),
        );
        std::os::unix::fs::symlink(&executable, path_directory.join("cline"))
            .expect("link nested CLI into PATH");

        let output = run_isolated_inventory_case(
            "agents::cline::tests::cline_inventory_does_not_attribute_an_inner_package_to_a_cline_ancestor",
            "metadata_fallback",
            path_directory.as_os_str(),
        );
        assert_child_passed(&output);
        assert!(
            marker.exists(),
            "wrong nearest package did not trigger CLI fallback"
        );
        temporary.remove();
    }

    #[test]
    fn cline_inventory_falls_back_when_npm_metadata_is_invalid_or_untrusted() {
        if run_child_case("metadata_fallback").is_some() {
            return;
        }

        let malformed = br#"{"name":"cline","version": "#;
        let wrong_name = br#"{"name":"other-package","version":"3.0.72"}"#;
        let invalid_version = br#"{"name":"cline","version":"3.0.72 account=private"}"#;
        let mut oversized = br#"{"name":"cline","version":"3.0.72"}"#.to_vec();
        oversized.resize(MAX_PACKAGE_JSON_BYTES as usize + 1, b' ');
        let build_metadata = br#"{"name":"cline","version":"3.0.72+private-account"}"#;
        let prerelease = br#"{"name":"cline","version":"3.0.72-rc.1"}"#;
        let cases: [(&str, &[u8], bool); 6] = [
            // Node itself refuses to execute a package beneath malformed
            // package.json, so fallback is truthfully unavailable.
            ("malformed", malformed, false),
            ("wrong-name", wrong_name, true),
            ("invalid-version", invalid_version, true),
            ("private-build-metadata", build_metadata, true),
            ("prerelease", prerelease, true),
            ("oversized", oversized.as_slice(), true),
        ];

        for (label, manifest, fallback_runs) in cases {
            let temporary = TemporaryDirectory::new();
            let marker = temporary.0.join("cli-was-executed");
            let path = write_fake_npm_package(&temporary, manifest, &marker);
            let output = run_isolated_inventory_case(
                "agents::cline::tests::cline_inventory_falls_back_when_npm_metadata_is_invalid_or_untrusted",
                if fallback_runs {
                    "metadata_fallback"
                } else {
                    "metadata_unusable"
                },
                path.as_os_str(),
            );
            assert_child_passed(&output);
            assert_eq!(
                marker.exists(),
                fallback_runs,
                "unexpected CLI fallback for {label} metadata"
            );
            temporary.remove();
        }
    }

    #[test]
    fn cline_inventory_hides_malformed_native_version_output() {
        if run_child_case("malformed").is_some() {
            return;
        }

        let temporary = TemporaryDirectory::new();
        write_node_executable(
            &temporary.0.join("cline"),
            "console.log('3.0.72 account=private-account');\n",
        );
        let output = run_isolated_inventory_case(
            "agents::cline::tests::cline_inventory_hides_malformed_native_version_output",
            "malformed",
            temporary.0.as_os_str(),
        );
        assert_child_passed(&output);
        temporary.remove();
    }

    #[test]
    fn cline_inventory_strips_private_build_metadata_from_cli_version() {
        if run_child_case("cli_build_metadata").is_some() {
            return;
        }

        let temporary = TemporaryDirectory::new();
        write_node_executable(
            &temporary.0.join("cline"),
            "console.log('9.8.7+private-account');\n",
        );
        let output = run_isolated_inventory_case(
            "agents::cline::tests::cline_inventory_strips_private_build_metadata_from_cli_version",
            "cli_build_metadata",
            temporary.0.as_os_str(),
        );
        assert_child_passed(&output);
        temporary.remove();
    }

    #[test]
    fn cline_inventory_rejects_prerelease_cli_version() {
        if run_child_case("cli_prerelease").is_some() {
            return;
        }

        let temporary = TemporaryDirectory::new();
        write_node_executable(&temporary.0.join("cline"), "console.log('9.8.7-rc.1');\n");
        let output = run_isolated_inventory_case(
            "agents::cline::tests::cline_inventory_rejects_prerelease_cli_version",
            "cli_prerelease",
            temporary.0.as_os_str(),
        );
        assert_child_passed(&output);
        temporary.remove();
    }

    #[test]
    fn cline_inventory_reports_a_missing_executable_without_inventing_a_version() {
        if run_child_case("missing").is_some() {
            return;
        }

        let temporary = TemporaryDirectory::new();
        let output = run_isolated_inventory_case(
            "agents::cline::tests::cline_inventory_reports_a_missing_executable_without_inventing_a_version",
            "missing",
            temporary.0.as_os_str(),
        );
        assert_child_passed(&output);
        temporary.remove();
    }

    #[test]
    fn cline_inventory_bounds_a_hung_version_probe() {
        if run_child_case("timeout").is_some() {
            return;
        }

        let temporary = TemporaryDirectory::new();
        write_node_executable(&temporary.0.join("cline"), "setTimeout(() => {}, 5000);\n");
        let output = run_isolated_inventory_case(
            "agents::cline::tests::cline_inventory_bounds_a_hung_version_probe",
            "timeout",
            temporary.0.as_os_str(),
        );
        assert_child_passed(&output);
        temporary.remove();
    }
}
