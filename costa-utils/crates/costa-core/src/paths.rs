//! XDG-aware locations shared by screenshot capture and management.

use std::fs;
use std::path::{Path, PathBuf};

pub const DEFAULT_SCREENSHOT_SETTING: &str = "~/Pictures/Screenshots";

pub fn pictures_directory() -> PathBuf {
    if let Some(path) = read_xdg_user_dir("XDG_PICTURES_DIR") {
        return path;
    }
    dirs::picture_dir().unwrap_or_else(|| home_dir().join("Pictures"))
}

pub fn screenshot_directory(setting: Option<&str>) -> PathBuf {
    match setting {
        None | Some(DEFAULT_SCREENSHOT_SETTING) => pictures_directory().join("Screenshots"),
        Some(raw) => expand_path(raw),
    }
}

fn home_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"))
}

fn expand_path(raw: &str) -> PathBuf {
    let with_home = if let Some(rest) = raw.strip_prefix("~/") {
        home_dir().join(rest)
    } else if raw == "~" {
        home_dir()
    } else {
        PathBuf::from(raw)
    };
    fs::canonicalize(&with_home).unwrap_or(with_home)
}

fn read_xdg_user_dir(key: &str) -> Option<PathBuf> {
    let config_home = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir().join(".config"));
    let user_dirs = config_home.join("user-dirs.dirs");
    let contents = fs::read_to_string(user_dirs).ok()?;
    for line in contents.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix(&format!("{key}=\"")) else {
            continue;
        };
        let Some(value) = rest.strip_suffix('"') else {
            continue;
        };
        let expanded = value.replace("$HOME", &home_dir().to_string_lossy());
        return Some(Path::new(&expanded).to_path_buf());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_screenshot_dir_under_pictures() {
        let dir = screenshot_directory(None);
        assert!(dir.ends_with("Screenshots"));
    }
}
