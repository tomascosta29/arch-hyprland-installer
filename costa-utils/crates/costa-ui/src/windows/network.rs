use crate::focus_guard::FocusLossGuard;
use crate::popup::{install_popup_dismiss, present_popup};
use crate::task::spawn_result;
use adw::prelude::*;
use costa_core::backends::network::{NetworkBackend, WifiNetwork, WifiProfile, WifiState};
use costa_core::command;
use gtk4::glib;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

struct NetworkWidgets {
    toast: adw::ToastOverlay,
    refresh_btn: gtk4::Button,
    wifi_switch: gtk4::Switch,
    stack: gtk4::Stack,
    listbox: gtk4::ListBox,
    password_title: gtk4::Label,
    password_entry: gtk4::Entry,
    loading_status: adw::StatusPage,
}

struct NetworkState {
    widgets: NetworkWidgets,
    backend: NetworkBackend,
    networks: RefCell<Vec<WifiNetwork>>,
    selected: RefCell<Option<WifiNetwork>>,
    connecting: Cell<bool>,
    updating_switch: Cell<bool>,
}

pub struct NetworkWindow {
    window: adw::ApplicationWindow,
    focus_guard: Rc<RefCell<FocusLossGuard>>,
    state: Rc<NetworkState>,
}

impl NetworkWindow {
    pub fn new(app: &adw::Application) -> Self {
        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title("Network Manager")
            .default_width(480)
            .default_height(450)
            .resizable(false)
            .build();
        crate::theme::style_window(&window);

        let toast_overlay = adw::ToastOverlay::new();
        window.set_content(Some(&toast_overlay));

        let view = adw::ToolbarView::new();
        toast_overlay.set_child(Some(&view));

        let header = crate::theme::header("Network", "Wi-Fi connections");
        let refresh_btn = gtk4::Button::from_icon_name("view-refresh-symbolic");
        refresh_btn.set_tooltip_text(Some("Refresh Wi-Fi list"));
        header.pack_end(&refresh_btn);
        view.add_top_bar(&header);

        let main_box = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
        main_box.set_margin_start(16);
        main_box.set_margin_end(16);
        main_box.set_margin_top(16);
        main_box.set_margin_bottom(16);
        view.set_content(Some(&main_box));

        let power_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
        power_box.add_css_class("card-box");
        power_box.append(&gtk4::Image::from_icon_name("network-wireless-symbolic"));
        let power_label = gtk4::Label::new(Some("Wi-Fi Enable"));
        power_label.set_hexpand(true);
        power_label.set_halign(gtk4::Align::Start);
        let wifi_switch = gtk4::Switch::new();
        power_box.append(&power_label);
        power_box.append(&wifi_switch);
        main_box.append(&power_box);

        let stack = gtk4::Stack::new();
        stack.set_transition_type(gtk4::StackTransitionType::Crossfade);
        stack.set_vexpand(true);
        main_box.append(&stack);

        let listbox = gtk4::ListBox::new();
        listbox.add_css_class("network-list");
        listbox.set_selection_mode(gtk4::SelectionMode::None);
        let scrolled = gtk4::ScrolledWindow::new();
        scrolled.set_child(Some(&listbox));
        scrolled.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);
        stack.add_named(&scrolled, Some("list"));

        let disabled = adw::StatusPage::builder()
            .title("Wi-Fi is Off")
            .description("Enable Wi-Fi to scan for networks")
            .icon_name("network-wireless-offline-symbolic")
            .build();
        stack.add_named(&disabled, Some("disabled"));

        let loading_status = adw::StatusPage::builder()
            .title("Connecting...")
            .icon_name("network-wireless-acquiring-symbolic")
            .build();
        stack.add_named(&loading_status, Some("loading"));

        let password_box = gtk4::Box::new(gtk4::Orientation::Vertical, 16);
        password_box.set_valign(gtk4::Align::Center);
        password_box.set_halign(gtk4::Align::Center);
        password_box.add_css_class("password-card");
        let password_title = gtk4::Label::new(None);
        password_title.add_css_class("password-title");
        let password_entry = gtk4::Entry::new();
        password_entry.set_placeholder_text(Some("Enter password"));
        password_entry.set_visibility(false);
        let actions = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
        actions.set_halign(gtk4::Align::Center);
        let cancel_btn = gtk4::Button::with_label("Cancel");
        let connect_btn = gtk4::Button::with_label("Connect");
        connect_btn.add_css_class("suggested-action");
        actions.append(&cancel_btn);
        actions.append(&connect_btn);
        password_box.append(&password_title);
        password_box.append(&password_entry);
        password_box.append(&actions);
        stack.add_named(&password_box, Some("password"));
        stack.set_visible_child_name("list");

        let widgets = NetworkWidgets {
            toast: toast_overlay.clone(),
            refresh_btn: refresh_btn.clone(),
            wifi_switch: wifi_switch.clone(),
            stack: stack.clone(),
            listbox: listbox.clone(),
            password_title,
            password_entry: password_entry.clone(),
            loading_status,
        };

        let state = Rc::new(NetworkState {
            widgets,
            backend: NetworkBackend::new(),
            networks: RefCell::new(Vec::new()),
            selected: RefCell::new(None),
            connecting: Cell::new(false),
            updating_switch: Cell::new(false),
        });

        {
            let state = state.clone();
            refresh_btn.connect_clicked(move |_| refresh_networks(&state));
        }
        {
            let state = state.clone();
            wifi_switch.connect_state_set(move |_, enabled| {
                if state.updating_switch.get() {
                    return glib::Propagation::Proceed;
                }
                let backend = state.backend.clone();
                let state = state.clone();
                spawn_result(
                    move || backend.set_radio(enabled),
                    {
                        let state = state.clone();
                        move |_| refresh_networks(&state)
                    },
                    {
                        let state = state.clone();
                        move |err| show_toast(&state, &format!("Wi-Fi toggle failed: {err}"))
                    },
                );
                glib::Propagation::Stop
            });
        }
        {
            let state = state.clone();
            listbox.connect_row_activated(move |_, row| {
                if state.connecting.get() {
                    return;
                }
                let index = row.index() as usize;
                let Some(network) = state.networks.borrow().get(index).cloned() else {
                    return;
                };
                if network.active {
                    return;
                }
                *state.selected.borrow_mut() = Some(network.clone());
                let backend = state.backend.clone();
                let state = state.clone();
                spawn_result(
                    move || backend.saved_profiles(),
                    {
                        let state = state.clone();
                        move |profiles| choose_connection(&state, network, profiles)
                    },
                    {
                        let state = state.clone();
                        move |err| show_toast(&state, &format!("Saved connections unavailable: {err}"))
                    },
                );
            });
        }
        {
            let state = state.clone();
            cancel_btn.connect_clicked(move |_| {
                state.widgets.stack.set_visible_child_name("list");
                *state.selected.borrow_mut() = None;
            });
        }
        {
            let state = state.clone();
            connect_btn.connect_clicked(move |_| {
                let password = state.widgets.password_entry.text().to_string();
                let Some(network) = state.selected.borrow().clone() else {
                    return;
                };
                if password.is_empty() {
                    show_toast(&state, "Enter the network password");
                    return;
                }
                start_connection(&state, network, Some(password), None);
            });
        }
        {
            let state = state.clone();
            password_entry.connect_activate(move |_| {
                let password = state.widgets.password_entry.text().to_string();
                let Some(network) = state.selected.borrow().clone() else {
                    return;
                };
                if password.is_empty() {
                    show_toast(&state, "Enter the network password");
                    return;
                }
                start_connection(&state, network, Some(password), None);
            });
        }

        let focus_guard = Rc::new(RefCell::new(FocusLossGuard::new()));
        install_popup_dismiss(&window, focus_guard.clone());

        Self {
            window,
            focus_guard,
            state,
        }
    }

    pub fn present(&self) {
        present_popup(&self.window, &self.focus_guard);
        refresh_networks(&self.state);
    }
}

fn show_toast(state: &NetworkState, text: &str) {
    state.widgets.toast.add_toast(adw::Toast::new(text));
}

fn refresh_networks(state: &Rc<NetworkState>) {
    state.widgets.refresh_btn.set_sensitive(false);
    let backend = state.backend.clone();
    let state = state.clone();
    spawn_result(
        move || backend.scan(),
        {
            let state = state.clone();
            move |wifi| apply_scan(&state, wifi)
        },
        {
            let state = state.clone();
            move |err| {
                state.widgets.refresh_btn.set_sensitive(true);
                show_toast(&state, &format!("Wi-Fi scan failed: {err}"));
            }
        },
    );
}

fn apply_scan(state: &NetworkState, wifi: WifiState) {
    state.updating_switch.set(true);
    state.widgets.wifi_switch.set_active(wifi.enabled);
    state.updating_switch.set(false);
    *state.networks.borrow_mut() = wifi.networks.clone();

    while let Some(row) = state.widgets.listbox.row_at_index(0) {
        state.widgets.listbox.remove(&row);
    }

    if !wifi.enabled {
        state.widgets.refresh_btn.set_sensitive(true);
        state.widgets.stack.set_visible_child_name("disabled");
        return;
    }

    for network in &wifi.networks {
        state.widgets.listbox.append(&network_row(network));
    }
    state.widgets.refresh_btn.set_sensitive(true);
    if !state.connecting.get() {
        state.widgets.stack.set_visible_child_name("list");
    }
}

fn network_row(net: &WifiNetwork) -> gtk4::ListBoxRow {
    let row = gtk4::ListBoxRow::new();
    let box_ = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
    box_.add_css_class("network-row");

    let icon_name = match net.signal {
        80.. => "network-wireless-signal-excellent-symbolic",
        60.. => "network-wireless-signal-good-symbolic",
        40.. => "network-wireless-signal-ok-symbolic",
        20.. => "network-wireless-signal-weak-symbolic",
        _ => "network-wireless-signal-none-symbolic",
    };
    let icon = gtk4::Image::from_icon_name(icon_name);
    if net.active {
        icon.add_css_class("accent-icon");
    }
    box_.append(&icon);

    let label = gtk4::Label::new(Some(&net.ssid));
    label.set_hexpand(true);
    label.set_halign(gtk4::Align::Start);
    if net.active {
        label.add_css_class("bold-label");
    }
    box_.append(&label);

    if !net.security.is_empty() && net.security != "--" {
        let lock = gtk4::Image::from_icon_name("network-wireless-encrypted-symbolic");
        lock.set_tooltip_text(Some(&net.security));
        box_.append(&lock);
    }
    if net.active {
        let check = gtk4::Image::from_icon_name("object-select-symbolic");
        check.add_css_class("accent-icon");
        box_.append(&check);
    }

    row.set_child(Some(&box_));
    row
}

fn choose_connection(state: &Rc<NetworkState>, network: WifiNetwork, profiles: Vec<WifiProfile>) {
    let exact = profiles.iter().find(|profile| {
        profile.ssid == network.ssid
            && !profile.bssid.is_empty()
            && profile.bssid.eq_ignore_ascii_case(&network.bssid)
    });
    let fallback = profiles
        .iter()
        .find(|profile| profile.ssid == network.ssid && profile.bssid.is_empty());
    if let Some(profile) = exact.or(fallback) {
        start_connection(state, network, None, Some(profile.uuid.clone()));
        return;
    }

    let security = network.security.to_ascii_uppercase();
    if security.is_empty() || security == "--" {
        start_connection(state, network, None, None);
    } else if security.contains("802.1X") || security.contains("EAP") {
        show_toast(
            state,
            "Enterprise Wi-Fi requires NetworkManager's full editor",
        );
        let _ = command::spawn(&["kitty", "--class", "nmtui", "-e", "nmtui-connect"]);
        state.window_hide();
    } else {
        prompt_password(state, &network);
        *state.selected.borrow_mut() = Some(network);
    }
}

impl NetworkState {
    fn window_hide(&self) {
        // Reach window via toast's root.
        if let Some(root) = self.widgets.toast.root() {
            if let Ok(win) = root.downcast::<gtk4::Window>() {
                win.set_visible(false);
            }
        }
    }
}

fn prompt_password(state: &NetworkState, network: &WifiNetwork) {
    let escaped = glib::markup_escape_text(&network.ssid);
    state
        .widgets
        .password_title
        .set_markup(&format!("Connect to <b>{escaped}</b>"));
    state.widgets.password_entry.set_text("");
    state.widgets.stack.set_visible_child_name("password");
    state.widgets.password_entry.grab_focus();
}

fn start_connection(
    state: &Rc<NetworkState>,
    network: WifiNetwork,
    password: Option<String>,
    profile_uuid: Option<String>,
) {
    let ssid = network.ssid.clone();
    state.connecting.set(true);
    state
        .widgets
        .loading_status
        .set_description(Some(&format!("Connecting to {ssid}...")));
    state.widgets.stack.set_visible_child_name("loading");

    let backend = state.backend.clone();
    let bssid = network.bssid.clone();
    let state = state.clone();
    spawn_result(
        move || {
            if let Some(uuid) = profile_uuid {
                backend.connect_saved(&uuid)
            } else if let Some(password) = password {
                backend.connect_personal(&ssid, &bssid, &password)
            } else {
                backend.connect_open(&ssid, &bssid)
            }
        },
        {
            let state = state.clone();
            let ssid = network.ssid.clone();
            move |_out| {
                state.connecting.set(false);
                show_toast(&state, &format!("Successfully connected to {ssid}"));
                state.window_hide();
            }
        },
        {
            let state = state.clone();
            let ssid = network.ssid.clone();
            move |err| {
                state.connecting.set(false);
                show_toast(&state, &format!("Connection failed: {err}"));
                state.widgets.stack.set_visible_child_name("list");
                refresh_networks(&state);
                let _ = ssid;
            }
        },
    );
}
