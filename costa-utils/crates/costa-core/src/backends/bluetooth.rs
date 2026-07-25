//! BlueZ operations via `bluetoothctl` (MVP; pairing agent can come later).

use crate::command;
use crate::Result;

#[derive(Debug, Clone)]
pub struct BtDevice {
    pub address: String,
    pub name: String,
    pub connected: bool,
    pub paired: bool,
}

#[derive(Debug, Clone)]
pub struct BluetoothState {
    pub powered: bool,
    pub discovering: bool,
    pub devices: Vec<BtDevice>,
}

#[derive(Debug, Default, Clone)]
pub struct BluetoothBackend;

impl BluetoothBackend {
    pub fn new() -> Self {
        Self
    }

    pub fn query(&self) -> Result<BluetoothState> {
        let show = command::run(&["bluetoothctl", "show"], false)?;
        let powered = show.stdout.lines().any(|line| {
            let line = line.trim();
            line.starts_with("Powered:") && line.contains("yes")
        });
        let discovering = show.stdout.lines().any(|line| {
            let line = line.trim();
            line.starts_with("Discovering:") && line.contains("yes")
        });

        let paired = Self::parse_devices(&command::run(&["bluetoothctl", "devices", "Paired"], false)?.stdout, true);
        let mut by_addr: std::collections::BTreeMap<String, BtDevice> = paired
            .into_iter()
            .map(|d| (d.address.clone(), d))
            .collect();
        for device in Self::parse_devices(
            &command::run(&["bluetoothctl", "devices"], false)?.stdout,
            false,
        ) {
            by_addr.entry(device.address.clone()).or_insert(device);
        }

        let connected_addrs: std::collections::HashSet<String> =
            Self::parse_devices(
                &command::run(&["bluetoothctl", "devices", "Connected"], false)?.stdout,
                true,
            )
            .into_iter()
            .map(|d| d.address)
            .collect();

        let mut devices: Vec<_> = by_addr.into_values().collect();
        for device in &mut devices {
            device.connected = connected_addrs.contains(&device.address);
        }
        devices.sort_by(|a, b| {
            (!a.connected, !a.paired, a.name.to_ascii_lowercase(), &a.address).cmp(&(
                !b.connected,
                !b.paired,
                b.name.to_ascii_lowercase(),
                &b.address,
            ))
        });

        Ok(BluetoothState {
            powered,
            discovering,
            devices,
        })
    }

    pub fn set_power(&self, powered: bool) -> Result<()> {
        command::run(
            &["bluetoothctl", "power", if powered { "on" } else { "off" }],
            true,
        )?;
        Ok(())
    }

    pub fn start_discovery(&self) -> Result<()> {
        let _ = command::run(&["bluetoothctl", "scan", "on"], false);
        Ok(())
    }

    pub fn stop_discovery(&self) -> Result<()> {
        let _ = command::run(&["bluetoothctl", "scan", "off"], false);
        Ok(())
    }

    pub fn connect(&self, address: &str) -> Result<()> {
        let _ = command::run(&["bluetoothctl", "trust", address], false);
        command::run(&["bluetoothctl", "connect", address], true)?;
        Ok(())
    }

    pub fn disconnect(&self, address: &str) -> Result<()> {
        command::run(&["bluetoothctl", "disconnect", address], true)?;
        Ok(())
    }

    pub fn pair(&self, address: &str) -> Result<()> {
        command::run(&["bluetoothctl", "pair", address], true)?;
        Ok(())
    }

    fn parse_devices(stdout: &str, paired: bool) -> Vec<BtDevice> {
        let mut devices = Vec::new();
        for line in stdout.lines() {
            let line = line.trim();
            let Some(rest) = line.strip_prefix("Device ") else {
                continue;
            };
            let mut parts = rest.splitn(2, ' ');
            let Some(address) = parts.next() else {
                continue;
            };
            let name = parts.next().unwrap_or(address).to_string();
            devices.push(BtDevice {
                address: address.to_string(),
                name,
                connected: false,
                paired,
            });
        }
        devices
    }
}
