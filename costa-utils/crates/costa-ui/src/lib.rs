//! GTK4 + libadwaita shell for Costa Utils overlays.

mod app;
mod artwork;
mod bluetooth_agent;
mod focus_guard;
mod popup;
mod task;
mod theme;
mod windows;

pub use app::{run, APPLICATION_ID};

pub const ACTIVATE_TARGET_ACTION: &str = "activate-target";
