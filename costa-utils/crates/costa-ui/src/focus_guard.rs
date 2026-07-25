//! Deterministic popup focus lifecycle.
//!
//! A popup must first become active before focus loss can dismiss it. This
//! ignores harmless inactive notifications emitted while a Wayland surface is
//! mapping without relying on timing or mouse position.

use std::cell::Cell;
use std::time::{Duration, Instant};

pub const LAUNCH_GESTURE_MS: u64 = 200;
const LAUNCH_GESTURE: Duration = Duration::from_millis(LAUNCH_GESTURE_MS);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FocusState {
    Hidden,
    AwaitingFocus,
    Focused,
}

#[derive(Debug)]
pub struct FocusLossGuard {
    state: FocusState,
    ignore_loss_until: Option<Instant>,
    /// Bumped whenever a pending focus-loss check must be invalidated.
    pub generation: Cell<u64>,
}

impl Default for FocusLossGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for FocusLossGuard {
    fn clone(&self) -> Self {
        Self {
            state: self.state,
            ignore_loss_until: self.ignore_loss_until,
            generation: Cell::new(self.generation.get()),
        }
    }
}

impl FocusLossGuard {
    pub fn new() -> Self {
        Self {
            state: FocusState::Hidden,
            ignore_loss_until: None,
            generation: Cell::new(0),
        }
    }

    pub fn presented(&mut self) {
        self.state = FocusState::AwaitingFocus;
        self.ignore_loss_until = Some(Instant::now() + LAUNCH_GESTURE);
        self.bump_generation();
    }

    pub fn visibility_changed(&mut self, visible: bool) {
        if !visible {
            self.state = FocusState::Hidden;
            self.ignore_loss_until = None;
            self.bump_generation();
        }
    }

    /// Returns true once a popup that genuinely held focus becomes inactive.
    pub fn should_hide(&mut self, active: bool) -> bool {
        if active {
            self.state = FocusState::Focused;
            self.bump_generation();
            return false;
        }

        if self
            .ignore_loss_until
            .is_some_and(|until| Instant::now() < until)
        {
            return false;
        }
        self.ignore_loss_until = None;
        self.state == FocusState::Focused
    }

    fn bump_generation(&self) {
        self.generation
            .set(self.generation.get().wrapping_add(1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignores_inactive_mapping_noise_until_focused() {
        let mut guard = FocusLossGuard::new();
        guard.presented();
        assert!(!guard.should_hide(false));
        assert!(!guard.should_hide(false));
        assert!(!guard.should_hide(true));
        guard.ignore_loss_until = None;
        assert!(guard.should_hide(false));
    }

    #[test]
    fn focus_regain_invalidates_pending_dismiss() {
        let mut guard = FocusLossGuard::new();
        guard.presented();
        assert!(!guard.should_hide(true));
        guard.ignore_loss_until = None;
        assert!(guard.should_hide(false));
        let pending_generation = guard.generation.get();
        assert!(!guard.should_hide(true));
        assert_ne!(guard.generation.get(), pending_generation);
    }

    #[test]
    fn hidden_popup_cannot_be_dismissed_again() {
        let mut guard = FocusLossGuard::new();
        guard.presented();
        assert!(!guard.should_hide(true));
        guard.visibility_changed(false);
        assert!(!guard.should_hide(false));
    }
}
