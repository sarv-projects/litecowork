mod agents;
mod local_filesystem;
mod operator;
mod runtime;

use std::{path::PathBuf, process::ExitCode};

const HELP: &str = "litecoworkd — LiteCowork local Runtime\n\nUSAGE:\n    litecoworkd [--help | --version]\n    litecoworkd run --data-dir <private-directory>\n    litecoworkd status --data-dir <private-directory>\n\n`run` starts the local daemon, acquires the single-instance lock, opens durable storage,\nand serves the local Operator API over OS-authenticated Unix IPC on supported platforms.\nWorkspace creation is idempotent per local Principal and request key. This build does not accept work: the Runtime remains DEGRADED until Task recovery and execution services are available, and it does not launch agents. SIGINT/SIGTERM are process-supervisor shutdown signals; there is no authenticated Operator stop command yet.\n";

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
