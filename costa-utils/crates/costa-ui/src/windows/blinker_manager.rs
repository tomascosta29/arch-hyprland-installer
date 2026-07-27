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
use std::fs;
use std::path::Path;
use std::rc::Rc;
use std::time::SystemTime;

pub struct BlinkerManagerWindow {
    window: adw::ApplicationWindow,
    focus_guard: Rc<RefCell<FocusLossGuard>>,
    flow: gtk4::FlowBox,
    stack: gtk4::Stack,
    backend: BlinkerBackend,
    toast: adw::ToastOverlay,
}

impl BlinkerManagerWindow {
    pub fn new(app: &adw::Application) -> Self {
        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title("Blinker Manager")
            .default_width(820)
            .default_height(560)
            .build();
        crate::theme::style_window(&window);

        let toast = adw::ToastOverlay::new();
        window.set_content(Some(&toast));
        let view = adw::ToolbarView::new();
        toast.set_child(Some(&view));
        let header = crate::theme::header("Screenshots", "Capture history");
        let settings = gtk4::Button::from_icon_name("emblem-system-symbolic");
        settings.set_tooltip_text(Some("Settings"));
        let open_dir = gtk4::Button::from_icon_name("folder-open-symbolic");
        open_dir.set_tooltip_text(Some("Open screenshot folder"));
        header.pack_end(&open_dir);
        header.pack_end(&settings);
        view.add_top_bar(&header);

        let stack = gtk4::Stack::new();
        stack.set_vexpand(true);
        view.set_content(Some(&stack));

        let flow = gtk4::FlowBox::new();
        flow.set_selection_mode(gtk4::SelectionMode::None);
        flow.set_homogeneous(false);
        flow.set_max_children_per_line(3);
        flow.set_min_children_per_line(1);
        flow.set_column_spacing(16);
        flow.set_row_spacing(16);
        flow.set_margin_start(20);
        flow.set_margin_end(20);
        flow.set_margin_top(20);
        flow.set_margin_bottom(20);
        let scrolled = gtk4::ScrolledWindow::new();
        scrolled.set_policy(gtk4::PolicyType::Automatic, gtk4::PolicyType::Automatic);
        scrolled.set_child(Some(&flow));
        stack.add_named(&scrolled, Some("gallery"));

        let empty = adw::StatusPage::builder()
            .title("No screenshots yet")
            .description("Captured images will appear here")
            .icon_name("camera-photo-symbolic")
            .build();
        stack.add_named(&empty, Some("empty"));

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

        let focus_guard = Rc::new(RefCell::new(FocusLossGuard::new()));
        install_popup_dismiss(&window, focus_guard.clone());

        Self {
            window,
            focus_guard,
            flow,
            stack,
            backend,
            toast,
        }
    }

    pub fn present(&self) {
        present_popup(&self.window, &self.focus_guard);
        self.reload();
    }

    pub fn reload(&self) {
        while let Some(child) = self.flow.child_at_index(0) {
            self.flow.remove(&child);
        }
        let paths = self.backend.recent_screenshots(100);
        if paths.is_empty() {
            self.stack.set_visible_child_name("empty");
            return;
        }
        self.stack.set_visible_child_name("gallery");
        for path in paths {
            let child = gtk4::FlowBoxChild::new();
            child.set_child(Some(&make_screenshot_card(
                &path,
                &self.backend,
                &self.toast,
            )));
            self.flow.append(&child);
        }
    }
}

fn make_screenshot_card(
    path: &Path,
    backend: &BlinkerBackend,
    toast: &adw::ToastOverlay,
) -> gtk4::Box {
    const CARD_W: i32 = 240;
    const THUMB_H: i32 = 150;

    let card = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    card.add_css_class("screenshot-card");
    card.set_size_request(CARD_W, -1);

    let click = gtk4::GestureClick::new();
    let path_open = path.to_path_buf();
    click.connect_pressed(move |_, _, _, _| {
        let _ = command::spawn(&["xdg-open", path_open.to_str().unwrap_or(".")]);
    });
    card.add_controller(click);

    let thumb_frame = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    thumb_frame.add_css_class("screenshot-thumb-frame");
    thumb_frame.set_size_request(CARD_W, THUMB_H);

    let picture = gtk4::Picture::new();
    picture.add_css_class("screenshot-thumb");
    picture.set_content_fit(gtk4::ContentFit::Cover);
    picture.set_can_shrink(false);
    picture.set_size_request(CARD_W, THUMB_H);
    if let Ok(pixbuf) = Pixbuf::from_file_at_scale(path, CARD_W * 2, THUMB_H * 2, true) {
        let texture = gdk::Texture::for_pixbuf(&pixbuf);
        picture.set_paintable(Some(&texture));
    }
    thumb_frame.append(&picture);
    card.append(&thumb_frame);

    let footer = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    footer.add_css_class("screenshot-card-footer");

    let info = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
    info.set_hexpand(true);
    info.set_halign(gtk4::Align::Fill);

    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("screenshot");
    let title = gtk4::Label::new(Some(name));
    title.set_halign(gtk4::Align::Start);
    title.set_xalign(0.0);
    title.set_ellipsize(gtk4::pango::EllipsizeMode::Middle);
    title.add_css_class("screenshot-title");
    info.append(&title);

    let meta = gtk4::Label::new(Some(&screenshot_meta(path)));
    meta.set_halign(gtk4::Align::Start);
    meta.set_xalign(0.0);
    meta.add_css_class("screenshot-meta");
    info.append(&meta);

    footer.append(&info);

    let copy = gtk4::Button::from_icon_name("edit-copy-symbolic");
    copy.add_css_class("flat");
    copy.add_css_class("screenshot-copy-btn");
    copy.set_valign(gtk4::Align::Center);
    copy.set_tooltip_text(Some("Copy to clipboard"));
    let backend_c = backend.clone();
    let path_c = path.to_path_buf();
    let toast_c = toast.clone();
    copy.connect_clicked(move |_| match backend_c.copy_image(&path_c) {
        Ok(()) => toast_c.add_toast(adw::Toast::new("Copied to clipboard")),
        Err(err) => toast_c.add_toast(adw::Toast::new(&format!("Copy failed: {err}"))),
    });
    footer.append(&copy);

    card.append(&footer);
    card
}

fn screenshot_meta(path: &Path) -> String {
    let Ok(meta) = fs::metadata(path) else {
        return String::new();
    };
    let size = format_file_size(meta.len());
    let age = meta
        .modified()
        .ok()
        .and_then(|modified| SystemTime::now().duration_since(modified).ok())
        .map(format_age)
        .unwrap_or_default();
    if age.is_empty() {
        size
    } else {
        format!("{age} · {size}")
    }
}

fn format_file_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.0} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

fn format_age(duration: std::time::Duration) -> String {
    let secs = duration.as_secs();
    if secs < 60 {
        "Just now".to_string()
    } else if secs < 3600 {
        format!("{} min ago", secs / 60)
    } else if secs < 86_400 {
        format!("{} hr ago", secs / 3600)
    } else {
        format!("{} days ago", secs / 86_400)
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

    let sound_sw = gtk4::Switch::new();
    sound_sw.set_active(config.play_sound);
    box_.append(&switch_row("Play shutter sound", &sound_sw));

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
            play_sound: sound_sw.is_active(),
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
