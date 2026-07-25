//! Screenshot capture via grim / slurp / hyprctl.

use crate::command;
use crate::paths::{screenshot_directory, DEFAULT_SCREENSHOT_SETTING};
use crate::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureMode {
    Full,
    Area,
    Window,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BlinkerConfig {
    #[serde(default = "default_dir")]
    pub screenshot_dir: String,
    #[serde(default = "default_pattern")]
    pub naming_pattern: String,
    #[serde(default = "default_true")]
    pub copy_to_clipboard: bool,
    #[serde(default = "default_true")]
    pub show_notification: bool,
    #[serde(default = "default_true")]
    pub open_manager_after_capture: bool,
}

fn default_dir() -> String {
    DEFAULT_SCREENSHOT_SETTING.into()
}
fn default_pattern() -> String {
    "Screenshot_%Y%m%d_%H%M%S".into()
}
fn default_true() -> bool {
    true
}

impl Default for BlinkerConfig {
    fn default() -> Self {
        Self {
            screenshot_dir: default_dir(),
            naming_pattern: default_pattern(),
            copy_to_clipboard: true,
            show_notification: true,
            open_manager_after_capture: true,
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct BlinkerBackend;

impl BlinkerBackend {
    pub fn new() -> Self {
        Self
    }

    pub fn load_config(&self) -> BlinkerConfig {
        let path = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("blinker/settings.json");
        fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn save_config(&self, config: &BlinkerConfig) -> Result<()> {
        let path = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("blinker/settings.json");
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, serde_json::to_string_pretty(config)?)?;
        Ok(())
    }

    pub fn recent_screenshots(&self, count: usize) -> Vec<PathBuf> {
        let config = self.load_config();
        let dir = screenshot_directory(Some(&config.screenshot_dir));
        let Ok(entries) = fs::read_dir(&dir) else {
            return Vec::new();
        };
        let mut files: Vec<_> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                matches!(
                    p.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()),
                    Some(ext) if matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "webp")
                )
            })
            .collect();
        files.sort_by_key(|p| std::cmp::Reverse(fs::metadata(p).and_then(|m| m.modified()).ok()));
        files.truncate(count);
        files
    }

    pub fn capture(&self, mode: CaptureMode) -> Result<PathBuf> {
        // Allow launcher hide animation to finish.
        thread::sleep(Duration::from_millis(250));
        let config = self.load_config();
        let dir = screenshot_directory(Some(&config.screenshot_dir));
        fs::create_dir_all(&dir)?;
        let stem = format_timestamp(&config.naming_pattern);
        let path = unique_png_path(&dir, &stem);

        match mode {
            CaptureMode::Full => {
                command::run(&["grim", path.to_str().unwrap_or("screenshot.png")], true)?;
            }
            CaptureMode::Area => {
                let slurp = Command::new("slurp")
                    .args([
                        "-b",
                        "21293699",
                        "-c",
                        "7FB0DEff",
                        "-s",
                        "7FB0DE0D",
                        "-w",
                        "2",
                    ])
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null())
                    .output()?;
                if !slurp.status.success() {
                    return Err(crate::Error::Message("Selection cancelled".into()));
                }
                let geometry = String::from_utf8_lossy(&slurp.stdout).trim().to_string();
                command::run(
                    &[
                        "grim",
                        "-g",
                        &geometry,
                        path.to_str().unwrap_or("screenshot.png"),
                    ],
                    true,
                )?;
            }
            CaptureMode::Window => {
                let raw = command::run(&["hyprctl", "-j", "activewindow"], true)?;
                let window: Value = serde_json::from_str(&raw.stdout)?;
                let at = window.get("at").and_then(|v| v.as_array());
                let size = window.get("size").and_then(|v| v.as_array());
                let (Some(at), Some(size)) = (at, size) else {
                    return Err(crate::Error::Message("No active window geometry".into()));
                };
                if at.len() != 2 || size.len() != 2 {
                    return Err(crate::Error::Message("No active window geometry".into()));
                }
                let x = at[0].as_i64().unwrap_or(0);
                let y = at[1].as_i64().unwrap_or(0);
                let w = size[0].as_i64().unwrap_or(0);
                let h = size[1].as_i64().unwrap_or(0);
                if w <= 0 || h <= 0 {
                    return Err(crate::Error::Message("Invalid window size".into()));
                }
                let geometry = format!("{x},{y} {w}x{h}");
                command::run(
                    &[
                        "grim",
                        "-g",
                        &geometry,
                        path.to_str().unwrap_or("screenshot.png"),
                    ],
                    true,
                )?;
            }
        }

        if config.copy_to_clipboard {
            let _ = copy_image_file(&path);
        }
        if config.show_notification {
            let _ = command::spawn(&[
                "notify-send",
                "Blinker",
                &format!("Saved {}", path.file_name().and_then(|s| s.to_str()).unwrap_or("screenshot")),
            ]);
        }
        Ok(path)
    }

    pub fn copy_image(&self, path: &Path) -> Result<()> {
        copy_image_file(path)
    }
}

fn copy_image_file(path: &Path) -> Result<()> {
    let mime = if path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("png"))
    {
        "image/png"
    } else {
        "image/jpeg"
    };
    let file = fs::File::open(path)?;
    let mut child = Command::new("wl-copy")
        .args(["-t", mime])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|source| crate::Error::CommandSpawn {
            argv: vec!["wl-copy".into()],
            source,
        })?;
    if let Some(mut stdin) = child.stdin.take() {
        std::io::copy(&mut std::io::BufReader::new(file), &mut stdin)?;
    }
    let status = child.wait()?;
    if !status.success() {
        return Err(crate::Error::Message("wl-copy failed".into()));
    }
    Ok(())
}

fn unique_png_path(directory: &Path, stem: &str) -> PathBuf {
    let safe = Path::new(stem)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("Screenshot");
    let mut candidate = directory.join(format!("{safe}.png"));
    let mut suffix = 1;
    while candidate.exists() {
        candidate = directory.join(format!("{safe}_{suffix}.png"));
        suffix += 1;
    }
    candidate
}

fn format_timestamp(pattern: &str) -> String {
    // Minimal strftime subset used by the default pattern.
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // Prefer `date` for full patterns.
    if let Ok(output) = Command::new("date")
        .arg(format!("+{pattern}"))
        .output()
    {
        if output.status.success() {
            return String::from_utf8_lossy(&output.stdout).trim().to_string();
        }
    }
    format!("Screenshot_{now}")
}
