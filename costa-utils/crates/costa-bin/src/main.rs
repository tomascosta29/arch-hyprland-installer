use anyhow::Context;
use costa_core::target::{parse_argv, CliMode, USAGE};
use tracing_subscriber::EnvFilter;

fn main() -> anyhow::Result<()> {
    let filter = std::env::var("COSTA_UTILS_LOG_LEVEL")
        .ok()
        .map(|level| EnvFilter::new(level.to_ascii_lowercase()))
        .or_else(|| EnvFilter::try_from_default_env().ok())
        .unwrap_or_else(|| EnvFilter::new("info"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();

    let argv: Vec<String> = std::env::args().collect();
    match parse_argv(&argv).context("parse arguments")? {
        CliMode::Help | CliMode::None => {
            println!("{USAGE}");
            Ok(())
        }
        CliMode::Target(target) => {
            let code = costa_ui::run(target);
            if code == 0 {
                Ok(())
            } else {
                std::process::exit(code);
            }
        }
    }
}
