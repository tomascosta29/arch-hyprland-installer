use crate::artwork;
use crate::focus_guard::FocusLossGuard;
use crate::popup::{install_popup_dismiss, present_popup};
use crate::task::spawn_result;
use adw::prelude::*;
use costa_core::backends::cliphist::{fuzzy_match, looks_like_existing_path, ClipBackend, ClipEntry};
use costa_core::command;
use gtk4::{gdk, glib, CssProvider, STYLE_PROVIDER_PRIORITY_APPLICATION};
use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::path::PathBuf;
use std::rc::Rc;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Filter {
    All,
    Text,
    Images,
}

struct Inner {
    backend: ClipBackend,
    list: gtk4::ListBox,
    search: gtk4::SearchEntry,
    toast: adw::ToastOverlay,
    entries: RefCell<Vec<ClipEntry>>,
    pins: RefCell<HashSet<String>>,
    filter: Cell<Filter>,
    selected_id: RefCell<Option<String>>,
    preview_stack: gtk4::Stack,
    preview_text: gtk4::TextView,
    preview_image: gtk4::Picture,
    info_label: gtk4::Label,
    edit_btn: gtk4::ToggleButton,
    json_btn: gtk4::Button,
    open_path_btn: gtk4::Button,
    #[allow(dead_code)]
    pin_btn: gtk4::Button,
    window: adw::ApplicationWindow,
}

pub struct ClipperWindow {
    window: adw::ApplicationWindow,
    focus_guard: Rc<RefCell<FocusLossGuard>>,
    inner: Rc<Inner>,
}

impl ClipperWindow {
    pub fn new(app: &adw::Application) -> Self {
        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title("Clipper")
            .default_width(900)
            .default_height(560)
            .build();

        let toast = adw::ToastOverlay::new();
        window.set_content(Some(&toast));
        let root = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        toast.set_child(Some(&root));

        let header = adw::HeaderBar::new();
        let search = gtk4::SearchEntry::new();
        search.set_placeholder_text(Some("Search clipboard..."));
        search.set_hexpand(true);
        header.set_title_widget(Some(&search));

        let filters = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
        filters.add_css_class("linked");
        let all = gtk4::ToggleButton::with_label("All");
        all.set_active(true);
        let text = gtk4::ToggleButton::with_label("Text");
        text.set_group(Some(&all));
        let images = gtk4::ToggleButton::with_label("Images");
        images.set_group(Some(&all));
        filters.append(&all);
        filters.append(&text);
        filters.append(&images);
        header.pack_start(&filters);

        let edit_btn = gtk4::ToggleButton::new();
        edit_btn.set_icon_name("document-edit-symbolic");
        edit_btn.set_tooltip_text(Some("Edit text (Ctrl+E)"));
        let json_btn = gtk4::Button::from_icon_name("text-x-script-symbolic");
        json_btn.set_tooltip_text(Some("Pretty-print JSON"));
        json_btn.set_visible(false);
        let open_path_btn = gtk4::Button::from_icon_name("folder-open-symbolic");
        open_path_btn.set_tooltip_text(Some("Open path"));
        open_path_btn.set_visible(false);
        let pin_btn = gtk4::Button::from_icon_name("user-bookmarks-symbolic");
        pin_btn.set_tooltip_text(Some("Pin / unpin (Ctrl+P)"));
        let wipe = gtk4::Button::from_icon_name("edit-delete-symbolic");
        wipe.set_tooltip_text(Some("Clear history (keeps pins)"));
        header.pack_end(&wipe);
        header.pack_end(&pin_btn);
        header.pack_end(&open_path_btn);
        header.pack_end(&json_btn);
        header.pack_end(&edit_btn);
        root.append(&header);

        let split = adw::NavigationSplitView::new();
        split.set_vexpand(true);
        root.append(&split);

        let list = gtk4::ListBox::new();
        list.add_css_class("clip-list");
        list.set_selection_mode(gtk4::SelectionMode::Single);
        let scrolled = gtk4::ScrolledWindow::new();
        scrolled.set_child(Some(&list));
        let sidebar = adw::ToolbarView::new();
        sidebar.set_content(Some(&scrolled));
        let sidebar_page = adw::NavigationPage::new(&sidebar, "History");
        split.set_sidebar(Some(&sidebar_page));

        let preview_stack = gtk4::Stack::new();
        preview_stack.set_transition_type(gtk4::StackTransitionType::Crossfade);
        let preview_text = gtk4::TextView::new();
        preview_text.set_monospace(true);
        preview_text.set_wrap_mode(gtk4::WrapMode::WordChar);
        preview_text.set_editable(false);
        preview_text.add_css_class("preview-text");
        let text_scroll = gtk4::ScrolledWindow::new();
        text_scroll.set_child(Some(&preview_text));
        preview_stack.add_named(&text_scroll, Some("text"));

        let preview_image = gtk4::Picture::new();
        preview_image.set_can_shrink(true);
        preview_image.set_content_fit(gtk4::ContentFit::Contain);
        let img_scroll = gtk4::ScrolledWindow::new();
        img_scroll.set_child(Some(&preview_image));
        preview_stack.add_named(&img_scroll, Some("image"));

        let empty = adw::StatusPage::builder()
            .title("Select an item")
            .icon_name("edit-copy-symbolic")
            .build();
        preview_stack.add_named(&empty, Some("empty"));
        preview_stack.set_visible_child_name("empty");

        let overlay = gtk4::Overlay::new();
        overlay.set_child(Some(&preview_stack));
        let info_label = gtk4::Label::new(None);
        info_label.add_css_class("preview-info-pill");
        info_label.set_halign(gtk4::Align::End);
        info_label.set_valign(gtk4::Align::End);
        info_label.set_margin_end(12);
        info_label.set_margin_bottom(12);
        overlay.add_overlay(&info_label);

        let content_page = adw::NavigationPage::new(&overlay, "Preview");
        split.set_content(Some(&content_page));

        let backend = ClipBackend::new();
        let pins = backend.load_pins();
        let inner = Rc::new(Inner {
            backend,
            list: list.clone(),
            search: search.clone(),
            toast: toast.clone(),
            entries: RefCell::new(Vec::new()),
            pins: RefCell::new(pins),
            filter: Cell::new(Filter::All),
            selected_id: RefCell::new(None),
            preview_stack,
            preview_text: preview_text.clone(),
            preview_image,
            info_label,
            edit_btn: edit_btn.clone(),
            json_btn: json_btn.clone(),
            open_path_btn: open_path_btn.clone(),
            pin_btn: pin_btn.clone(),
            window: window.clone(),
        });

        {
            let inner = inner.clone();
            search.connect_search_changed(move |_| apply_filter(&inner));
        }
        {
            let inner = inner.clone();
            all.connect_toggled(move |btn| {
                if btn.is_active() {
                    inner.filter.set(Filter::All);
                    apply_filter(&inner);
                }
            });
        }
        {
            let inner = inner.clone();
            text.connect_toggled(move |btn| {
                if btn.is_active() {
                    inner.filter.set(Filter::Text);
                    apply_filter(&inner);
                }
            });
        }
        {
            let inner = inner.clone();
            images.connect_toggled(move |btn| {
                if btn.is_active() {
                    inner.filter.set(Filter::Images);
                    apply_filter(&inner);
                }
            });
        }
        {
            let inner = inner.clone();
            list.connect_row_selected(move |_, row| {
                let Some(row) = row else {
                    *inner.selected_id.borrow_mut() = None;
                    clear_preview(&inner);
                    return;
                };
                let index = row.index() as usize;
                if let Some(entry) = visible_entries(&inner).get(index).cloned() {
                    *inner.selected_id.borrow_mut() = Some(entry.id.clone());
                    load_preview(&inner, entry);
                }
            });
        }
        {
            let inner = inner.clone();
            list.connect_row_activated(move |_, _| copy_selected(&inner, true));
        }
        {
            let inner = inner.clone();
            edit_btn.connect_toggled(move |btn| {
                let editable = btn.is_active();
                inner.preview_text.set_editable(editable);
                if editable {
                    inner.preview_text.add_css_class("editable");
                } else {
                    inner.preview_text.remove_css_class("editable");
                }
            });
        }
        {
            let inner = inner.clone();
            json_btn.connect_clicked(move |_| {
                let buffer = inner.preview_text.buffer();
                let (start, end) = buffer.bounds();
                let text = buffer.text(&start, &end, true).to_string();
                if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
                    if let Ok(pretty) = serde_json::to_string_pretty(&value) {
                        buffer.set_text(&pretty);
                    }
                }
            });
        }
        {
            let inner = inner.clone();
            open_path_btn.connect_clicked(move |_| {
                let buffer = inner.preview_text.buffer();
                let (start, end) = buffer.bounds();
                let text = buffer.text(&start, &end, true).to_string();
                let path = expand_path(text.trim());
                let open = if path.is_file() {
                    path.parent()
                        .map(PathBuf::from)
                        .unwrap_or(path)
                } else {
                    path
                };
                let _ = command::spawn(&["xdg-open", open.to_str().unwrap_or(".")]);
            });
        }
        {
            let inner = inner.clone();
            pin_btn.connect_clicked(move |_| toggle_pin(&inner));
        }
        {
            let inner = inner.clone();
            wipe.connect_clicked(move |_| {
                let backend = inner.backend.clone();
                let pins = inner.pins.borrow().clone();
                let inner = inner.clone();
                spawn_result(
                    move || backend.wipe_preserving_pins(&pins),
                    {
                        let inner = inner.clone();
                        move |_| reload(&inner)
                    },
                    {
                        let inner = inner.clone();
                        move |err| {
                            inner
                                .toast
                                .add_toast(adw::Toast::new(&format!("Wipe failed: {err}")));
                        }
                    },
                );
            });
        }

        let key = gtk4::EventControllerKey::new();
        {
            let inner = inner.clone();
            key.connect_key_pressed(move |_, keyval, _, state| {
                if keyval == gdk::Key::Escape {
                    inner.window.set_visible(false);
                    return glib::Propagation::Stop;
                }
                if state.contains(gdk::ModifierType::CONTROL_MASK) {
                    if keyval == gdk::Key::p || keyval == gdk::Key::P {
                        toggle_pin(&inner);
                        return glib::Propagation::Stop;
                    }
                    if keyval == gdk::Key::e || keyval == gdk::Key::E {
                        inner.edit_btn.set_active(!inner.edit_btn.is_active());
                        return glib::Propagation::Stop;
                    }
                }
                glib::Propagation::Proceed
            });
        }
        window.add_controller(key);

        let focus_guard = Rc::new(RefCell::new(FocusLossGuard::new()));
        install_popup_dismiss(&window, focus_guard.clone());
        load_css();

        Self {
            window,
            focus_guard,
            inner,
        }
    }

    pub fn present(&self) {
        present_popup(&self.window, &self.focus_guard);
        reload(&self.inner);
        self.inner.search.grab_focus();
    }
}

fn expand_path(raw: &str) -> PathBuf {
    if let Some(rest) = raw.strip_prefix("~/") {
        PathBuf::from(std::env::var_os("HOME").unwrap_or_else(|| "/".into())).join(rest)
    } else if raw == "~" {
        PathBuf::from(std::env::var_os("HOME").unwrap_or_else(|| "/".into()))
    } else {
        PathBuf::from(raw)
    }
}

fn reload(inner: &Rc<Inner>) {
    let backend = inner.backend.clone();
    let pins = backend.load_pins();
    *inner.pins.borrow_mut() = pins;
    let inner_ok = inner.clone();
    let inner_err = inner.clone();
    spawn_result(
        move || backend.list(),
        move |entries| {
            *inner_ok.entries.borrow_mut() = entries;
            apply_filter(&inner_ok);
        },
        move |err| {
            inner_err
                .toast
                .add_toast(adw::Toast::new(&format!("Clipboard unavailable: {err}")));
        },
    );
}

fn visible_entries(inner: &Inner) -> Vec<ClipEntry> {
    let query = inner.search.text().to_string();
    let filter = inner.filter.get();
    let pins = inner.pins.borrow();
    let mut entries: Vec<_> = inner
        .entries
        .borrow()
        .iter()
        .filter(|entry| match filter {
            Filter::All => true,
            Filter::Text => !entry.is_image,
            Filter::Images => entry.is_image,
        })
        .filter(|entry| fuzzy_match(&query, &entry.preview))
        .cloned()
        .collect();
    entries.sort_by_key(|e| (!pins.contains(&e.id), e.id.clone()));
    entries
}

fn apply_filter(inner: &Inner) {
    while let Some(row) = inner.list.row_at_index(0) {
        inner.list.remove(&row);
    }
    let pins = inner.pins.borrow().clone();
    let selected = inner.selected_id.borrow().clone();
    let mut select_index = None;
    for (i, entry) in visible_entries(inner).into_iter().enumerate() {
        let row = gtk4::ListBoxRow::new();
        let box_ = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
        box_.set_margin_start(12);
        box_.set_margin_end(12);
        box_.set_margin_top(10);
        box_.set_margin_bottom(10);

        let thumb = gtk4::Image::from_icon_name(if entry.is_image {
            "image-x-generic-symbolic"
        } else {
            "text-x-generic-symbolic"
        });
        thumb.set_pixel_size(28);
        if entry.is_image {
            let backend = inner.backend.clone();
            let id = entry.id.clone();
            let thumb_c = thumb.clone();
            spawn_result(
                move || backend.decode(&id),
                move |bytes| {
                    if let Some(texture) = artwork::texture_from_bytes(&bytes, 48) {
                        thumb_c.set_paintable(Some(&texture));
                    }
                },
                |_| {},
            );
        }

        let label = gtk4::Label::new(Some(&entry.preview));
        label.set_halign(gtk4::Align::Start);
        label.set_hexpand(true);
        label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        box_.append(&thumb);
        box_.append(&label);
        if pins.contains(&entry.id) {
            let pin = gtk4::Image::from_icon_name("user-bookmarks-symbolic");
            pin.set_pixel_size(14);
            box_.append(&pin);
        }
        row.set_child(Some(&box_));
        inner.list.append(&row);
        if selected.as_ref() == Some(&entry.id) {
            select_index = Some(i as i32);
        }
    }
    if let Some(index) = select_index {
        if let Some(row) = inner.list.row_at_index(index) {
            inner.list.select_row(Some(&row));
        }
    } else if inner.list.row_at_index(0).is_some() {
        // keep empty selection until user picks
    }
}

fn clear_preview(inner: &Inner) {
    inner.preview_stack.set_visible_child_name("empty");
    inner.info_label.set_text("");
    inner.json_btn.set_visible(false);
    inner.open_path_btn.set_visible(false);
    inner.edit_btn.set_sensitive(false);
}

fn load_preview(inner: &Rc<Inner>, entry: ClipEntry) {
    inner.edit_btn.set_sensitive(!entry.is_image);
    if entry.is_image {
        inner.edit_btn.set_active(false);
        inner.json_btn.set_visible(false);
        inner.open_path_btn.set_visible(false);
        let backend = inner.backend.clone();
        let id = entry.id.clone();
        let inner_ok = inner.clone();
        let inner_err = inner.clone();
        spawn_result(
            move || backend.decode(&id),
            move |bytes| {
                if let Some(texture) = artwork::texture_from_bytes(&bytes, 720) {
                    inner_ok.preview_image.set_paintable(Some(&texture));
                    inner_ok.preview_stack.set_visible_child_name("image");
                    inner_ok
                        .info_label
                        .set_text(&format!("{} bytes", bytes.len()));
                } else {
                    inner_ok.preview_stack.set_visible_child_name("empty");
                }
            },
            move |err| {
                inner_err
                    .toast
                    .add_toast(adw::Toast::new(&format!("Preview failed: {err}")));
            },
        );
        return;
    }

    let backend = inner.backend.clone();
    let id = entry.id.clone();
    let inner_ok = inner.clone();
    let inner_err = inner.clone();
    spawn_result(
        move || backend.decode_text(&id),
        move |text| {
            inner_ok.preview_text.buffer().set_text(&text);
            inner_ok.preview_stack.set_visible_child_name("text");
            inner_ok
                .info_label
                .set_text(&format!("{} chars", text.chars().count()));
            let looks_json =
                text.trim_start().starts_with('{') || text.trim_start().starts_with('[');
            inner_ok.json_btn.set_visible(looks_json);
            inner_ok
                .open_path_btn
                .set_visible(looks_like_existing_path(&text));
        },
        move |err| {
            inner_err
                .toast
                .add_toast(adw::Toast::new(&format!("Preview failed: {err}")));
        },
    );
}

fn toggle_pin(inner: &Inner) {
    let Some(id) = inner.selected_id.borrow().clone() else {
        return;
    };
    {
        let mut pins = inner.pins.borrow_mut();
        if pins.contains(&id) {
            pins.remove(&id);
        } else {
            pins.insert(id);
        }
        let _ = inner.backend.save_pins(&pins);
    }
    apply_filter(inner);
}

fn copy_selected(inner: &Rc<Inner>, close: bool) {
    let Some(id) = inner.selected_id.borrow().clone() else {
        return;
    };
    if inner.edit_btn.is_active() {
        let buffer = inner.preview_text.buffer();
        let (start, end) = buffer.bounds();
        let text = buffer.text(&start, &end, true).to_string();
        let backend = inner.backend.clone();
        let inner_ok = inner.clone();
        let inner_err = inner.clone();
        spawn_result(
            move || backend.copy_text(&text),
            move |_| {
                if close {
                    inner_ok.window.set_visible(false);
                }
            },
            move |err| {
                inner_err
                    .toast
                    .add_toast(adw::Toast::new(&format!("Copy failed: {err}")));
            },
        );
        return;
    }
    let backend = inner.backend.clone();
    let inner_ok = inner.clone();
    let inner_err = inner.clone();
    spawn_result(
        move || backend.copy_id(&id),
        move |_| {
            if close {
                inner_ok.window.set_visible(false);
            }
        },
        move |err| {
            inner_err
                .toast
                .add_toast(adw::Toast::new(&format!("Copy failed: {err}")));
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
            .clip-list {
                background: alpha(@view_fg_color, 0.02);
                border: 1px solid alpha(@view_fg_color, 0.08);
                border-radius: 12px;
            }
            .preview-text { font-family: monospace; padding: 12px; }
            .preview-text.editable {
                background: alpha(@accent_bg_color, 0.08);
            }
            .preview-info-pill {
                background: alpha(@window_bg_color, 0.85);
                border-radius: 999px;
                padding: 4px 10px;
                font-size: 0.8em;
                opacity: 0.8;
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
