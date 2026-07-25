//! MPRIS helpers via `playerctl`.

use crate::command;
use crate::Result;
use std::fs;
use std::io::Read;
use std::process::{Command, Stdio};

const FIELD_SEPARATOR: char = '\u{1f}';
const MAX_ARTWORK_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MediaState {
    pub status: String,
    pub title: String,
    pub artist: String,
    pub artwork_url: String,
}

impl MediaState {
    pub fn has_track(&self) -> bool {
        !self.title.is_empty() || !self.artist.is_empty()
    }

    pub fn playing(&self) -> bool {
        self.status.eq_ignore_ascii_case("Playing")
    }
}

pub fn parse_media_record(line: &str) -> Option<MediaState> {
    let mut parts = line.trim_end_matches('\n').splitn(4, FIELD_SEPARATOR);
    let status = parts.next()?.to_string();
    let title = parts.next()?.to_string();
    let artist = parts.next()?.to_string();
    let artwork_url = parts.next()?.to_string();
    Some(MediaState {
        status,
        title,
        artist,
        artwork_url,
    })
}

#[derive(Debug, Default, Clone)]
pub struct MediaBackend;

impl MediaBackend {
    pub fn new() -> Self {
        Self
    }

    pub fn metadata_format() -> String {
        [
            "{{status}}",
            "{{title}}",
            "{{artist}}",
            "{{mpris:artUrl}}",
        ]
        .join(&FIELD_SEPARATOR.to_string())
    }

    pub fn current(&self) -> Result<Option<MediaState>> {
        let result = command::run(
            &[
                "playerctl",
                "metadata",
                "--format",
                &Self::metadata_format(),
            ],
            false,
        )?;
        if !result.ok() {
            return Ok(None);
        }
        Ok(parse_media_record(result.stdout.trim_end()))
    }

    pub fn command(&self, action: &str) -> Result<()> {
        match action {
            "previous" | "play-pause" | "next" => {}
            other => {
                return Err(crate::Error::Message(format!(
                    "unsupported media action: {other}"
                )))
            }
        }
        command::run(&["playerctl", action], true)?;
        Ok(())
    }

    pub fn fetch_artwork(&self, url: &str) -> Result<Vec<u8>> {
        let url = url.trim();
        if url.is_empty() {
            return Err(crate::Error::Message("empty artwork url".into()));
        }
        if let Some(path) = url.strip_prefix("file://") {
            let path = percent_decode(path);
            let mut file = fs::File::open(path)?;
            let mut data = Vec::new();
            file.by_ref()
                .take(MAX_ARTWORK_BYTES as u64 + 1)
                .read_to_end(&mut data)?;
            if data.len() > MAX_ARTWORK_BYTES {
                return Err(crate::Error::Message("media artwork exceeds 2 MiB".into()));
            }
            return Ok(data);
        }
        if url.starts_with("http://") || url.starts_with("https://") {
            let output = Command::new("curl")
                .args(["-fsSL", "--max-time", "4", "-A", "CostaUtils/1.0", url])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .map_err(|source| crate::Error::CommandSpawn {
                    argv: vec!["curl".into(), url.into()],
                    source,
                })?;
            if !output.status.success() {
                return Err(crate::Error::CommandFailed {
                    argv: vec!["curl".into(), url.into()],
                    code: output.status.code().unwrap_or(-1),
                    stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
                });
            }
            if output.stdout.len() > MAX_ARTWORK_BYTES {
                return Err(crate::Error::Message("media artwork exceeds 2 MiB".into()));
            }
            return Ok(output.stdout);
        }
        Err(crate::Error::Message(format!(
            "unsupported artwork URL scheme: {url}"
        )))
    }
}

fn percent_decode(path: &str) -> String {
    let bytes = path.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (from_hex(bytes[i + 1]), from_hex(bytes[i + 2])) {
                out.push((h << 4) | l);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn from_hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_playerctl_record() {
        let line = format!(
            "Playing{FIELD_SEPARATOR}Song{FIELD_SEPARATOR}Artist{FIELD_SEPARATOR}file:///art.png"
        );
        let state = parse_media_record(&line).unwrap();
        assert!(state.playing());
        assert_eq!(state.title, "Song");
    }
}
