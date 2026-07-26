use crate::windows::{
    AppMenuWindow, BlinkerManagerWindow, BlinkerWindow, BluetoothWindow, ClipperWindow,
    ControlCenterWindow, MonitorWindow, NetworkWindow, PowerWindow, VolumeWindow,
};
use crate::ACTIVATE_TARGET_ACTION;
use adw::prelude::*;
use costa_core::Target;
use glib::clone;
use gtk4::gdk;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use tracing::{info, warn};

pub const APPLICATION_ID: &str = "org.fcosta.CostaUtils";

#[derive(Default)]
struct Windows {
    power: Option<PowerWindow>,
    volume: Option<VolumeWindow>,
    network: Option<NetworkWindow>,
    bluetooth: Option<BluetoothWindow>,
    app_menu: Option<AppMenuWindow>,
    runner: Option<AppMenuWindow>,
    clipper: Option<ClipperWindow>,
    blinker: Option<BlinkerWindow>,
    blinker_manager: Option<BlinkerManagerWindow>,
    monitor: Option<MonitorWindow>,
    control_center: Option<ControlCenterWindow>,
}

struct AppState {
    windows: RefCell<Windows>,
    _hold: gio::ApplicationHoldGuard,
}

pub fn run(initial: Target) -> i32 {
    configure_renderer();

    let app = adw::Application::builder()
        .application_id(APPLICATION_ID)
        .build();

    if let Err(err) = app.register(gio::Cancellable::NONE) {
        warn!(%err, "failed to register application");
        return 1;
    }

    crate::theme::install();
    ensure_bundled_icons();

    if app.is_remote() {
        info!(target = initial.flag(), "forwarding to primary instance");
        app.activate_action(
            ACTIVATE_TARGET_ACTION,
            Some(&glib::Variant::from(initial.flag())),
        );
        let ctx = glib::MainContext::default();
        while ctx.iteration(false) {}
        return 0;
    }

    let hold = app.hold();
    let state = Rc::new(AppState {
        windows: RefCell::new(Windows::default()),
        _hold: hold,
    });

    let activate = gio::SimpleAction::new(ACTIVATE_TARGET_ACTION, Some(glib::VariantTy::STRING));
    activate.connect_activate(clone!(
        #[weak]
        app,
        #[strong]
        state,
        move |_, param| {
            let Some(variant) = param else {
                return;
            };
            let Some(flag) = variant.get::<String>() else {
                return;
            };
            match Target::parse(&flag) {
                Ok(target) => activate_target(&app, &state, target),
                Err(err) => warn!(%err, "ignored activate-target"),
            }
        }
    ));
    app.add_action(&activate);

    let about = gio::SimpleAction::new("about", None);
    about.connect_activate(clone!(
        #[weak]
        app,
        move |_, _| {
            let dialog = adw::AboutWindow::builder()
                .application_name("Costa Utils")
                .application_icon(APPLICATION_ID)
                .developer_name("fcosta")
                .version(env!("CARGO_PKG_VERSION"))
                .comments("Desktop utilities for the Arch Hyprland workstation.")
                .build();
            if let Some(win) = app.active_window() {
                dialog.set_transient_for(Some(&win));
            }
            dialog.present();
        }
    ));
    app.add_action(&about);

    app.connect_activate(clone!(
        #[strong]
        state,
        move |app| {
            activate_target(app, &state, initial);
        }
    ));

    app.run_with_args::<&str>(&[]).into()
}

fn activate_target(app: &adw::Application, state: &AppState, target: Target) {
    info!(target = target.flag(), "activate");
    let mut windows = state.windows.borrow_mut();
    match target {
        Target::Daemon => {
            info!("resident service ready");
        }
        Target::Shutdown => app.quit(),
        Target::PowerMenu => {
            if windows.power.is_none() {
                windows.power = Some(PowerWindow::new(app));
            }
            windows.power.as_ref().unwrap().present();
        }
        Target::VolumeMenu => {
            if windows.volume.is_none() {
                windows.volume = Some(VolumeWindow::new(app));
            }
            windows.volume.as_ref().unwrap().present();
        }
        Target::NetworkMenu => {
            if windows.network.is_none() {
                windows.network = Some(NetworkWindow::new(app));
            }
            windows.network.as_ref().unwrap().present();
        }
        Target::BluetoothMenu => {
            if windows.bluetooth.is_none() {
                windows.bluetooth = Some(BluetoothWindow::new(app));
            }
            windows.bluetooth.as_ref().unwrap().present();
        }
        Target::AppMenu => {
            if windows.app_menu.is_none() {
                windows.app_menu = Some(AppMenuWindow::new(app, false));
            }
            windows.app_menu.as_ref().unwrap().present();
        }
        Target::Runner => {
            if windows.runner.is_none() {
                windows.runner = Some(AppMenuWindow::new(app, true));
            }
            windows.runner.as_ref().unwrap().present();
        }
        Target::Clipper => {
            if windows.clipper.is_none() {
                windows.clipper = Some(ClipperWindow::new(app));
            }
            windows.clipper.as_ref().unwrap().present();
        }
        Target::Blinker => {
            if windows.blinker.is_none() {
                windows.blinker = Some(BlinkerWindow::new(app));
            }
            windows.blinker.as_ref().unwrap().present();
        }
        Target::BlinkerArea => {
            if windows.blinker.is_none() {
                windows.blinker = Some(BlinkerWindow::new(app));
            }
            windows.blinker.as_ref().unwrap().capture_area();
        }
        Target::BlinkerManager => {
            if windows.blinker_manager.is_none() {
                windows.blinker_manager = Some(BlinkerManagerWindow::new(app));
            }
            windows.blinker_manager.as_ref().unwrap().present();
        }
        Target::MonitorMenu => {
            if windows.monitor.is_none() {
                windows.monitor = Some(MonitorWindow::new(app));
            }
            windows.monitor.as_ref().unwrap().present();
        }
        Target::ControlCenter => {
            if windows.control_center.is_none() {
                windows.control_center = Some(ControlCenterWindow::new(app));
            }
            windows.control_center.as_ref().unwrap().present();
        }
    }
}

fn configure_renderer() {
    if std::path::Path::new("/sys/module/virtio_gpu").is_dir() {
        std::env::set_var("GSK_RENDERER", "cairo");
    } else if !std::path::Path::new("/sys/module/amdgpu").is_dir()
        && std::env::var_os("GSK_RENDERER").is_none()
    {
        std::env::set_var("GSK_RENDERER", "gl");
    }
}

/// Register bundled icon themes (e.g. suspend) missing from Adwaita.
fn ensure_bundled_icons() {
    static LOADED: std::sync::Once = std::sync::Once::new();
    LOADED.call_once(|| {
        let Some(display) = gdk::Display::default() else {
            return;
        };
        let theme = gtk4::IconTheme::for_display(&display);
        for path in bundled_icon_search_paths() {
            if path.is_dir() {
                theme.add_search_path(&path);
            }
        }
    });
}

fn bundled_icon_search_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(home) = std::env::var_os("HOME") {
        let data = PathBuf::from(home).join(".local/share");
        paths.push(data.join("costa-utils/icons"));
        paths.push(data.join("icons"));
    }
    paths
}
