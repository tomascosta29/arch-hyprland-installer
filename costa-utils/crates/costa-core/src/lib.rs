//! Shared, UI-free logic for Costa Utils.
//!
//! Backends talk to the desktop (PipeWire, NetworkManager, BlueZ, …) while
//! `costa-ui` owns GTK windows. Keep this crate free of GTK so it stays
//! testable and reusable from non-GUI contexts.

pub mod backends;
pub mod command;
pub mod error;
pub mod paths;
pub mod target;

pub use error::{Error, Result};
pub use target::Target;
