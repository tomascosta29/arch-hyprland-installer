//! PipeWire / WirePlumber / PulseAudio helpers via `wpctl` and `pactl`.

use crate::command::{self, CommandResult};
use crate::Result;
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct AudioDevice {
    pub name: String,
    pub description: String,
    pub mute: bool,
    pub volume_percent: f64,
    pub media_class: String,
}

#[derive(Debug, Clone)]
pub struct AudioSnapshot {
    pub sinks: Vec<AudioDevice>,
    pub sources: Vec<AudioDevice>,
    pub default_sink: String,
    pub default_source: String,
}

#[derive(Debug, Default, Clone)]
pub struct AudioBackend;

impl AudioBackend {
    pub fn new() -> Self {
        Self
    }

    pub fn list_devices(&self) -> Result<AudioSnapshot> {
        let sinks = Self::parse_devices(&command::run(
            &["pactl", "-f", "json", "list", "sinks"],
            false,
        )?)?;
        let sources = Self::parse_devices(&command::run(
            &["pactl", "-f", "json", "list", "sources"],
            false,
        )?)?
        .into_iter()
        .filter(|device| !device.name.contains(".monitor"))
        .collect();
        let default_sink = command::run(&["pactl", "get-default-sink"], false)?
            .stdout
            .trim()
            .to_string();
        let default_source = command::run(&["pactl", "get-default-source"], false)?
            .stdout
            .trim()
            .to_string();
        Ok(AudioSnapshot {
            sinks,
            sources,
            default_sink,
            default_source,
        })
    }

    pub fn get_default_volume(&self, target: &str) -> Result<(u32, bool)> {
        let result = command::run(&["wpctl", "get-volume", target], false)?;
        if !result.ok() || !result.stdout.contains("Volume:") {
            return Ok((0, false));
        }
        let after = result
            .stdout
            .split_once(':')
            .map(|(_, rest)| rest.trim())
            .unwrap_or("");
        let mut parts = after.split_whitespace();
        let level = parts
            .next()
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(0.0);
        let muted = result.stdout.contains("[MUTED]");
        Ok(((level * 100.0).round() as u32, muted))
    }

    pub fn set_volume(&self, target: &str, percentage: f64) -> Result<()> {
        let value = format!("{:.3}", percentage.max(0.0) / 100.0);
        command::run(&["wpctl", "set-volume", target, &value], true)?;
        Ok(())
    }

    pub fn toggle_mute(&self, target: &str) -> Result<()> {
        command::run(&["wpctl", "set-mute", target, "toggle"], true)?;
        Ok(())
    }

    pub fn set_default_sink(&self, name: &str) -> Result<()> {
        command::run(&["pactl", "set-default-sink", name], true)?;
        Ok(())
    }

    pub fn set_default_source(&self, name: &str) -> Result<()> {
        command::run(&["pactl", "set-default-source", name], true)?;
        Ok(())
    }

    fn parse_devices(result: &CommandResult) -> Result<Vec<AudioDevice>> {
        if !result.ok() || result.stdout.trim().is_empty() {
            return Ok(Vec::new());
        }
        let value: Value = serde_json::from_str(&result.stdout)?;
        let items = match value {
            Value::Array(items) => items,
            Value::Object(_) => vec![value],
            _ => return Ok(Vec::new()),
        };
        Ok(items.into_iter().filter_map(Self::parse_device).collect())
    }

    fn parse_device(value: Value) -> Option<AudioDevice> {
        let obj = value.as_object()?;
        let name = obj.get("name")?.as_str()?.to_string();
        let description = obj
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or(&name)
            .to_string();
        let mute = obj.get("mute").and_then(|v| v.as_bool()).unwrap_or(false);
        let media_class = obj
            .get("properties")
            .and_then(|props| props.get("media.class"))
            .and_then(|v| v.as_str())
            .or_else(|| obj.get("media.class").and_then(|v| v.as_str()))
            .unwrap_or("")
            .to_string();
        Some(AudioDevice {
            name,
            description,
            mute,
            volume_percent: channel_volume_percent(&value),
            media_class,
        })
    }
}

pub fn channel_volume_percent(node: &Value) -> f64 {
    let Some(volume) = node.get("volume").and_then(|v| v.as_object()) else {
        return 0.0;
    };
    let channel = volume
        .get("front-left")
        .or_else(|| volume.values().next());
    let Some(channel) = channel else {
        return 0.0;
    };
    channel
        .get("value_percent")
        .and_then(|v| v.as_str())
        .map(|s| s.trim_end_matches('%').parse::<f64>().unwrap_or(0.0))
        .unwrap_or(0.0)
}
