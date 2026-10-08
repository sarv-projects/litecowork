#[cfg(target_os = "linux")]
fn main() {
    use std::{
        env,
        io::{self, Read, Write},
        os::unix::process::CommandExt,
        process::Command,
    };

    const READY: &[u8] = b"LITECOWORK_SCOPE_GATE_V1\n";
    const GO_CONSUMED: &[u8] = b"LITECOWORK_SCOPE_GO_CONSUMED_V1\n";
    const GO: &[u8] = b"GO\n";

    let mut arguments = env::args_os().skip(1);
    let Some(executable) = arguments.next() else {
        eprintln!("LiteCowork scope gate: executable missing");
        std::process::exit(126);
    };
    if !std::path::Path::new(&executable).is_absolute() {
        eprintln!("LiteCowork scope gate: executable must be absolute");
        std::process::exit(126);
    }

    if io::stdout().write_all(READY).and_then(|_| io::stdout().flush()).is_err() {
        std::process::exit(126);
    }
    let mut go = [0; GO.len()];
    if io::stdin().read_exact(&mut go).is_err() || go != *GO {
        eprintln!("LiteCowork scope gate: GO handshake missing or invalid");
        std::process::exit(126);
    }
    // This marker proves only that the gate consumed exactly GO. It is emitted before
    // attempting exec; the parent waits for it before exposing stdin to the caller, so
    // agent protocol bytes cannot be prefetched by the gate's handshake read.
    if io::stdout().write_all(GO_CONSUMED).and_then(|_| io::stdout().flush()).is_err() {
        std::process::exit(126);
    }

    let error = Command::new(executable).args(arguments).exec();
    eprintln!("LiteCowork scope gate: agent exec failed: {error}");
    std::process::exit(126);
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("LiteCowork scope gate is unsupported on this platform");
    std::process::exit(126);
}
