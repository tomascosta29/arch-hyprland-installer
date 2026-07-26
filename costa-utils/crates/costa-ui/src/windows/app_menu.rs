use crate::focus_guard::FocusLossGuard;
use crate::popup::{install_popup_dismiss, present_popup};
use crate::task::spawn_result;
use adw::prelude::*;
use costa_core::backends::apps::{
    clear_runner_history, evaluate_math, load_runner_history, remember_runner_command,
    should_list_app_id,
};
use costa_core::command;
use gtk4::{gdk, glib};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

struct AppEntry {
    info: gio::AppInfo,
    name: String,
    search_text: String,
}

struct Inner {
    runner_mode: bool,
    search: gtk4::SearchEntry,
    stack: gtk4::Stack,
    flowbox: Option<gtk4::FlowBox>,
    history_list: Option<gtk4::ListBox>,
    live_box: gtk4::Box,
    live_title: gtk4::Label,
    live_value: gtk4::Label,
    live_icon: gtk4::Image,
    apps: RefCell<Vec<AppEntry>>,
    history: RefCell<Vec<String>>,
    filtered_history: RefCell<Vec<String>>,
    filtered_apps: RefCell<Vec<usize>>,
    live_action: RefCell<Option<Rc<dyn Fn()>>>,
    showing_output: Cell<bool>,
    window: adw::ApplicationWindow,
}

pub struct AppMenuWindow {
    window: adw::ApplicationWindow,
    focus_guard: Rc<RefCell<FocusLossGuard>>,
    inner: Rc<Inner>,
}

impl AppMenuWindow {
    pub fn new(app: &adw::Application, runner_mode: bool) -> Self {
        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title(if runner_mode { "Runner" } else { "AppMenu" })
            .default_width(720)
            .default_height(520)
            .resizable(false)
            .build();
        crate::theme::style_window(&window);

        let main = gtk4::Box::new(gtk4::Orientation::Vertical, 16);
        main.set_margin_top(24);
        main.set_margin_bottom(24);
        main.set_margin_start(24);
        main.set_margin_end(24);
        window.set_content(Some(&main));

        let search = gtk4::SearchEntry::new();
        search.add_css_class("costa-search");
        search.set_placeholder_text(Some(if runner_mode {
            "Run command..."
        } else {
            "Search applications..."
        }));
        search.set_hexpand(true);
        main.append(&search);

        let live_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 16);
        live_box.add_css_class("live-result");
        live_box.set_visible(false);
        let live_icon = gtk4::Image::new();
        live_icon.set_pixel_size(32);
        let labels = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
        let live_title = gtk4::Label::new(None);
        live_title.set_halign(gtk4::Align::Start);
        live_title.add_css_class("dim-label");
        let live_value = gtk4::Label::new(None);
        live_value.set_halign(gtk4::Align::Start);
        live_value.add_css_class("live-result-value");
        labels.append(&live_title);
        labels.append(&live_value);
        live_box.append(&live_icon);
        live_box.append(&labels);
        main.append(&live_box);

        let stack = gtk4::Stack::new();
        stack.set_transition_type(gtk4::StackTransitionType::Crossfade);
        stack.set_vexpand(true);
        main.append(&stack);

        let scrolled = gtk4::ScrolledWindow::new();
        scrolled.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);
        let (flowbox, history_list) = if runner_mode {
            let list = gtk4::ListBox::new();
            list.add_css_class("history-list");
            scrolled.set_child(Some(&list));
            (None, Some(list))
        } else {
            let flow = gtk4::FlowBox::new();
            flow.set_valign(gtk4::Align::Start);
            flow.set_selection_mode(gtk4::SelectionMode::None);
            flow.set_max_children_per_line(5);
            flow.set_min_children_per_line(5);
            flow.set_column_spacing(12);
            flow.set_row_spacing(12);
            flow.set_activate_on_single_click(true);
            scrolled.set_child(Some(&flow));
            (Some(flow), None)
        };
        stack.add_named(&scrolled, Some("grid"));
        stack.add_named(
            &adw::StatusPage::builder()
                .icon_name("system-search-symbolic")
                .title("No Results")
                .description("Try a different search query")
                .build(),
            Some("empty"),
        );
        stack.set_visible_child_name("grid");

        let inner = Rc::new(Inner {
            runner_mode,
            search: search.clone(),
            stack: stack.clone(),
            flowbox: flowbox.clone(),
            history_list: history_list.clone(),
            live_box: live_box.clone(),
            live_title,
            live_value,
            live_icon,
            apps: RefCell::new(if runner_mode { Vec::new() } else { load_apps() }),
            history: RefCell::new(if runner_mode {
                load_runner_history()
            } else {
                Vec::new()
            }),
            filtered_history: RefCell::new(Vec::new()),
            filtered_apps: RefCell::new(Vec::new()),
            live_action: RefCell::new(None),
            showing_output: Cell::new(false),
            window: window.clone(),
        });

        {
            let inner = inner.clone();
            search.connect_search_changed(move |_| refresh_filter(&inner));
        }
        {
            let inner = inner.clone();
            search.connect_activate(move |_| activate_search(&inner));
        }
        {
            let inner = inner.clone();
            let click = gtk4::GestureClick::new();
            click.connect_released(move |_, _, _, _| {
                if let Some(action) = inner.live_action.borrow().clone() {
                    action();
                }
            });
            live_box.add_controller(click);
        }
        if let Some(flow) = &flowbox {
            let inner = inner.clone();
            flow.connect_child_activated(move |_, child| {
                let index = child.index() as usize;
                let apps = inner.apps.borrow();
                let filtered = inner.filtered_apps.borrow();
                if let Some(&app_idx) = filtered.get(index) {
                    if let Some(app) = apps.get(app_idx) {
                        let _ = app.info.launch(&[], gio::AppLaunchContext::NONE);
                    }
                }
                hide_window(&inner);
            });
        }
        if let Some(list) = &history_list {
            let inner = inner.clone();
            list.connect_row_activated(move |_, row| {
                let index = row.index() as usize;
                let cmd = inner.filtered_history.borrow().get(index).cloned();
                if let Some(cmd) = cmd {
                    run_command_line(&inner, &cmd);
                }
            });
        }

        let key = gtk4::EventControllerKey::new();
        key.set_propagation_phase(gtk4::PropagationPhase::Capture);
        {
            let inner = inner.clone();
            key.connect_key_pressed(move |_, keyval, _, state| {
                if keyval == gdk::Key::Escape {
                    hide_window(&inner);
                    return glib::Propagation::Stop;
                }
                if inner.runner_mode
                    && state.contains(gdk::ModifierType::CONTROL_MASK)
                    && state.contains(gdk::ModifierType::SHIFT_MASK)
                    && (keyval == gdk::Key::Delete || keyval == gdk::Key::KP_Delete)
                {
                    clear_runner_history();
                    inner.history.borrow_mut().clear();
                    refresh_filter(&inner);
                    return glib::Propagation::Stop;
                }
                if inner.runner_mode
                    && keyval == gdk::Key::Return
                    && state.contains(gdk::ModifierType::SHIFT_MASK)
                {
                    let mut cmd = inner.search.text().to_string();
                    if cmd.trim().is_empty() {
                        if let Some(list) = &inner.history_list {
                            if let Some(row) = list.selected_row() {
                                let index = row.index() as usize;
                                if let Some(c) = inner.filtered_history.borrow().get(index) {
                                    cmd = c.clone();
                                }
                            }
                        }
                    }
                    if !cmd.trim().is_empty() {
                        remember_runner_command(&mut inner.history.borrow_mut(), &cmd);
                        let _ = command::spawn(&["kitty", "--hold", "sh", "-lc", cmd.trim()]);
                        hide_window(&inner);
                    }
                    return glib::Propagation::Stop;
                }
                if inner.runner_mode {
                    if keyval == gdk::Key::Down {
                        if let Some(list) = &inner.history_list {
                            if inner.search.has_focus() {
                                if let Some(row) = list.row_at_index(0) {
                                    list.select_row(Some(&row));
                                    row.grab_focus();
                                }
                            } else if let Some(selected) = list.selected_row() {
                                let next = list.row_at_index(selected.index() + 1);
                                if let Some(row) = next {
                                    list.select_row(Some(&row));
                                    row.grab_focus();
                                }
                            }
                        }
                        return glib::Propagation::Stop;
                    }
                    if keyval == gdk::Key::Up {
                        if let Some(list) = &inner.history_list {
                            if !inner.search.has_focus() {
                                if let Some(selected) = list.selected_row() {
                                    if selected.index() == 0 {
                                        list.unselect_all();
                                        inner.search.grab_focus();
                                    } else if let Some(row) =
                                        list.row_at_index(selected.index() - 1)
                                    {
                                        list.select_row(Some(&row));
                                        row.grab_focus();
                                    }
                                }
                            }
                        }
                        return glib::Propagation::Stop;
                    }
                    if (keyval == gdk::Key::Return || keyval == gdk::Key::KP_Enter)
                        && !inner.search.has_focus()
                    {
                        if let Some(list) = &inner.history_list {
                            if let Some(row) = list.selected_row() {
                                let index = row.index() as usize;
                                if let Some(cmd) = inner.filtered_history.borrow().get(index).cloned() {
                                    run_command_line(&inner, &cmd);
                                    return glib::Propagation::Stop;
                                }
                            }
                        }
                    }
                } else {
                    if keyval == gdk::Key::Down && inner.search.has_focus() {
                        if let Some(flow) = &inner.flowbox {
                            if let Some(child) = flow.child_at_index(0) {
                                child.grab_focus();
                            }
                        }
                        return glib::Propagation::Stop;
                    }
                    if keyval == gdk::Key::Up && !inner.search.has_focus() {
                        inner.search.grab_focus();
                        return glib::Propagation::Stop;
                    }
                }
                glib::Propagation::Proceed
            });
        }
        window.add_controller(key);

        let focus_guard = Rc::new(RefCell::new(FocusLossGuard::new()));
        install_popup_dismiss(&window, focus_guard.clone());
        refresh_filter(&inner);

        Self {
            window,
            focus_guard,
            inner,
        }
    }

    pub fn present(&self) {
        present_popup(&self.window, &self.focus_guard);
        self.inner.search.set_text("");
        refresh_filter(&self.inner);
        self.inner.search.grab_focus();
    }
}

fn hide_window(inner: &Rc<Inner>) {
    inner.window.set_visible(false);
    let inner = inner.clone();
    glib::idle_add_local_once(move || {
        inner.search.set_text("");
        hide_live(&inner);
    });
}

fn hide_live(inner: &Inner) {
    inner.live_box.set_visible(false);
    *inner.live_action.borrow_mut() = None;
    inner.showing_output.set(false);
}

fn show_live(inner: &Inner, title: &str, value: &str, icon: &str, action: Option<Rc<dyn Fn()>>) {
    inner.live_title.set_label(title);
    inner.live_value.set_label(value);
    inner.live_icon.set_icon_name(Some(icon));
    *inner.live_action.borrow_mut() = action;
    inner.live_box.set_visible(true);
}

fn activate_search(inner: &Rc<Inner>) {
    if let Some(action) = inner.live_action.borrow().clone() {
        action();
        return;
    }
    if inner.runner_mode {
        let query = inner.search.text().to_string();
        if !query.trim().is_empty() {
            run_command_line(inner, &query);
        }
        return;
    }
    if let Some(flow) = &inner.flowbox {
        if let Some(child) = flow.child_at_index(0) {
            let index = child.index() as usize;
            let apps = inner.apps.borrow();
            let filtered = inner.filtered_apps.borrow();
            if let Some(&app_idx) = filtered.get(index) {
                if let Some(app) = apps.get(app_idx) {
                    let _ = app.info.launch(&[], gio::AppLaunchContext::NONE);
                }
            }
            hide_window(inner);
        }
    }
}

fn run_command_line(inner: &Rc<Inner>, cmd: &str) {
    remember_runner_command(&mut inner.history.borrow_mut(), cmd);
    let _ = command::spawn(&["sh", "-lc", cmd.trim()]);
    hide_window(inner);
}

fn refresh_filter(inner: &Rc<Inner>) {
    let raw = inner.search.text().to_string();
    let query = raw.trim().to_string();
    let lower = query.to_ascii_lowercase();
    let mut has = false;

    if inner.showing_output.get() && !query.starts_with('>') {
        hide_live(inner);
    }
    if !inner.showing_output.get() {
        if let Some(list) = &inner.history_list {
            while let Some(row) = list.row_at_index(0) {
                list.remove(&row);
            }
        }
        if let Some(flow) = &inner.flowbox {
            while let Some(child) = flow.child_at_index(0) {
                flow.remove(&child);
            }
        }
        if !query.starts_with('>') {
            hide_live(inner);
        }
    }

    if let Some(result) = evaluate_math(&query) {
        let result_c = result.clone();
        let window = inner.window.clone();
        show_live(
            inner,
            "Calculator",
            &result,
            "accessories-calculator-symbolic",
            Some(Rc::new(move || {
                if let Some(display) = gdk::Display::default() {
                    display.clipboard().set_text(&result_c);
                }
                window.set_visible(false);
            })),
        );
        has = true;
        inner.showing_output.set(false);
    }

    if let Some(cmd) = query.strip_prefix('>').map(str::trim) {
        if !cmd.is_empty() {
            let cmd_for_label = cmd.to_string();
            let cmd_for_action = cmd.to_string();
            let inner_c = inner.clone();
            show_live(
                inner,
                "Run Command",
                &cmd_for_label,
                "utilities-terminal-symbolic",
                Some(Rc::new(move || run_terminal_preview(&inner_c, &cmd_for_action))),
            );
            has = true;
        }
    }

    if inner.runner_mode {
        if !query.starts_with('>') {
            let history = inner.history.borrow().clone();
            if !lower.is_empty() {
                let raw = raw.clone();
                let hist = inner.history.clone();
                // hist is RefCell inside Inner - can't clone RefCell alone easily
                let window = inner.window.clone();
                let history_rc = inner.clone();
                show_live(
                    inner,
                    "Run Command",
                    &query,
                    "utilities-terminal-symbolic",
                    Some(Rc::new(move || {
                        run_command_line(&history_rc, &raw);
                        let _ = hist;
                        let _ = window;
                    })),
                );
                has = true;
            }
            let filtered: Vec<_> = if lower.is_empty() {
                history
            } else {
                history
                    .into_iter()
                    .filter(|h| h.to_ascii_lowercase().contains(&lower))
                    .collect()
            };
            if let Some(list) = &inner.history_list {
                *inner.filtered_history.borrow_mut() = filtered.clone();
                for cmd in filtered.into_iter().take(10) {
                    list.append(&history_row(&cmd));
                    has = true;
                }
            }
        }
    } else if !query.starts_with('>') {
        let apps = inner.apps.borrow();
        let mut idxs: Vec<usize> = if lower.is_empty() {
            (0..apps.len()).collect()
        } else {
            apps.iter()
                .enumerate()
                .filter(|(_, app)| app.search_text.contains(&lower))
                .map(|(i, _)| i)
                .collect()
        };
        idxs.sort_by_key(|&i| {
            let name = apps[i].name.to_ascii_lowercase();
            let score = if name == lower {
                0
            } else if name.starts_with(&lower) {
                1
            } else if name.contains(&format!(" {lower}")) {
                2
            } else if name.contains(&lower) {
                3
            } else {
                4
            };
            (score, name)
        });
        idxs.truncate(50);
        *inner.filtered_apps.borrow_mut() = idxs.clone();
        if let Some(flow) = &inner.flowbox {
            for idx in idxs {
                flow.append(&app_card(&apps[idx]));
                has = true;
            }
        }
    }

    inner
        .stack
        .set_visible_child_name(if has { "grid" } else { "empty" });
}

fn run_terminal_preview(inner: &Inner, cmd: &str) {
    inner.showing_output.set(true);
    inner.live_title.set_label("Running...");
    inner.live_value.set_label(cmd);
    inner.live_icon.set_icon_name(Some("view-refresh-symbolic"));
    let cmd = cmd.to_string();
    let title = inner.live_title.clone();
    let value = inner.live_value.clone();
    let icon = inner.live_icon.clone();
    spawn_result(
        move || {
            let output = std::process::Command::new("sh")
                .args(["-lc", &cmd])
                .output()
                .map_err(|e| costa_core::Error::Message(e.to_string()))?;
            let text = String::from_utf8_lossy(&output.stdout);
            let err = String::from_utf8_lossy(&output.stderr);
            let combined = text.trim();
            let combined = if combined.is_empty() {
                err.trim()
            } else {
                combined
            };
            Ok(if combined.is_empty() {
                "Command finished (no output)".into()
            } else {
                combined.to_string()
            })
        },
        move |out| {
            title.set_label("Command Output");
            value.set_label(&out);
            icon.set_icon_name(Some("utilities-terminal-symbolic"));
        },
        {
            let title = inner.live_title.clone();
            let value = inner.live_value.clone();
            let icon = inner.live_icon.clone();
            move |err| {
                title.set_label("Error");
                value.set_label(&err.to_string());
                icon.set_icon_name(Some("dialog-error-symbolic"));
            }
        },
    );
}

fn load_apps() -> Vec<AppEntry> {
    let mut apps = Vec::new();
    for info in gio::AppInfo::all() {
        let id = info.id().unwrap_or_default().to_string();
        let categories = info
            .downcast_ref::<gio::DesktopAppInfo>()
            .and_then(|d| d.categories())
            .map(|s| s.to_string())
            .unwrap_or_default();
        if !info.should_show() || !should_list_app_id(&id, &categories) {
            continue;
        }
        let name = info.display_name().to_string();
        let desc = info.description().map(|s| s.to_string()).unwrap_or_default();
        let search_text = format!("{name} {desc}").to_ascii_lowercase();
        apps.push(AppEntry {
            info,
            name,
            search_text,
        });
    }
    apps.sort_by(|a, b| a.name.to_ascii_lowercase().cmp(&b.name.to_ascii_lowercase()));
    apps
}

fn app_card(app: &AppEntry) -> gtk4::Box {
    let content = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
    content.set_halign(gtk4::Align::Center);
    content.add_css_class("app-card");
    let icon = if let Some(gicon) = app.info.icon() {
        gtk4::Image::from_gicon(&gicon)
    } else {
        gtk4::Image::from_icon_name("application-x-executable")
    };
    icon.set_pixel_size(48);
    content.append(&icon);
    let label = gtk4::Label::new(Some(&app.name));
    label.add_css_class("app-label");
    label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    label.set_max_width_chars(12);
    content.append(&label);
    content
}

fn history_row(cmd: &str) -> gtk4::ListBoxRow {
    let row = gtk4::ListBoxRow::new();
    let box_ = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
    box_.set_margin_start(12);
    box_.set_margin_end(12);
    box_.set_margin_top(8);
    box_.set_margin_bottom(8);
    box_.append(&gtk4::Image::from_icon_name("document-open-recent-symbolic"));
    let label = gtk4::Label::new(Some(cmd));
    label.set_halign(gtk4::Align::Start);
    box_.append(&label);
    row.set_child(Some(&box_));
    row
}
