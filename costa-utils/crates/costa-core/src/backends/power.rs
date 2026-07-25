//! Session power actions (lock / suspend / logout / reboot / poweroff).

use crate::command;
use crate::Result;
use std::env;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerAction {
    Lock,
    Suspend,
    Logout,
    Reboot,
    Shutdown,
}

impl PowerAction {
    pub fn id(self) -> &'static str {
        match self {
            Self::Lock => "lock",
            Self::Suspend => "suspend",
            Self::Logout => "logout",
            Self::Reboot => "reboot",
            Self::Shutdown => "shutdown",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Lock => "Lock",
            Self::Suspend => "Suspend",
            Self::Logout => "Log Out",
            Self::Reboot => "Reboot",
            Self::Shutdown => "Shutdown",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Self::Lock => "system-lock-screen-symbolic",
            Self::Suspend => "system-suspend-symbolic",
            Self::Logout => "system-log-out-symbolic",
            Self::Reboot => "system-reboot-symbolic",
            Self::Shutdown => "application-exit-symbolic",
        }
    }

    pub fn requires_confirm(self) -> bool {
        matches!(self, Self::Logout | Self::Reboot | Self::Shutdown)
    }

    pub fn all() -> &'static [PowerAction] {
        &[
            Self::Lock,
            Self::Suspend,
            Self::Logout,
            Self::Reboot,
            Self::Shutdown,
        ]
    }
}

#[derive(Debug, Default, Clone)]
pub struct PowerBackend;

impl PowerBackend {
    pub fn new() -> Self {
        Self
    }

    pub fn execute(&self, action: PowerAction) -> Result<()> {
        match action {
            PowerAction::Lock => command::spawn(&["loginctl", "lock-session"]),
            PowerAction::Suspend => command::spawn(&["systemctl", "suspend"]),
            PowerAction::Logout => {
                if let Ok(session_id) = env::var("XDG_SESSION_ID") {
                    command::spawn(&["loginctl", "terminate-session", &session_id])
                } else {
                    command::spawn(&["hyprctl", "dispatch", "exit"])
                }
            }
            PowerAction::Reboot => command::spawn(&["systemctl", "reboot"]),
            PowerAction::Shutdown => command::spawn(&["systemctl", "poweroff"]),
        }
    }
}
