use std::process::Command;

fn litecoworkd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_litecoworkd"))
}

#[test]
fn help_is_available_without_starting_runtime_services() {
    let output = litecoworkd().arg("--help").output().expect("run CLI");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 help output");
    assert!(stdout.contains("USAGE:"));
    assert!(stdout.contains("does not start a listener"));
    assert!(output.stderr.is_empty());
}

#[test]
fn no_arguments_show_help() {
    let output = litecoworkd().output().expect("run CLI");

    assert!(output.status.success());
    assert!(
        String::from_utf8(output.stdout)
            .expect("UTF-8 help output")
            .contains("USAGE:")
    );
}

#[test]
fn version_matches_the_built_package() {
    let output = litecoworkd().arg("--version").output().expect("run CLI");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).expect("UTF-8 version output"),
        format!("litecoworkd {}\n", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn unsupported_argument_fails_without_echoing_values_or_starting_a_server() {
    let secret_shaped_value = "do-not-log-this-test-token";
    let output = litecoworkd()
        .arg(format!("--token={secret_shaped_value}"))
        .output()
        .expect("run CLI");

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).expect("UTF-8 error output");
    assert!(stderr.contains("unsupported arguments."));
    assert!(!stderr.contains(secret_shaped_value));
    assert!(
        !String::from_utf8(output.stdout)
            .expect("UTF-8 output")
            .contains(secret_shaped_value)
    );
}
