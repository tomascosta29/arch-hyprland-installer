use crate::focus_guard::FocusLossGuard;
use crate::popup::{install_popup_dismiss, present_popup};
use crate::task::spawn_result;
use adw::prelude::*;
use costa_core::backends::blinker::{BlinkerBackend, CaptureMode};
use costa_core::command;
use gtk4::{gdk, glib};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

pub struct BlinkerWindow {
    window: adw::ApplicationWindow,
    focus_guard: Rc<RefCell<FocusLossGuard>>,
    toast: adw::ToastOverlay,
    backend: BlinkerBackend,
    capturing: Rc<Cell<bool>>,
}

impl BlinkerWindow {
    pub fn new(app: &adw::Application) -> Self {
        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title("Blinker")
            .default_width(480)
            .default_height(340)
            .resizable(false)
            .build();
        crate::theme::style_window(&window);

        let toast = adw::ToastOverlay::new();
        window.set_content(Some(&toast));

        let main_box = gtk4::Box::new(gtk4::Orientation::Vertical, 20);
        main_box.set_halign(gtk4::Align::Fill);
        main_box.set_margin_top(24);
        main_box.set_margin_bottom(24);
        main_box.set_margin_start(24);
        main_box.set_margin_end(24);
        toast.set_child(Some(&main_box));

        let title = gtk4::Label::new(None);
        title.set_markup("<span size='large' weight='bold'>Screenshot</span>");
        title.set_halign(gtk4::Align::Center);
        main_box.append(&title);

        let flow = gtk4::FlowBox::new();
        flow.set_halign(gtk4::Align::Center);
        flow.set_valign(gtk4::Align::Center);
        flow.set_selection_mode(gtk4::SelectionMode::None);
        flow.set_max_children_per_line(2);
        flow.set_min_children_per_line(2);
        flow.set_column_spacing(16);
        flow.set_row_spacing(16);
        flow.set_vexpand(true);
        main_box.append(&flow);

        let focus_guard = Rc::new(RefCell::new(FocusLossGuard::new()));
        let backend = BlinkerBackend::new();
        let capturing = Rc::new(Cell::new(false));
        let this_toast = toast.clone();
        let this_window = window.clone();

        let options = [
            (
                CaptureMode::Full,
                "Full Screen",
                "F1",
                "view-fullscreen-symbolic",
                gdk::Key::F1,
            ),
            (
                CaptureMode::Area,
                "Select Area",
                "F2",
                "find-location-symbolic",
                gdk::Key::F2,
            ),
            (
                CaptureMode::Window,
                "Active Window",
                "F3",
                "focus-windows-symbolic",
                gdk::Key::F3,
            ),
        ];

        for (mode, label, shortcut, icon_name, key) in options {
            let button = make_capture_tile(label, shortcut, icon_name);
            let window_click = this_window.clone();
            let toast_click = this_toast.clone();
            let backend_click = backend.clone();
            let capturing_click = capturing.clone();
            button.connect_clicked(move |_| {
                capture(
                    &window_click,
                    &toast_click,
                    &backend_click,
                    &capturing_click,
                    mode,
                );
            });
            flow.append(&button);

            let window_key = this_window.clone();
            let toast_key = this_toast.clone();
            let backend_key = backend.clone();
            let capturing_key = capturing.clone();
            let key_ctrl = gtk4::EventControllerKey::new();
            key_ctrl.connect_key_pressed(move |_, keyval, _, _| {
                if keyval == key {
                    capture(&window_key, &toast_key, &backend_key, &capturing_key, mode);
                    return glib::Propagation::Stop;
                }
                glib::Propagation::Proceed
            });
            this_window.add_controller(key_ctrl);
        }

        let ocr = make_capture_tile("Copy Text", "F4", "edit-copy-symbolic");
        let ocr_window = window.clone();
        let ocr_toast = toast.clone();
        let ocr_backend = backend.clone();
        let ocr_capturing = capturing.clone();
        ocr.connect_clicked(move |_| {
            capture_text(&ocr_window, &ocr_toast, &ocr_backend, &ocr_capturing);
        });
        flow.append(&ocr);
        let ocr_window = window.clone();
        let ocr_toast = toast.clone();
        let ocr_backend = backend.clone();
        let ocr_capturing = capturing.clone();
        let ocr_key = gtk4::EventControllerKey::new();
        ocr_key.connect_key_pressed(move |_, keyval, _, _| {
            if keyval == gdk::Key::F4 {
                capture_text(&ocr_window, &ocr_toast, &ocr_backend, &ocr_capturing);
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        window.add_controller(ocr_key);

        let footer = gtk4::Box::new(gtk4::Orientation::Horizontal, 16);
        footer.set_halign(gtk4::Align::Center);
        let settings = gtk4::Button::new();
        settings.set_label("Settings");
        settings.add_css_class("flat");
        footer.append(&settings);
        let hint = gtk4::Label::new(Some("Press Esc to close"));
        hint.add_css_class("dim-label");
        footer.append(&hint);
        main_box.append(&footer);

        let this_window_settings = window.clone();
        settings.connect_clicked(move |_| {
            let _ = command::spawn(&["costa-utils", "--blinker-manager"]);
            this_window_settings.set_visible(false);
        });

        let key = gtk4::EventControllerKey::new();
        {
            let window = window.clone();
            key.connect_key_pressed(move |_, keyval, _, _| {
                if keyval == gdk::Key::Escape {
                    window.set_visible(false);
                    return glib::Propagation::Stop;
                }
                glib::Propagation::Proceed
            });
        }
        window.add_controller(key);

        install_popup_dismiss(&window, focus_guard.clone());

        Self {
            window,
            focus_guard,
            toast,
            backend,
            capturing,
        }
    }

    pub fn present(&self) {
        present_popup(&self.window, &self.focus_guard);
    }

    pub fn capture_area(&self) {
        capture(
            &self.window,
            &self.toast,
            &self.backend,
            &self.capturing,
            CaptureMode::Area,
        );
    }

    pub fn capture_window(&self) {
        capture(
            &self.window,
            &self.toast,
            &self.backend,
            &self.capturing,
            CaptureMode::Window,
        );
    }
}

fn capture_text(
    window: &adw::ApplicationWindow,
    toast: &adw::ToastOverlay,
    backend: &BlinkerBackend,
    capturing: &Rc<Cell<bool>>,
) {
    if capturing.get() {
        return;
    }
    capturing.set(true);
    window.set_visible(false);
    let backend = backend.clone();
    let success = toast.clone();
    let failure = toast.clone();
    let done = capturing.clone();
    let failed = capturing.clone();
    spawn_result(
        move || backend.capture_text(),
        move |_| {
            done.set(false);
            success.add_toast(adw::Toast::new("Copied recognized text"));
        },
        move |err| {
            failed.set(false);
            failure.add_toast(adw::Toast::new(&err.to_string()));
        },
    );
}

fn make_capture_tile(label: &str, shortcut: &str, icon_name: &str) -> gtk4::Button {
    let button = gtk4::Button::new();
    button.add_css_class("capture-tile");

    let box_ = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
    box_.set_halign(gtk4::Align::Center);
    box_.set_valign(gtk4::Align::Center);

    let icon = gtk4::Image::from_icon_name(icon_name);
    icon.set_pixel_size(40);
    box_.append(&icon);

    let title = gtk4::Label::new(Some(label));
    title.add_css_class("capture-tile-label");
    box_.append(&title);

    let key = gtk4::Label::new(Some(shortcut));
    key.add_css_class("capture-tile-shortcut");
    box_.append(&key);

    button.set_child(Some(&box_));
    button
}

fn capture(
    window: &adw::ApplicationWindow,
    toast: &adw::ToastOverlay,
    backend: &BlinkerBackend,
    capturing: &Rc<Cell<bool>>,
    mode: CaptureMode,
) {
    if capturing.get() {
        return;
    }
    capturing.set(true);
    window.set_visible(false);
    let backend = backend.clone();
    let toast = toast.clone();
    let toast_err = toast.clone();
    let capturing = capturing.clone();
    let capturing_err = capturing.clone();
    spawn_result(
        move || backend.capture(mode),
        move |path| {
            capturing.set(false);
            toast.add_toast(adw::Toast::new(&format!(
                "Saved {}",
                path.file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("screenshot")
            )));
            let config = BlinkerBackend::new().load_config();
            if config.open_manager_after_capture {
                let _ = command::spawn(&["costa-utils", "--blinker-manager"]);
            }
        },
        move |err| {
            capturing_err.set(false);
            toast_err.add_toast(adw::Toast::new(&format!("{err}")));
        },
    );
}
