use crate::focus_guard::FocusLossGuard;
use crate::popup::{install_popup_dismiss, present_popup};
use adw::prelude::*;
use costa_core::backends::blinker::{BlinkerBackend, BlinkerConfig};
use costa_core::command;
use costa_core::paths::screenshot_directory;
use gdk_pixbuf::Pixbuf;
use gtk4::gdk;
use gtk4::prelude::IsA;
use std::cell::RefCell;
use std::rc::Rc;

pub struct BlinkerManagerWindow {
    window: adw::ApplicationWindow,
    focus_guard: Rc<RefCell<FocusLossGuard>>,
    list: gtk4::ListBox,
    backend: BlinkerBackend,
    toast: adw::ToastOverlay,
}

impl BlinkerManagerWindow {
    pub fn new(app: &adw::Application) -> Self {
        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title("Blinker Manager")
            .default_width(720)
            .default_height(520)
            .build();

        let toast = adw::ToastOverlay::new();
        window.set_content(Some(&toast));
        let view = adw::ToolbarView::new();
        toast.set_child(Some(&view));
        let header = adw::HeaderBar::new();
        let title = gtk4::Label::new(None);
        title.set_markup("<b>Screenshots</b>");
        header.set_title_widget(Some(&title));
        let settings = gtk4::Button::from_icon_name("emblem-system-symbolic");
        settings.set_tooltip_text(Some("Settings"));
        let open_dir = gtk4::Button::from_icon_name("folder-open-symbolic");
        header.pack_end(&open_dir);
        header.pack_end(&settings);
        view.add_top_bar(&header);

        let list = gtk4::ListBox::new();
        list.add_css_class("boxed-list");
        let scrolled = gtk4::ScrolledWindow::new();
        scrolled.set_child(Some(&list));
        view.set_content(Some(&scrolled));

        let backend = BlinkerBackend::new();
        {
            let backend = backend.clone();
            open_dir.connect_clicked(move |_| {
                let config = backend.load_config();
                let dir = screenshot_directory(Some(&config.screenshot_dir));
                let _ = command::spawn(&["xdg-open", dir.to_str().unwrap_or(".")]);
            });
        }
        {
            let window = window.clone();
            let backend = backend.clone();
            let toast = toast.clone();
            settings.connect_clicked(move |_| {
                open_settings(&window, &backend, &toast);
            });
        }
        {
            let backend = backend.clone();
            list.connect_row_activated(move |_, row| {
                let index = row.index() as usize;
                if let Some(path) = backend.recent_screenshots(100).get(index) {
                    let _ = command::spawn(&["xdg-open", path.to_str().unwrap_or(".")]);
                }
            });
        }

        let focus_guard = Rc::new(RefCell::new(FocusLossGuard::new()));
        install_popup_dismiss(&window, focus_guard.clone());

        Self {
            window,
            focus_guard,
            list,
            backend,
            toast,
        }
    }

    pub fn present(&self) {
        present_popup(&self.window, &self.focus_guard);
        self.reload();
    }

    pub fn reload(&self) {
        while let Some(row) = self.list.row_at_index(0) {
            self.list.remove(&row);
        }
        for path in self.backend.recent_screenshots(100) {
            let name = path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("screenshot");
            let row = adw::ActionRow::builder().title(name).build();
            let thumb = gtk4::Image::from_icon_name("image-x-generic-symbolic");
            thumb.set_pixel_size(48);
            if let Ok(pixbuf) = Pixbuf::from_file_at_scale(&path, 64, 64, true) {
                let texture = gdk::Texture::for_pixbuf(&pixbuf);
                thumb.set_paintable(Some(&texture));
            }
            row.add_prefix(&thumb);
            let copy = gtk4::Button::from_icon_name("edit-copy-symbolic");
            copy.add_css_class("flat");
            let backend = self.backend.clone();
            let path_c = path.clone();
            let toast = self.toast.clone();
            copy.connect_clicked(move |_| match backend.copy_image(&path_c) {
                Ok(()) => toast.add_toast(adw::Toast::new("Copied to clipboard")),
                Err(err) => toast.add_toast(adw::Toast::new(&format!("Copy failed: {err}"))),
            });
            row.add_suffix(&copy);
            self.list.append(&row);
        }
    }
}

fn open_settings(
    parent: &adw::ApplicationWindow,
    backend: &BlinkerBackend,
    toast: &adw::ToastOverlay,
) {
    let config = backend.load_config();
    let dialog = adw::MessageDialog::new(
        Some(parent),
        Some("Blinker Settings"),
        Some("Capture and save preferences"),
    );
    dialog.add_response("cancel", "Cancel");
    dialog.add_response("save", "Save");
    dialog.set_response_appearance("save", adw::ResponseAppearance::Suggested);
    dialog.set_default_response(Some("save"));
    dialog.set_close_response("cancel");

    let box_ = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    box_.set_margin_top(8);

    let dir_entry = gtk4::Entry::new();
    dir_entry.set_text(&config.screenshot_dir);
    dir_entry.set_placeholder_text(Some("Screenshot directory"));
    box_.append(&labeled("Directory", &dir_entry));

    let pattern_entry = gtk4::Entry::new();
    pattern_entry.set_text(&config.naming_pattern);
    pattern_entry.set_placeholder_text(Some("Naming pattern (strftime)"));
    box_.append(&labeled("Filename pattern", &pattern_entry));

    let copy_sw = gtk4::Switch::new();
    copy_sw.set_active(config.copy_to_clipboard);
    box_.append(&switch_row("Copy to clipboard", &copy_sw));

    let notify_sw = gtk4::Switch::new();
    notify_sw.set_active(config.show_notification);
    box_.append(&switch_row("Show notification", &notify_sw));

    let open_sw = gtk4::Switch::new();
    open_sw.set_active(config.open_manager_after_capture);
    box_.append(&switch_row("Open manager after capture", &open_sw));

    dialog.set_extra_child(Some(&box_));

    let backend = backend.clone();
    let toast = toast.clone();
    dialog.connect_response(None, move |_, response| {
        if response != "save" {
            return;
        }
        let config = BlinkerConfig {
            screenshot_dir: dir_entry.text().to_string(),
            naming_pattern: pattern_entry.text().to_string(),
            copy_to_clipboard: copy_sw.is_active(),
            show_notification: notify_sw.is_active(),
            open_manager_after_capture: open_sw.is_active(),
        };
        match backend.save_config(&config) {
            Ok(()) => toast.add_toast(adw::Toast::new("Settings saved")),
            Err(err) => toast.add_toast(adw::Toast::new(&format!("Save failed: {err}"))),
        }
    });
    dialog.present();
}

fn labeled(title: &str, child: &impl IsA<gtk4::Widget>) -> gtk4::Box {
    let box_ = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
    let label = gtk4::Label::new(Some(title));
    label.set_halign(gtk4::Align::Start);
    label.add_css_class("dim-label");
    box_.append(&label);
    box_.append(child);
    box_
}

fn switch_row(title: &str, switch: &gtk4::Switch) -> gtk4::Box {
    let box_ = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    let label = gtk4::Label::new(Some(title));
    label.set_halign(gtk4::Align::Start);
    label.set_hexpand(true);
    box_.append(&label);
    box_.append(switch);
    box_
}
