//! Compatibility marker retained by window structs.
//!
//! Popup dismissal no longer depends on compositor focus events. The shared
//! modal surface owns explicit backdrop and Escape dismissal instead.

#[derive(Debug, Default, Clone)]
pub struct FocusLossGuard;

impl FocusLossGuard {
    pub fn new() -> Self {
        Self
    }
}
