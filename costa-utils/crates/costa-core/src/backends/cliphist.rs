//! cliphist list / decode / wipe helpers + pin state.

use crate::command;
use crate::Result;
use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

#[derive(Debug, Clone)]
pub struct ClipEntry {
    pub id: String,
    pub preview: String,
    pub is_image: bool,
}

#[derive(Debug, Default, Clone)]
pub struct ClipBackend;

impl ClipBackend {
    pub fn new() -> Self {
        Self
    }

    pub fn list(&self) -> Result<Vec<ClipEntry>> {
        let result = command::run(&["cliphist", "list"], false)?;
        if !result.ok() {
            return Ok(Vec::new());
        }
        Ok(result
            .stdout
            .lines()
            .filter_map(|line| {
                let (id, preview) = line.split_once('\t')?;
                let preview = preview.to_string();
                let is_image = preview.contains("[[ binary data")
                    || preview.to_ascii_lowercase().contains("image/");
                Some(ClipEntry {
                    id: id.to_string(),
                    preview,
                    is_image,
                })
            })
            .collect())
    }

    pub fn decode(&self, id: &str) -> Result<Vec<u8>> {
        let mut child = Command::new("cliphist")
            .args(["decode", id])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|source| crate::Error::CommandSpawn {
                argv: vec!["cliphist".into(), "decode".into(), id.into()],
                source,
            })?;
        let mut stdout = Vec::new();
        if let Some(mut out) = child.stdout.take() {
            std::io::Read::read_to_end(&mut out, &mut stdout)?;
        }
        let status = child.wait()?;
        if !status.success() {
            return Err(crate::Error::CommandFailed {
                argv: vec!["cliphist".into(), "decode".into(), id.into()],
                code: status.code().unwrap_or(-1),
                stderr: String::new(),
            });
        }
        Ok(stdout)
    }

    pub fn decode_text(&self, id: &str) -> Result<String> {
        let data = self.decode(id)?;
        Ok(String::from_utf8_lossy(&data).into_owned())
    }

    pub fn copy_id(&self, id: &str) -> Result<()> {
        let data = self.decode(id)?;
        self.copy_bytes(&data)
    }

    pub fn copy_text(&self, text: &str) -> Result<()> {
        self.copy_bytes(text.as_bytes())
    }

    pub fn copy_bytes(&self, data: &[u8]) -> Result<()> {
        let mime = detect_mime(data);
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
            stdin.write_all(data)?;
        }
        let status = child.wait()?;
        if !status.success() {
            return Err(crate::Error::Message("wl-copy failed".into()));
        }
        Ok(())
    }

    pub fn delete_id(&self, id: &str) -> Result<()> {
        // cliphist delete expects the list line on stdin.
        let line = format!("{id}\t");
        command::run_with_input(&["cliphist", "delete"], Some(&line), std::time::Duration::from_secs(5), true)?;
        Ok(())
    }

    pub fn wipe(&self) -> Result<()> {
        command::run(&["cliphist", "wipe"], true)?;
        Ok(())
    }

    /// Wipe history but re-seed pinned entries so they survive.
    pub fn wipe_preserving_pins(&self, pins: &HashSet<String>) -> Result<()> {
        let entries = self.list()?;
        let pinned_payloads: Vec<(String, Vec<u8>)> = entries
            .into_iter()
            .filter(|e| pins.contains(&e.id))
            .filter_map(|e| self.decode(&e.id).ok().map(|data| (e.id, data)))
            .collect();
        self.wipe()?;
        for (_id, data) in pinned_payloads {
            let _ = self.copy_bytes(&data);
            // Store back into cliphist via stdin.
            let mut child = Command::new("cliphist")
                .arg("store")
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .ok();
            if let Some(child) = child.as_mut() {
                if let Some(mut stdin) = child.stdin.take() {
                    let _ = stdin.write_all(&data);
                }
                let _ = child.wait();
            }
        }
        Ok(())
    }

    pub fn load_pins(&self) -> HashSet<String> {
        let path = pins_path();
        let legacy = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("clipper/pins");
        let path = if path.exists() {
            path
        } else if legacy.exists() {
            legacy
        } else {
            return HashSet::new();
        };
        fs::read_to_string(path)
            .map(|text| {
                text.lines()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn save_pins(&self, pins: &HashSet<String>) -> Result<()> {
        let path = pins_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut lines: Vec<_> = pins.iter().cloned().collect();
        lines.sort();
        fs::write(path, lines.join("\n") + if lines.is_empty() { "" } else { "\n" })?;
        Ok(())
    }
}

fn pins_path() -> PathBuf {
    dirs::state_dir()
        .or_else(dirs::data_local_dir)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("costa-utils/clipper/pins")
}

fn detect_mime(data: &[u8]) -> &'static str {
    if data.starts_with(b"\x89PNG\r\n\x1a\n") {
        "image/png"
    } else if data.starts_with(&[0xff, 0xd8, 0xff]) {
        "image/jpeg"
    } else if data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a") {
        "image/gif"
    } else if data.len() > 12 && data.starts_with(b"RIFF") && &data[8..12] == b"WEBP" {
        "image/webp"
    } else if std::str::from_utf8(data).is_ok() {
        "text/plain;charset=utf-8"
    } else {
        "application/octet-stream"
    }
}

/// Subsequence fuzzy match (case-insensitive).
pub fn fuzzy_match(query: &str, text: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    let haystack = text.to_ascii_lowercase();
    let mut it = haystack.chars();
    query
        .to_ascii_lowercase()
        .chars()
        .all(|c| it.any(|t| t == c))
}

/// True when text looks like an absolute/home path that exists.
pub fn looks_like_existing_path(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() || trimmed.contains('\n') {
        return false;
    }
    let path = if let Some(rest) = trimmed.strip_prefix("~/") {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("/"))
            .join(rest)
    } else {
        PathBuf::from(trimmed)
    };
    path.exists()
}
