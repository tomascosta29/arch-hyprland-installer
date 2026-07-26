//! Shared floating popup lifecycle.

use crate::focus_guard::FocusLossGuard;
use adw::prelude::*;
use glib::clone;
use gtk4::{gdk, glib};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::OnceLock;
use std::time::Instant;
use tracing::{debug, info};

pub fn install_popup_dismiss(
    window: &adw::ApplicationWindow,
    _focus_guard: Rc<RefCell<FocusLossGuard>>,
) {
    let key = gtk4::EventControllerKey::new();
    key.connect_key_pressed(clone!(
        #[weak]
        window,
        #[upgrade_or]
        return glib::Propagation::Proceed,
        move |_, keyval, _, _| {
            if keyval == gdk::Key::Escape {
                info!(title = ?window.title(), reason = "escape", "popup dismissed");
                window.set_visible(false);
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        }
    ));
    window.add_controller(key);

    window.connect_close_request(|win| {
        info!(title = ?win.title(), reason = "close-request", "popup dismissed");
        win.set_visible(false);
        glib::Propagation::Stop
    });
}

pub fn present_popup(window: &adw::ApplicationWindow, _focus_guard: &RefCell<FocusLossGuard>) {
    info!(title = ?window.title(), "popup presented");
    crate::theme::install();
    sync_popup_size(window);
    install_size_logger(window);
    // Compact floating card only — Hyprland float+center rules place it.
    // No fullscreen dimmer; Escape / close dismiss.
    if window.is_fullscreen() {
        window.unfullscreen();
    }
    window.present();
}

/// Measure and pin the compact popup before its first mapped frame.
fn sync_popup_size(window: &adw::ApplicationWindow) {
    gtk4::prelude::WidgetExt::realize(window);
    if let Some(content) = window.content() {
        content.queue_allocate();
    }
    let (_min, natural) = window.preferred_size();
    let (default_w, default_h) = window.default_size();
    let width = natural.width().max(default_w).max(1);
    // Prefer content height when default height is unset/placeholder.
    let height = if default_h > 0 {
        natural.height().max(default_h).max(1)
    } else {
        natural.height().max(1)
    };
    window.set_default_size(width, height);
}

fn debug_size_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        matches!(
            std::env::var("COSTA_UTILS_DEBUG_SIZE").as_deref(),
            Ok("1") | Ok("true") | Ok("TRUE") | Ok("yes")
        )
    })
}

fn install_size_logger(window: &adw::ApplicationWindow) {
    if !debug_size_enabled() {
        return;
    }
    if unsafe { window.data::<Cell<bool>>("costa-size-logger").is_some() } {
        return;
    }
    unsafe {
        window.set_data("costa-size-logger", Cell::new(true));
    }

    let started = Instant::now();
    let last = Rc::new(Cell::new((-1i32, -1i32)));
    let title = window.title().unwrap_or_else(|| "window".into());

    let log_size = {
        let last = last.clone();
        let title = title.clone();
        move |win: &adw::ApplicationWindow| {
            let size = (win.width(), win.height());
            if size.0 <= 0 || size.1 <= 0 {
                return;
            }
            let prev = last.get();
            if prev != size {
                let ms = started.elapsed().as_secs_f64() * 1000.0;
                eprintln!(
                    "[costa-size] {title}: {prev:?} -> {size:?} at {ms:.1}ms (default={:?})",
                    win.default_size()
                );
                debug!(%title, ?prev, ?size, ms, "popup size change");
                last.set(size);
            }
        }
    };

    window.connect_default_width_notify(clone!(
        #[strong]
        log_size,
        move |win| log_size(win)
    ));
    window.connect_default_height_notify(clone!(
        #[strong]
        log_size,
        move |win| log_size(win)
    ));
    window.connect_map(clone!(
        #[strong]
        log_size,
        move |win| log_size(win)
    ));

    let win = window.clone();
    let log_size = Rc::new(log_size);
    glib::timeout_add_local(std::time::Duration::from_millis(0), {
        let win = win.clone();
        let log_size = log_size.clone();
        move || {
            log_size(&win);
            glib::ControlFlow::Break
        }
    });
    for delay_ms in [16u64, 33, 50, 100, 200] {
        let win = win.clone();
        let log_size = log_size.clone();
        glib::timeout_add_local(std::time::Duration::from_millis(delay_ms), move || {
            log_size(&win);
            glib::ControlFlow::Break
        });
    }
}
