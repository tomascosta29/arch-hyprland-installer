//! Shared modal popup lifecycle with explicit backdrop-click dismissal.

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
    install_modal_backdrop(window);
    sync_popup_size(window);
    install_size_logger(window);
    if let Some(backdrop) =
        unsafe { window.data::<adw::ApplicationWindow>("costa-modal-backdrop") }
    {
        unsafe { backdrop.as_ref() }.maximize();
        unsafe { backdrop.as_ref() }.present();
    }
    window.present();
}

/// Create a separate workspace-sized surface behind the compact popup. Keeping
/// the popup in its own toplevel preserves its original GTK allocation and CSS.
fn install_modal_backdrop(window: &adw::ApplicationWindow) {
    if unsafe {
        window
            .data::<adw::ApplicationWindow>("costa-modal-backdrop")
            .is_some()
    } {
        return;
    }

    let Some(application) = window.application() else {
        return;
    };

    let backdrop = adw::ApplicationWindow::builder()
        .application(&application)
        .title("Costa Modal Backdrop")
        .decorated(false)
        .resizable(true)
        .build();
    backdrop.add_css_class("costa-modal-backdrop-window");

    let click_target = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    click_target.set_hexpand(true);
    click_target.set_vexpand(true);
    backdrop.set_content(Some(&click_target));

    let click = gtk4::GestureClick::new();
    click.connect_released(clone!(
        #[weak]
        window,
        move |_, _, _, _| {
            info!(title = ?window.title(), reason = "backdrop-click", "popup dismissed");
            window.set_visible(false);
        }
    ));
    click_target.add_controller(click);

    backdrop.connect_close_request(clone!(
        #[weak]
        window,
        #[upgrade_or]
        glib::Propagation::Proceed,
        move |backdrop| {
            window.set_visible(false);
            backdrop.set_visible(false);
            glib::Propagation::Stop
        }
    ));
    window.connect_visible_notify(clone!(
        #[weak]
        backdrop,
        move |popup| {
            if !popup.is_visible() {
                backdrop.set_visible(false);
            }
        }
    ));
    window.set_transient_for(Some(&backdrop));
    install_modal_css();
    unsafe {
        window.set_data("costa-modal-backdrop", backdrop);
    }
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
    let height = natural.height().max(default_h).max(1);
    window.set_default_size(width, height);
    window.set_size_request(width, height);
}

fn install_modal_css() {
    static LOADED: std::sync::Once = std::sync::Once::new();
    LOADED.call_once(|| {
        let provider = gtk4::CssProvider::new();
        provider.load_from_string(
            r#"
            window.costa-modal-backdrop-window {
                background: alpha(black, 0.16);
            }
            "#,
        );
        if let Some(display) = gdk::Display::default() {
            gtk4::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk4::STYLE_PROVIDER_PRIORITY_USER,
            );
        }
    });
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
