use crate::focus_guard::FocusLossGuard;
use crate::popup::{install_popup_dismiss, present_popup};
use adw::prelude::*;
use gtk4::gdk;
use gtk4::glib;
use std::cell::RefCell;
use std::rc::Rc;
use tracing::info;

pub struct MonitorWindow {
    window: adw::ApplicationWindow,
    focus_guard: Rc<RefCell<FocusLossGuard>>,
}

impl MonitorWindow {
    pub fn new(app: &adw::Application) -> Self {
        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title("Monitor Layout")
            .default_width(520)
            .default_height(340)
            .resizable(false)
            .build();
        crate::theme::style_window(&window);

        let focus_guard = Rc::new(RefCell::new(FocusLossGuard::new()));

        let main_box = gtk4::Box::new(gtk4::Orientation::Vertical, 16);
        main_box.set_margin_top(16);
        main_box.set_margin_bottom(20);
        main_box.set_margin_start(20);
        main_box.set_margin_end(20);
        window.set_content(Some(&main_box));

        let header = crate::theme::header("Monitor Layout", "Select active display configuration");
        main_box.append(&header);

        let content_box = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
        content_box.set_vexpand(true);
        main_box.append(&content_box);

        let current_profile = detect_current_profile();

        let layouts = [
            (
                "dual",
                "Dual 1440p Layout",
                "DP-1 (Left, 180Hz) + HDMI-A-1 (Main Right, 144Hz)",
                "video-display-symbolic",
            ),
            (
                "single",
                "Single / VM Auto-Detect",
                "Auto-configured single display output for standalone screen or VM",
                "computer-symbolic",
            ),
        ];

        for (profile_id, name, desc, icon_name) in layouts {
            let is_active = current_profile == profile_id;

            let card = gtk4::Button::new();
            card.add_css_class("card-box");
            card.add_css_class("toggle-card");
            if is_active {
                card.add_css_class("toggle-card-active");
            }

            let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 16);
            row.set_valign(gtk4::Align::Center);

            let icon = gtk4::Image::from_icon_name(icon_name);
            icon.set_pixel_size(28);
            icon.add_css_class("toggle-icon");
            row.append(&icon);

            let text_box = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
            text_box.set_hexpand(true);

            let title_lbl = gtk4::Label::new(Some(name));
            title_lbl.add_css_class("toggle-title");
            title_lbl.set_halign(gtk4::Align::Start);

            let desc_lbl = gtk4::Label::new(Some(desc));
            desc_lbl.add_css_class("toggle-subtitle");
            desc_lbl.set_halign(gtk4::Align::Start);

            text_box.append(&title_lbl);
            text_box.append(&desc_lbl);
            row.append(&text_box);

            if is_active {
                let badge = gtk4::Label::new(Some("Active"));
                badge.add_css_class("preview-info-pill");
                row.append(&badge);
            }

            card.set_child(Some(&row));

            let window_weak = window.downgrade();
            let profile_id = profile_id.to_string();

            card.connect_clicked(move |_| {
                if let Some(win) = window_weak.upgrade() {
                    win.set_visible(false);
                }
                info!(profile = %profile_id, "applying monitor layout profile");
                let script_path = dirs_home_script("monitor-select");
                let _ = costa_core::command::spawn(&[script_path.as_str(), profile_id.as_str()]);
            });

            content_box.append(&card);
        }

        let hint = gtk4::Label::new(Some("Press Esc to cancel"));
        hint.add_css_class("dim-label");
        hint.set_halign(gtk4::Align::Center);
        main_box.append(&hint);

        let key = gtk4::EventControllerKey::new();
        key.set_propagation_phase(gtk4::PropagationPhase::Capture);
        let window_weak = window.downgrade();
        key.connect_key_pressed(move |_, keyval, _, _| {
            if keyval == gdk::Key::Escape {
                if let Some(win) = window_weak.upgrade() {
                    win.set_visible(false);
                }
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        window.add_controller(key);

        install_popup_dismiss(&window, focus_guard.clone());

        Self {
            window,
            focus_guard,
        }
    }

    pub fn present(&self) {
        present_popup(&self.window, &self.focus_guard);
    }
}

fn detect_current_profile() -> &'static str {
    if let Ok(home) = std::env::var("HOME") {
        let path = std::path::PathBuf::from(home).join(".config/hypr/monitors.lua");
        if let Ok(content) = std::fs::read_to_string(path) {
            if content.contains("monitors-dual-host") || content.contains("dual-host") {
                return "dual";
            }
        }
    }
    "single"
}

fn dirs_home_script(script_name: &str) -> String {
    if let Ok(home) = std::env::var("HOME") {
        let path = std::path::PathBuf::from(home).join(format!(".config/scripts/{}", script_name));
        if path.exists() {
            return path.to_string_lossy().to_string();
        }
    }
    script_name.to_string()
}
