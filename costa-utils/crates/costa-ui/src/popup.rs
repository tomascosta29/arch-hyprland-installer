//! Shared popup lifecycle: claim focus, Esc, hide-on-close, and focus-loss dismiss.

use crate::focus_guard::{FocusLossGuard, LAUNCH_GESTURE_MS};
use adw::prelude::*;
use glib::clone;
use gtk4::{gdk, glib};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::OnceLock;
use std::time::Instant;
use tracing::debug;

pub fn install_popup_dismiss(window: &adw::ApplicationWindow, focus_guard: Rc<RefCell<FocusLossGuard>>) {
    let key = gtk4::EventControllerKey::new();
    key.connect_key_pressed(clone!(
        #[weak]
        window,
        #[upgrade_or]
        return glib::Propagation::Proceed,
        move |_, keyval, _, _| {
            if keyval == gdk::Key::Escape {
                window.set_visible(false);
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        }
    ));
    window.add_controller(key);

    window.connect_close_request(|win| {
        win.set_visible(false);
        glib::Propagation::Stop
    });

    window.connect_notify_local(
        Some("is-active"),
        clone!(
            #[strong]
            focus_guard,
            #[weak]
            window,
            move |win, _| {
                if !focus_guard.borrow_mut().should_hide(win.is_active()) {
                    return;
                }
                let gen = focus_guard.borrow().generation.get();
                let focus_guard = focus_guard.clone();
                // Confirm at the next idle turn. A transient inactive→active
                // pair in the same compositor update invalidates this check,
                // while genuine outside clicks close without a visible delay.
                glib::idle_add_local_once(
                    clone!(
                        #[weak]
                        window,
                        #[strong]
                        focus_guard,
                        move || {
                            if focus_guard.borrow().generation.get() != gen {
                                return;
                            }
                            if window.is_visible()
                                && focus_guard.borrow_mut().should_hide(window.is_active())
                            {
                                window.set_visible(false);
                            }
                        }
                    ),
                );
            }
        ),
    );

    window.connect_notify_local(
        Some("visible"),
        clone!(
            #[strong]
            focus_guard,
            move |win, _| {
                focus_guard.borrow_mut().visibility_changed(win.is_visible());
            }
        ),
    );
}

pub fn present_popup(window: &adw::ApplicationWindow, focus_guard: &RefCell<FocusLossGuard>) {
    focus_guard.borrow_mut().presented();
    sync_popup_size(window);
    install_size_logger(window);
    window.present();
    // A bar click can return focus on button-release after the surface maps.
    // Reclaim it once when that launch gesture is over; later focus loss is a
    // genuine outside interaction and dismisses normally.
    glib::timeout_add_local_once(
        std::time::Duration::from_millis(LAUNCH_GESTURE_MS),
        clone!(
            #[weak]
            window,
            move || {
                if window.is_visible() && !window.is_active() {
                    window.present();
                }
            }
        ),
    );
}

/// Measure content and pin default + size-request to the final natural size
/// *before* the first mapped frame. Undersized defaults (e.g. 480×420 for a
/// 584×524 power menu) otherwise map, grow, and get re-centered by Hyprland —
/// which looks like a different window swapping in.
fn sync_popup_size(window: &adw::ApplicationWindow) {
    gtk4::prelude::WidgetExt::realize(window);
    if let Some(content) = window.content() {
        content.queue_allocate();
    }
    let (_min, natural) = window.preferred_size();
    let (default_w, default_h) = window.default_size();
    let width = natural.width().max(default_w).max(1);
    let height = natural.height().max(default_h).max(1);
    window.set_default_size(width, height);
    window.set_size_request(width, height);
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

    // Sample a few frames after present — catches allocate settles that do not
    // bump default-width/height.
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
