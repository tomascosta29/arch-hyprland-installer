use crate::focus_guard::FocusLossGuard;
use crate::popup::{install_popup_dismiss, present_popup};
use crate::task::spawn_result;
use adw::prelude::*;
use costa_core::backends::blinker::{BlinkerBackend, CaptureMode};
use costa_core::command;
use gtk4::{gdk, CssProvider, STYLE_PROVIDER_PRIORITY_APPLICATION};
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
            .default_width(420)
            .default_height(260)
            .resizable(false)
            .build();

        let toast = adw::ToastOverlay::new();
        window.set_content(Some(&toast));
        let view = adw::ToolbarView::new();
        toast.set_child(Some(&view));
        let header = adw::HeaderBar::new();
        let title = gtk4::Label::new(None);
        title.set_markup("<b>Blinker</b>");
        header.set_title_widget(Some(&title));
        let settings = gtk4::Button::from_icon_name("settings-symbolic");
        header.pack_end(&settings);
        view.add_top_bar(&header);

        let main = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
        main.set_margin_start(12);
        main.set_margin_end(12);
        main.set_margin_top(8);
        main.set_margin_bottom(12);
        view.set_content(Some(&main));

        let list = gtk4::ListBox::new();
        list.add_css_class("boxed-list");
        main.append(&list);

        let focus_guard = Rc::new(RefCell::new(FocusLossGuard::new()));
        let backend = BlinkerBackend::new();
        let capturing = Rc::new(Cell::new(false));
        let this_toast = toast.clone();
        let this_window = window.clone();

        let options = [
            (CaptureMode::Full, "Full Screen", "F1", "Capture entire screen"),
            (CaptureMode::Area, "Select Area", "F2", "Draw to select region"),
            (
                CaptureMode::Window,
                "Active Window",
                "F3",
                "Capture focused window",
            ),
        ];
        for (mode, label, shortcut, subtitle) in options {
            let row = adw::ActionRow::builder()
                .title(label)
                .subtitle(subtitle)
                .build();
            let badge = gtk4::Label::new(Some(shortcut));
            badge.add_css_class("shortcut-badge");
            row.add_prefix(&badge);
            let btn = gtk4::Button::from_icon_name("camera-photo-symbolic");
            btn.add_css_class("flat");
            let window = this_window.clone();
            let toast = this_toast.clone();
            let backend = backend.clone();
            let capturing = capturing.clone();
            btn.connect_clicked(move |_| {
                capture(&window, &toast, &backend, &capturing, mode);
            });
            row.add_suffix(&btn);
            list.append(&row);
        }

        settings.connect_clicked(move |_| {
            let _ = command::spawn(&["costa-utils", "--blinker-manager"]);
            this_window.set_visible(false);
        });

        install_popup_dismiss(&window, focus_guard.clone());
        load_css();

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

fn load_css() {
    static LOADED: std::sync::Once = std::sync::Once::new();
    LOADED.call_once(|| {
        let provider = CssProvider::new();
        provider.load_from_string(
            r#"
            window { background: alpha(@window_bg_color, 0.95); }
            .shortcut-badge {
                font-size: 0.8em; font-weight: bold;
                background: alpha(@view_fg_color, 0.1);
                padding: 2px 6px; border-radius: 6px;
            }
            "#,
        );
        if let Some(display) = gdk::Display::default() {
            gtk4::style_context_add_provider_for_display(
                &display,
                &provider,
                STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
    });
}
