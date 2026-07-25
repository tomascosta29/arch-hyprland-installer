//! CLI / argv0 target resolution — keep Hyprland binds stable.

use crate::Error;
use std::path::Path;

/// Overlay / action the running app should present.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Target {
    AppMenu,
    Runner,
    Blinker,
    BlinkerArea,
    BlinkerManager,
    Clipper,
    PowerMenu,
    NetworkMenu,
    BluetoothMenu,
    VolumeMenu,
    ControlCenter,
    Shutdown,
}

impl Target {
    /// Canonical `--flag` form used by the Python original and desktop file.
    pub fn flag(self) -> &'static str {
        match self {
            Self::AppMenu => "--app-menu",
            Self::Runner => "--runner",
            Self::Blinker => "--blinker",
            Self::BlinkerArea => "--blinker-area",
            Self::BlinkerManager => "--blinker-manager",
            Self::Clipper => "--clipper",
            Self::PowerMenu => "--power-menu",
            Self::NetworkMenu => "--network-menu",
            Self::BluetoothMenu => "--bluetooth-menu",
            Self::VolumeMenu => "--volume-menu",
            Self::ControlCenter => "--control-center",
            Self::Shutdown => "--shutdown",
        }
    }

    pub fn all() -> &'static [Target] {
        &[
            Self::AppMenu,
            Self::Runner,
            Self::Blinker,
            Self::BlinkerArea,
            Self::BlinkerManager,
            Self::Clipper,
            Self::PowerMenu,
            Self::NetworkMenu,
            Self::BluetoothMenu,
            Self::VolumeMenu,
            Self::ControlCenter,
            Self::Shutdown,
        ]
    }

    /// Resolve `--app-menu`, `app-menu`, `appmenu`, etc.
    pub fn parse(value: &str) -> crate::Result<Self> {
        let key = value.trim();
        if key.is_empty() {
            return Err(Error::UnknownTarget(value.to_string()));
        }

        let normalized = key.to_ascii_lowercase();
        let matched = match normalized.as_str() {
            "--app-menu" | "app-menu" | "appmenu" => Self::AppMenu,
            "--runner" | "runner" => Self::Runner,
            "--blinker" | "blinker" => Self::Blinker,
            "--blinker-area" | "blinker-area" | "blinker_area" => Self::BlinkerArea,
            "--blinker-manager" | "blinker-manager" | "blinker_manager" => Self::BlinkerManager,
            "--clipper" | "clipper" => Self::Clipper,
            "--power-menu" | "power-menu" | "power_menu" | "power" => Self::PowerMenu,
            "--network-menu" | "network-menu" | "network_menu" | "network" => Self::NetworkMenu,
            "--bluetooth-menu" | "bluetooth-menu" | "bluetooth_menu" | "bluetooth" => {
                Self::BluetoothMenu
            }
            "--volume-menu" | "volume-menu" | "volume_menu" | "volume" => Self::VolumeMenu,
            "--control-center" | "control-center" | "control_center" | "control" => {
                Self::ControlCenter
            }
            "--shutdown" | "shutdown" => Self::Shutdown,
            _ => return Err(Error::UnknownTarget(value.to_string())),
        };
        Ok(matched)
    }

    /// Infer from argv0 basename (symlink aliases like `power-menu`).
    pub fn from_argv0(argv0: &str) -> Option<Self> {
        let name = Path::new(argv0)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(argv0);
        Self::parse(name).ok()
    }
}

pub const USAGE: &str = "Usage: costa-utils [--app-menu | --runner | --blinker | --blinker-area | --blinker-manager | --clipper | --power-menu | --network-menu | --bluetooth-menu | --volume-menu | --control-center | --shutdown]";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliMode {
    Help,
    Target(Target),
    None,
}

/// Parse `argv` the same way the Python entrypoint did.
pub fn parse_argv(argv: &[String]) -> crate::Result<CliMode> {
    if argv.len() > 1 {
        let raw = argv[1].as_str();
        if matches!(raw, "-h" | "--help") {
            return Ok(CliMode::Help);
        }
        return Ok(CliMode::Target(Target::parse(raw)?));
    }

    if let Some(inferred) = argv.first().and_then(|a| Target::from_argv0(a)) {
        return Ok(CliMode::Target(inferred));
    }

    Ok(CliMode::None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_flags_and_aliases() {
        assert_eq!(Target::parse("--power-menu").unwrap(), Target::PowerMenu);
        assert_eq!(Target::parse("power").unwrap(), Target::PowerMenu);
        assert_eq!(Target::parse("appmenu").unwrap(), Target::AppMenu);
        assert!(Target::parse("nope").is_err());
    }

    #[test]
    fn infers_from_argv0() {
        assert_eq!(
            Target::from_argv0("/usr/bin/clipper"),
            Some(Target::Clipper)
        );
    }
}
