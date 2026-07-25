//! App launcher filtering + runner history.

use serde_json::Value;
use std::fs;
use std::path::PathBuf;

const HIDDEN_APP_IDS: &[&str] = &[
    "org.fcosta.costautils",
    "avahi-discover",
    "bssh",
    "bvnc",
    "qv4l2",
    "qvidcap",
    "htop",
    "nvim",
    "nvim-qt",
    "cmake-gui",
    "electron",
    "nm-connection-editor",
    "pavucontrol",
    "org.pulseaudio.pavucontrol",
    "blueman-manager",
    "blueman-adapters",
    "chromium",
    "chromium-browser",
    "org.chromium.chromium",
    "google-chrome",
    "com.google.chrome",
    "brave-browser",
    "org.brave.browser",
    "microsoft-edge",
    "opera",
    "vivaldi-stable",
    "librewolf",
    "org.gnome.software",
    "org.freedesktop.xwayland",
    "xdvi",
    "lstopo",
    "xgps",
    "xgpsspeed",
    "rofi",
    "rofi-theme-selector",
];

const HIDDEN_PREFIXES: &[&str] = &[
    "org.gnome.settings",
    "org.gnome.systemmonitor",
    "gnome-system-monitor",
    "qv4l2",
];

pub fn normalize_app_id(app_id: &str) -> String {
    app_id
        .trim()
        .strip_suffix(".desktop")
        .unwrap_or(app_id)
        .to_ascii_lowercase()
}

pub fn should_list_app_id(app_id: &str, categories: &str) -> bool {
    let id = normalize_app_id(app_id);
    if id.is_empty() {
        return false;
    }
    if HIDDEN_APP_IDS.contains(&id.as_str()) {
        return false;
    }
    if HIDDEN_PREFIXES.iter().any(|prefix| id.starts_with(prefix)) {
        return false;
    }
    let categories = categories
        .split(';')
        .map(|c| c.trim().to_ascii_lowercase())
        .filter(|c| !c.is_empty())
        .collect::<Vec<_>>();
    if categories.iter().any(|c| c == "webbrowser")
        && id != "firefox"
        && id != "org.mozilla.firefox"
    {
        return false;
    }
    true
}

pub fn history_path() -> PathBuf {
    let state = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join(".local/state"));
    state.join("costa-utils/runner_history.json")
}

pub fn load_runner_history() -> Vec<String> {
    let Ok(text) = fs::read_to_string(history_path()) else {
        return Vec::new();
    };
    match serde_json::from_str::<Value>(&text) {
        Ok(Value::Array(items)) => items
            .into_iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect(),
        _ => Vec::new(),
    }
}

pub fn save_runner_history(history: &[String]) {
    let path = history_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let payload = serde_json::to_string(&history.iter().take(50).collect::<Vec<_>>())
        .unwrap_or_else(|_| "[]".into());
    let _ = fs::write(&path, payload);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
}

pub fn clear_runner_history() {
    let path = history_path();
    let _ = fs::write(&path, "[]");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
}

pub fn is_private_runner_command(cmd: &str) -> bool {
    cmd.starts_with(' ')
}

pub fn remember_runner_command(history: &mut Vec<String>, cmd: &str) {
    if is_private_runner_command(cmd) {
        return;
    }
    let cmd = cmd.trim();
    if cmd.is_empty() {
        return;
    }
    history.retain(|item| item != cmd);
    history.insert(0, cmd.to_string());
    save_runner_history(history);
}

pub fn evaluate_math(query: &str) -> Option<String> {
    if !query
        .chars()
        .all(|c| c.is_ascii_digit() || " +-*/().%".contains(c))
    {
        return None;
    }
    if !query.chars().any(|c| "+-*/%".contains(c)) || query.contains("**") {
        return None;
    }
    // Use python for safe-ish eval parity with the original tool.
    let output = std::process::Command::new("python3")
        .args([
            "-c",
            &format!(
                "import math\nprint(eval({query:?}, {{'__builtins__': None, 'math': math}}))"
            ),
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}
