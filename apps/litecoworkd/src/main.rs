use std::process::ExitCode;

const HELP: &str = "litecoworkd — LiteCowork local Runtime\n\nUSAGE:\n    litecoworkd [--help | --version]\n\nThis foundation build only reports help and version. It does not start a listener,\nopen a database, or launch an agent.\n";

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

    eprintln!("error: unsupported arguments.");
    eprintln!("Run 'litecoworkd --help' for usage.");
    ExitCode::from(2)
}
