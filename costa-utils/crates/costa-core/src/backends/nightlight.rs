//! Explicit manual night-light state via hyprsunset.

use crate::command;
use crate::Result;
use std::sync::{Arc, Mutex};

const NIGHT_TEMP: i32 = 4500;
const DAY_TEMP: i32 = 6500;
const ENABLED_BELOW: f64 = 6000.0;

#[derive(Debug, Clone)]
pub struct NightLightBackend {
    temperature: i32,
    /// Shared across clones so toggle + refresh see the same override.
    enabled: Arc<Mutex<Option<bool>>>,
}

impl Default for NightLightBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl NightLightBackend {
    pub fn new() -> Self {
        Self {
            temperature: NIGHT_TEMP,
            enabled: Arc::new(Mutex::new(None)),
        }
    }

    pub fn set_enabled(&self, enabled: bool) -> Result<bool> {
        // `identity` resets the CTM but leaves `temperature` reporting the last
        // kelvin value, so query() would still think night light is on. Set an
        // explicit day temperature when disabling.
        if enabled {
            command::run(
                &[
                    "hyprctl",
                    "hyprsunset",
                    "temperature",
                    &self.temperature.to_string(),
                ],
                true,
            )?;
        } else {
            command::run(
                &["hyprctl", "hyprsunset", "temperature", &DAY_TEMP.to_string()],
                true,
            )?;
        }
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
        let temperature = result.stdout.trim().parse::<f64>().unwrap_or(DAY_TEMP as f64);
        let enabled = result.ok() && temperature < ENABLED_BELOW;
        *guard = Some(enabled);
        Ok(enabled)
    }
}
