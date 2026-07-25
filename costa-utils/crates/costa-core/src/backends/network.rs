//! NetworkManager operations with BSSID-aware Wi-Fi identities.

use crate::command;
use crate::Result;
use std::collections::BTreeMap;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct WifiNetwork {
    pub ssid: String,
    pub bssid: String,
    pub signal: i32,
    pub active: bool,
    pub security: String,
    pub bars: String,
}

#[derive(Debug, Clone)]
pub struct WifiState {
    pub enabled: bool,
    pub networks: Vec<WifiNetwork>,
}

#[derive(Debug, Clone)]
pub struct WifiProfile {
    pub name: String,
    pub uuid: String,
    pub ssid: String,
    pub bssid: String,
}

#[derive(Debug, Default, Clone)]
pub struct NetworkBackend;

impl NetworkBackend {
    pub fn new() -> Self {
        Self
    }

    pub fn radio_enabled(&self) -> Result<bool> {
        let result = command::run(&["nmcli", "radio", "wifi"], true)?;
        Ok(result.stdout.trim() == "enabled")
    }

    pub fn set_radio(&self, enabled: bool) -> Result<()> {
        command::run(
            &["nmcli", "radio", "wifi", if enabled { "on" } else { "off" }],
            true,
        )?;
        Ok(())
    }

    pub fn active_status(&self) -> Result<(bool, String)> {
        let enabled = self.radio_enabled()?;
        if !enabled {
            return Ok((false, "Disabled".into()));
        }
        let result = command::run_with_timeout(
            &[
                "nmcli",
                "--terse",
                "--escape",
                "yes",
                "--fields",
                "IN-USE,SSID",
                "device",
                "wifi",
            ],
            Duration::from_secs(8),
            false,
        )?;
        for line in result.stdout.lines() {
            let fields = parse_nmcli_terse(line);
            if fields.len() >= 2 && fields[0] == "*" {
                let ssid = if fields[1].is_empty() {
                    "Connected".into()
                } else {
                    fields[1].clone()
                };
                return Ok((true, ssid));
            }
        }
        Ok((false, "Disconnected".into()))
    }

    pub fn scan(&self) -> Result<WifiState> {
        let enabled = self.radio_enabled()?;
        if !enabled {
            return Ok(WifiState {
                enabled: false,
                networks: Vec::new(),
            });
        }
        let _ = command::run_with_timeout(&["nmcli", "device", "wifi", "rescan"], Duration::from_secs(10), false);
        let result = command::run_with_timeout(
            &[
                "nmcli",
                "--terse",
                "--escape",
                "yes",
                "--fields",
                "SSID,BSSID,SIGNAL,IN-USE,SECURITY,BARS",
                "device",
                "wifi",
                "list",
            ],
            Duration::from_secs(12),
            true,
        )?;
        let mut networks: BTreeMap<String, WifiNetwork> = BTreeMap::new();
        for line in result.stdout.lines() {
            let fields = parse_nmcli_terse(line);
            if fields.len() < 6 || fields[0].is_empty() {
                continue;
            }
            let ssid = fields[0].clone();
            let bssid = fields[1].clone();
            let signal = fields[2].parse::<i32>().unwrap_or(0);
            let active = fields[3] == "*";
            let security = fields[4].trim().to_string();
            let bars = fields[5].clone();
            let key = if bssid.is_empty() {
                format!("{ssid}:{signal}")
            } else {
                bssid.clone()
            };
            networks.insert(
                key,
                WifiNetwork {
                    ssid,
                    bssid,
                    signal,
                    active,
                    security,
                    bars,
                },
            );
        }
        let mut ordered: Vec<_> = networks.into_values().collect();
        ordered.sort_by(|a, b| {
            (!a.active, a.ssid.to_ascii_lowercase(), -a.signal, &a.bssid)
                .cmp(&(!b.active, b.ssid.to_ascii_lowercase(), -b.signal, &b.bssid))
        });
        Ok(WifiState {
            enabled: true,
            networks: ordered,
        })
    }

    pub fn saved_profiles(&self) -> Result<Vec<WifiProfile>> {
        let result = command::run(
            &[
                "nmcli",
                "--terse",
                "--escape",
                "yes",
                "--fields",
                "NAME,UUID,802-11-wireless.ssid,802-11-wireless.bssid",
                "connection",
                "show",
            ],
            true,
        )?;
        let mut profiles = Vec::new();
        for line in result.stdout.lines() {
            let fields = parse_nmcli_terse(line);
            if fields.len() >= 4 && !fields[2].is_empty() {
                profiles.push(WifiProfile {
                    name: fields[0].clone(),
                    uuid: fields[1].clone(),
                    ssid: fields[2].clone(),
                    bssid: fields[3].clone(),
                });
            }
        }
        Ok(profiles)
    }

    pub fn connect_saved(&self, uuid: &str) -> Result<String> {
        Ok(command::run_with_timeout(
            &["nmcli", "connection", "up", "uuid", uuid],
            Duration::from_secs(35),
            true,
        )?
        .stdout)
    }

    pub fn connect_open(&self, ssid: &str, bssid: &str) -> Result<String> {
        let mut argv = vec!["nmcli", "device", "wifi", "connect", ssid];
        if !bssid.is_empty() {
            argv.extend(["bssid", bssid]);
        }
        Ok(command::run_with_timeout(&argv, Duration::from_secs(35), true)?.stdout)
    }

    pub fn connect_personal(&self, ssid: &str, bssid: &str, password: &str) -> Result<String> {
        let mut argv = vec!["nmcli", "--ask", "device", "wifi", "connect", ssid];
        if !bssid.is_empty() {
            argv.extend(["bssid", bssid]);
        }
        let input = format!("{password}\n");
        Ok(command::run_with_input(&argv, Some(&input), Duration::from_secs(35), true)?.stdout)
    }
}

pub fn parse_nmcli_terse(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut escaped = false;
    for char in line.chars() {
        if escaped {
            current.push(char);
            escaped = false;
        } else if char == '\\' {
            escaped = true;
        } else if char == ':' {
            fields.push(std::mem::take(&mut current));
        } else {
            current.push(char);
        }
    }
    if escaped {
        current.push('\\');
    }
    fields.push(current);
    fields
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_escaped_nmcli() {
        let fields = parse_nmcli_terse(r"Cafe\:Net:AA\:BB:70:*:WPA2:▂▄▆_");
        assert_eq!(fields[0], "Cafe:Net");
        assert_eq!(fields[1], "AA:BB");
        assert_eq!(fields[3], "*");
    }
}
