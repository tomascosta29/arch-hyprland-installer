use crate::bluetooth_agent::PairingAgent;
use crate::focus_guard::FocusLossGuard;
use crate::popup::{install_popup_dismiss, present_popup};
use crate::task::spawn_result;
use adw::prelude::*;
use costa_core::backends::bluetooth::{BluetoothBackend, BluetoothState, BtDevice};
use gtk4::glib;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

struct Inner {
    backend: BluetoothBackend,
    toast: adw::ToastOverlay,
    power: gtk4::Switch,
    list: gtk4::ListBox,
    stack: gtk4::Stack,
    devices: RefCell<Vec<BtDevice>>,
    updating: Cell<bool>,
    connecting: Cell<bool>,
    window: adw::ApplicationWindow,
    _agent: Option<PairingAgent>,
}

pub struct BluetoothWindow {
    window: adw::ApplicationWindow,
    focus_guard: Rc<RefCell<FocusLossGuard>>,
    inner: Rc<Inner>,
}

impl BluetoothWindow {
    pub fn new(app: &adw::Application) -> Self {
        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title("Bluetooth Manager")
            .default_width(480)
            .default_height(450)
            .resizable(false)
            .build();
        crate::theme::style_window(&window);

        let toast = adw::ToastOverlay::new();
        window.set_content(Some(&toast));
        let view = adw::ToolbarView::new();
        toast.set_child(Some(&view));
        let header = crate::theme::header("Bluetooth", "Devices and connections");
        let refresh = gtk4::Button::from_icon_name("view-refresh-symbolic");
        header.pack_end(&refresh);
        view.add_top_bar(&header);

        let main = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
        main.set_margin_start(16);
        main.set_margin_end(16);
        main.set_margin_top(16);
        main.set_margin_bottom(16);
        view.set_content(Some(&main));

        let power_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
        power_box.add_css_class("card-box");
        power_box.append(&gtk4::Image::from_icon_name("bluetooth-active-symbolic"));
        let label = gtk4::Label::new(Some("Bluetooth Enable"));
        label.set_hexpand(true);
        label.set_halign(gtk4::Align::Start);
        let power = gtk4::Switch::new();
        power_box.append(&label);
        power_box.append(&power);
        main.append(&power_box);

        let stack = gtk4::Stack::new();
        stack.set_vexpand(true);
        main.append(&stack);
        let list = gtk4::ListBox::new();
        list.add_css_class("network-list");
        let scrolled = gtk4::ScrolledWindow::new();
        scrolled.set_child(Some(&list));
        stack.add_named(&scrolled, Some("list"));
        stack.add_named(
            &adw::StatusPage::builder()
                .title("Bluetooth is Off")
                .icon_name("bluetooth-disabled-symbolic")
                .build(),
            Some("disabled"),
        );

        let agent = PairingAgent::register(&window, &toast);
        let inner = Rc::new(Inner {
            backend: BluetoothBackend::new(),
            toast: toast.clone(),
            power: power.clone(),
            list: list.clone(),
            stack: stack.clone(),
            devices: RefCell::new(Vec::new()),
            updating: Cell::new(false),
            connecting: Cell::new(false),
            window: window.clone(),
            _agent: agent,
        });

        {
            let inner = inner.clone();
            refresh.connect_clicked(move |_| {
                let _ = inner.backend.start_discovery();
                reload(&inner);
                let inner = inner.clone();
                glib::timeout_add_seconds_local(8, move || {
                    let _ = inner.backend.stop_discovery();
                    reload(&inner);
                    glib::ControlFlow::Break
                });
            });
        }
        {
            let inner = inner.clone();
            power.connect_state_set(move |_, enabled| {
                if inner.updating.get() {
                    return glib::Propagation::Proceed;
                }
                let backend = inner.backend.clone();
                let inner = inner.clone();
                spawn_result(
                    move || backend.set_power(enabled),
                    {
                        let inner = inner.clone();
                        move |_| reload(&inner)
                    },
                    {
                        let inner = inner.clone();
                        move |err| {
                            inner
                                .toast
                                .add_toast(adw::Toast::new(&format!("Power toggle failed: {err}")));
                        }
                    },
                );
                glib::Propagation::Stop
            });
        }
        {
            let inner = inner.clone();
            list.connect_row_activated(move |_, row| {
                if inner.connecting.get() {
                    return;
                }
                let index = row.index() as usize;
                let Some(device) = inner.devices.borrow().get(index).cloned() else {
                    return;
                };
                inner.connecting.set(true);
                let backend = inner.backend.clone();
                let inner = inner.clone();
                spawn_result(
                    move || {
                        if device.connected {
                            backend.disconnect(&device.address)
                        } else {
                            if !device.paired {
                                let _ = backend.pair(&device.address);
                            }
                            backend.connect(&device.address)
                        }
                    },
                    {
                        let inner = inner.clone();
                        move |_| {
                            inner.connecting.set(false);
                            reload(&inner);
                        }
                    },
                    {
                        let inner = inner.clone();
                        move |err| {
                            inner.connecting.set(false);
                            inner.toast.add_toast(adw::Toast::new(&format!(
                                "Bluetooth action failed: {err}"
                            )));
                            reload(&inner);
                        }
                    },
                );
            });
        }

        let focus_guard = Rc::new(RefCell::new(FocusLossGuard::new()));
        install_popup_dismiss(&window, focus_guard.clone());

        Self {
            window,
            focus_guard,
            inner,
        }
    }

    pub fn present(&self) {
        present_popup(&self.window, &self.focus_guard);
        reload(&self.inner);
    }
}

fn reload(inner: &Rc<Inner>) {
    let backend = inner.backend.clone();
    let inner_ok = inner.clone();
    let inner_err = inner.clone();
    spawn_result(
        move || backend.query(),
        move |state| apply_state(&inner_ok, state),
        move |err| {
            inner_err
                .toast
                .add_toast(adw::Toast::new(&format!("Bluetooth unavailable: {err}")));
        },
    );
}

fn apply_state(inner: &Inner, state: BluetoothState) {
    inner.updating.set(true);
    inner.power.set_active(state.powered);
    inner.updating.set(false);
    *inner.devices.borrow_mut() = state.devices.clone();
    while let Some(row) = inner.list.row_at_index(0) {
        inner.list.remove(&row);
    }
    if !state.powered {
        inner.stack.set_visible_child_name("disabled");
        return;
    }
    for device in &state.devices {
        let row = gtk4::ListBoxRow::new();
        let box_ = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
        box_.add_css_class("network-row");
        box_.set_margin_start(12);
        box_.set_margin_end(12);
        box_.set_margin_top(10);
        box_.set_margin_bottom(10);
        let icon = gtk4::Image::from_icon_name("bluetooth-active-symbolic");
        if device.connected {
            icon.add_css_class("accent-icon");
        }
        box_.append(&icon);
        let labels = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
        let name = gtk4::Label::new(Some(&device.name));
        name.set_halign(gtk4::Align::Start);
        name.set_hexpand(true);
        if device.connected {
            name.add_css_class("bold-label");
        }
        let status = gtk4::Label::new(Some(if device.connected {
            "Connected"
        } else if device.paired {
            "Paired"
        } else {
            "Available"
        }));
        status.set_halign(gtk4::Align::Start);
        status.add_css_class("dim-label");
        labels.append(&name);
        labels.append(&status);
        box_.append(&labels);
        if device.connected {
            let check = gtk4::Image::from_icon_name("object-select-symbolic");
            check.add_css_class("accent-icon");
            box_.append(&check);
        }
        row.set_child(Some(&box_));
        inner.list.append(&row);
    }
    inner.stack.set_visible_child_name("list");
}
