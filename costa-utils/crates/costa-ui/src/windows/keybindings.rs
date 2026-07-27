use adw::prelude::*;
use gtk4::glib;
use serde_json::Value;

#[derive(Clone)]
struct Binding {
    key: String,
    action: String,
}

pub struct KeybindingsWindow {
    window: adw::ApplicationWindow,
    search: gtk4::SearchEntry,
    list: gtk4::ListBox,
}

impl KeybindingsWindow {
    pub fn new(app: &adw::Application) -> Self {
        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title("Keybindings")
            .default_width(620)
            .default_height(620)
            .build();
        crate::theme::style_window(&window);

        let content = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
        content.set_margin_top(16);
        content.set_margin_bottom(16);
        content.set_margin_start(16);
        content.set_margin_end(16);
        content.append(&crate::theme::header(
            "Keybindings",
            "Search the bindings active in this Hyprland session",
        ));

        let search = gtk4::SearchEntry::new();
        search.set_placeholder_text(Some("Search keys, dispatcher, or command"));
        content.append(&search);

        let list = gtk4::ListBox::new();
        list.add_css_class("boxed-list");
        list.set_selection_mode(gtk4::SelectionMode::None);
        let scroller = gtk4::ScrolledWindow::builder()
            .vexpand(true)
            .child(&list)
            .build();
        content.append(&scroller);
        window.set_content(Some(&content));

        let bindings = load_bindings();
        populate(&list, &bindings, "");
        search.connect_search_changed(glib::clone!(
            #[weak]
            list,
            #[strong]
            bindings,
            move |entry| populate(&list, &bindings, entry.text().as_str())
        ));

        Self {
            window,
            search,
            list,
        }
    }

    pub fn present(&self) {
        self.window.present();
        self.search.grab_focus();
        let _ = &self.list;
    }
}

fn load_bindings() -> Vec<Binding> {
    if let Some(home) = std::env::var_os("HOME") {
        let path = std::path::PathBuf::from(home).join(".config/hypr/keybindings.json");
        if let Ok(bytes) = std::fs::read(path) {
            if let Ok(items) = serde_json::from_slice::<Vec<Value>>(&bytes) {
                let documented = items
                    .into_iter()
                    .filter_map(|item| {
                        Some(Binding {
                            key: item.get("key")?.as_str()?.to_string(),
                            action: item.get("description")?.as_str()?.to_string(),
                        })
                    })
                    .collect::<Vec<_>>();
                if !documented.is_empty() {
                    return documented;
                }
            }
        }
    }

    // Fallback for an unmanaged Hyprland configuration.
    let Ok(output) = std::process::Command::new("hyprctl")
        .args(["-j", "binds"])
        .output()
    else {
        return Vec::new();
    };
    let Ok(items) = serde_json::from_slice::<Vec<Value>>(&output.stdout) else {
        return Vec::new();
    };
    let mut result = Vec::new();
    for item in items {
        let key = item.get("key").and_then(Value::as_str).unwrap_or("");
        if key.is_empty() {
            continue;
        }
        let mods = item.get("modmask").and_then(Value::as_u64).unwrap_or(0);
        let dispatcher = item.get("dispatcher").and_then(Value::as_str).unwrap_or("");
        let arg = item.get("arg").and_then(Value::as_str).unwrap_or("");
        result.push(Binding {
            key: format!("{}{}", modifier_text(mods), key),
            action: [dispatcher, arg]
                .into_iter()
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join("  "),
        });
    }
    result.sort_by(|a, b| a.key.cmp(&b.key));
    result
}

fn modifier_text(mask: u64) -> String {
    let mut parts = Vec::new();
    if mask & 64 != 0 {
        parts.push("Super");
    }
    if mask & 4 != 0 {
        parts.push("Ctrl");
    }
    if mask & 8 != 0 {
        parts.push("Alt");
    }
    if mask & 1 != 0 {
        parts.push("Shift");
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!("{} + ", parts.join(" + "))
    }
}

fn populate(list: &gtk4::ListBox, bindings: &[Binding], query: &str) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
    let query = query.to_lowercase();
    for binding in bindings {
        if !query.is_empty()
            && !binding.key.to_lowercase().contains(&query)
            && !binding.action.to_lowercase().contains(&query)
        {
            continue;
        }
        let row = adw::ActionRow::builder()
            .title(&binding.key)
            .subtitle(&binding.action)
            .build();
        list.append(&row);
    }
}
