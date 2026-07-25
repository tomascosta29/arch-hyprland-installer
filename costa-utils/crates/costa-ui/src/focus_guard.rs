//! Ignore inactive notifications until a newly shown window was focused.
//!
//! Bar / panel launches often flash `is-active` then immediately lose focus to
//! the click-release on the panel. Focus events during the present-grace must
//! not arm dismiss — only focus *after* the grace period counts.

use std::cell::Cell;
use std::time::{Duration, Instant};

const PRESENT_GRACE: Duration = Duration::from_millis(450);
pub const FOCUS_LOSS_DEBOUNCE_MS: u32 = 200;

#[derive(Debug)]
pub struct FocusLossGuard {
    /// True only after `is-active` while *outside* the present grace window.
    seen_focus_after_grace: bool,
    ignore_until: Option<Instant>,
    /// Generation bumped on present / regain-focus so stale hide timers no-op.
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
            seen_focus_after_grace: self.seen_focus_after_grace,
            ignore_until: self.ignore_until,
            generation: Cell::new(self.generation.get()),
        }
    }
}

impl FocusLossGuard {
    pub fn new() -> Self {
        Self {
            seen_focus_after_grace: false,
            ignore_until: None,
            generation: Cell::new(0),
        }
    }

    /// Call when the overlay is shown so bar-click focus races are ignored.
    pub fn presented(&mut self) {
        self.seen_focus_after_grace = false;
        self.ignore_until = Some(Instant::now() + PRESENT_GRACE);
        self.generation.set(self.generation.get().wrapping_add(1));
    }

    pub fn visibility_changed(&mut self, visible: bool) {
        if !visible {
            self.seen_focus_after_grace = false;
            self.ignore_until = None;
            self.generation.set(self.generation.get().wrapping_add(1));
        }
    }

    pub fn should_hide(&mut self, active: bool) -> bool {
        if let Some(until) = self.ignore_until {
            if Instant::now() < until {
                // Do not arm dismiss from focus flashes while opening.
                return false;
            }
            self.ignore_until = None;
        }

        if active {
            self.seen_focus_after_grace = true;
            self.generation.set(self.generation.get().wrapping_add(1));
            return false;
        }

        self.seen_focus_after_grace
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn waits_for_first_focus_after_grace() {
        let mut guard = FocusLossGuard::new();
        assert!(!guard.should_hide(false));
        assert!(!guard.should_hide(true));
        assert!(guard.should_hide(false));
        guard.visibility_changed(false);
        assert!(!guard.should_hide(false));
    }

    #[test]
    fn focus_during_grace_does_not_arm_dismiss() {
        let mut guard = FocusLossGuard::new();
        guard.presented();
        assert!(!guard.should_hide(true));
        assert!(!guard.should_hide(false));
        thread::sleep(PRESENT_GRACE + Duration::from_millis(20));
        // Still inactive after grace — never hide until focused post-grace.
        assert!(!guard.should_hide(false));
        assert!(!guard.should_hide(true));
        assert!(guard.should_hide(false));
    }
}
