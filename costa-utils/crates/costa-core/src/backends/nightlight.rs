//! Explicit manual night-light state via hyprsunset.

use crate::command;
use crate::Result;
use std::sync::Mutex;

#[derive(Debug, Default)]
pub struct NightLightBackend {
    temperature: i32,
    enabled: Mutex<Option<bool>>,
}

impl Clone for NightLightBackend {
    fn clone(&self) -> Self {
        Self {
            temperature: self.temperature,
            enabled: Mutex::new(*self.enabled.lock().unwrap_or_else(|e| e.into_inner())),
        }
    }
}

impl NightLightBackend {
    pub fn new() -> Self {
        Self {
            temperature: 4500,
            enabled: Mutex::new(None),
        }
    }

    pub fn set_enabled(&self, enabled: bool) -> Result<bool> {
        let profile = if enabled {
            format!("temperature {}", self.temperature)
        } else {
            "identity".into()
        };
        command::run(&["hyprctl", "hyprsunset", &profile], true)?;
        *self.enabled.lock().unwrap_or_else(|e| e.into_inner()) = Some(enabled);
        Ok(enabled)
    }

    pub fn toggle(&self) -> Result<bool> {
        let current = self.query()?;
        self.set_enabled(!current)
    }

    pub fn query(&self) -> Result<bool> {
        let mut guard = self.enabled.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(enabled) = *guard {
            return Ok(enabled);
        }
        let result = command::run(&["hyprctl", "hyprsunset", "temperature"], false)?;
        let temperature = result.stdout.trim().parse::<f64>().unwrap_or(6500.0);
        let enabled = result.ok() && temperature < 6000.0;
        *guard = Some(enabled);
        Ok(enabled)
    }
}
