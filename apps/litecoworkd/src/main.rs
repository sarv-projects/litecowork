mod agent_credentials_operator;
mod agents;
mod attempt_process_scope;
mod backup_operator;
// Browser input control and profile/consent policy. Compiled but not wired: no
// Operator route, provider, or Chrome process is connected yet.
#[allow(dead_code)]
mod browser_consent;
#[allow(dead_code)]
mod browser_control_service;
// Action batches, deadline preflight, and warm Environment holds. Compiled but not
// wired: no Operator route or provider dispatch consumes these decisions yet.
#[allow(dead_code)]
mod environment_action_batch;
// Desktop computer use and opt-in observation policy.
#[allow(dead_code)]
mod environment_computer_use;
// Runtime-local isolation evidence for Task-scoped Environments. It is compiled but
// not wired into Task admission: the planning isolation gate stays closed until a
// qualified provider produces an attestation for the current Runtime incarnation.
#[allow(dead_code)]
mod environment_isolation;
// Office, data, and research capability qualification rules.
#[allow(dead_code)]
mod environment_office_qualification;
// Environment provider capability, budget admission, and lifecycle guards.
#[allow(dead_code)]
mod environment_provider;
mod local_filesystem;
mod operator;
mod runtime;
mod workspace_root_cleanup_worker;
// The evaluator is intentionally compiled but not wired until the atomic
// CapabilityBroker admission transaction and audit persistence exist.
#[allow(dead_code)]
mod trust;

use std::{path::PathBuf, process::ExitCode};

const HELP: &str = "litecoworkd — LiteCowork local Runtime\n\nUSAGE:\n    litecoworkd [--help | --version]\n    litecoworkd run --data-dir <private-directory>\n    litecoworkd status --data-dir <private-directory>\n    litecoworkd backup <create|verify|restore|upgrade-check> [options]\n\n`run` starts the local daemon, acquires the single-instance lock, opens durable storage,\nand serves the local Operator API over OS-authenticated local IPC on supported platforms.\nWorkspace creation is idempotent per local Principal and request key. This build does not accept work: the Runtime remains DEGRADED until Task recovery and execution services are available, and it does not launch agents. SIGINT/SIGTERM are process-supervisor shutdown signals; there is no authenticated Operator stop command yet.\n\n`backup` is the local recovery surface. Run `litecoworkd backup --help` for its options. A backup\ncontainer is encrypted with an operator supplied passphrase; the passphrase is never written to the\narchive, to the archive name, or to a log line.\n";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.is_empty() || args.as_slice() == ["--help"] || args.as_slice() == ["-h"] {
        print!("{HELP}");
        return ExitCode::SUCCESS;
    }
    if args.as_slice() == ["--version"] || args.as_slice() == ["-V"] {
        println!("litecoworkd {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }

    match args.as_slice() {
        [command, rest @ ..] if command == "backup" => backup_operator::run(rest),
        [command, option, value]
            if (command == "run" || command == "status") && option == "--data-dir" =>
        {
            let data_dir = PathBuf::from(value.as_str());
            let result = if command == "run" {
                runtime::run(&data_dir)
            } else {
                runtime::print_status(&data_dir)
            };
            match result {
                Ok(()) => ExitCode::SUCCESS,
                Err(error) => {
                    eprintln!("litecoworkd: {error}");
                    ExitCode::FAILURE
                }
            }
        }
        _ => {
            eprintln!("error: unsupported arguments.");
            eprintln!("Run 'litecoworkd --help' for usage.");
            ExitCode::from(2)
        }
    }
}
