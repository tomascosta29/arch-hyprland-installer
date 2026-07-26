//! Compatibility marker retained by the popup window structs.
//!
//! Dismissal is click-driven. Focus loss is not a usable signal under
//! focus-follows-mouse compositors such as Hyprland.

#[derive(Debug, Default, Clone)]
pub struct FocusLossGuard;

impl FocusLossGuard {
    pub fn new() -> Self {
        Self
    }
}
