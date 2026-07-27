mod audit;
mod command;
mod output;
mod scanner;

use anyhow::{bail, Result};

fn main() {
    if let Err(error) = run() {
        output::error(&format!("{error:#}"));
        std::process::exit(3);
    }
}

fn run() -> Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let action = args.first().map(String::as_str).unwrap_or("policy");
    let json = args.iter().any(|arg| arg == "--json");
    let verbose = args.iter().any(|arg| arg == "--verbose" || arg == "-v");
    let component = args
        .iter()
        .position(|arg| arg == "--component")
        .and_then(|index| args.get(index + 1))
        .map(String::as_str);

    let code = match action {
        "policy" => audit::check(json)?,
        "vulns" => scanner::scan(json, verbose, component)?,
        "sbom" => scanner::sbom(json)?,
        "help" | "--help" | "-h" => {
            output::help();
            0
        }
        "version" | "--version" | "-V" => {
            println!("pkg-audit {}", env!("CARGO_PKG_VERSION"));
            0
        }
        other => bail!("unknown command '{other}'; run pkg-audit --help"),
    };
    std::process::exit(code);
}
