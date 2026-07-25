use crate::focus_guard::FocusLossGuard;
use crate::popup::{install_popup_dismiss, present_popup};
use crate::task::{spawn_result, Debouncer};
use adw::prelude::*;
use costa_core::backends::audio::AudioBackend;
use costa_core::backends::bluetooth::BluetoothBackend;
use costa_core::backends::media::{MediaBackend, MediaState};
use costa_core::backends::network::NetworkBackend;
use costa_core::backends::nightlight::NightLightBackend;
use costa_core::backends::power::{PowerAction, PowerBackend};
use costa_core::command;
use gtk4::{gdk, glib, CssProvider, STYLE_PROVIDER_PRIORITY_APPLICATION};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

struct ToggleCard {
    button: gtk4::Button,
    subtitle: gtk4::Label,
    active: Cell<bool>,
}

struct Inner {
    toast: adw::ToastOverlay,
    audio: AudioBackend,
    media: MediaBackend,
    network: NetworkBackend,
    bluetooth: BluetoothBackend,
    nightlight: NightLightBackend,
    power: PowerBackend,
    vol_slider: gtk4::Scale,
    bright_slider: gtk4::Scale,
    updating: Cell<bool>,
    wifi: ToggleCard,
    bt: ToggleCard,
    nl: ToggleCard,
    dnd: ToggleCard,
    media_card: gtk4::Box,
    media_art: gtk4::Image,
    media_title: gtk4::Label,
    media_artist: gtk4::Label,
    play_btn: gtk4::Button,
    media_poll: Cell<Option<glib::SourceId>>,
    volume_debouncer: Debouncer,
    brightness_debouncer: Debouncer,
}

pub struct ControlCenterWindow {
    window: adw::ApplicationWindow,
    focus_guard: Rc<RefCell<FocusLossGuard>>,
    inner: Rc<Inner>,
}

impl ControlCenterWindow {
    pub fn new(app: &adw::Application) -> Self {
        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title("Control Menu")
            .default_width(460)
            .default_height(560)
            .resizable(false)
            .build();

        let toast = adw::ToastOverlay::new();
        window.set_content(Some(&toast));
        let view = adw::ToolbarView::new();
        toast.set_child(Some(&view));
        let header = adw::HeaderBar::new();
        let title = gtk4::Label::new(None);
        title.set_markup("<b>Control Center</b>");
        header.set_title_widget(Some(&title));
        view.add_top_bar(&header);

        let main = gtk4::Box::new(gtk4::Orientation::Vertical, 16);
        main.set_margin_start(16);
        main.set_margin_end(16);
        main.set_margin_top(16);
        main.set_margin_bottom(16);
        view.set_content(Some(&main));

        let grid = gtk4::FlowBox::new();
        grid.set_selection_mode(gtk4::SelectionMode::None);
        grid.set_max_children_per_line(2);
        grid.set_min_children_per_line(2);
        grid.set_column_spacing(12);
        grid.set_row_spacing(12);
        main.append(&grid);

        let wifi = make_toggle("network-wireless-symbolic", "Wi-Fi", "Disconnected");
        let bt = make_toggle("bluetooth-active-symbolic", "Bluetooth", "Disabled");
        let nl = make_toggle("night-light-symbolic", "Night Light", "Off");
        let dnd = make_toggle("notifications-disabled-symbolic", "Do Not Disturb", "Off");
        grid.append(&wifi.button);
        grid.append(&bt.button);
        grid.append(&nl.button);
        grid.append(&dnd.button);

        let sliders = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
        sliders.add_css_class("card-box");
        main.append(&sliders);
        let vol_slider = gtk4::Scale::with_range(gtk4::Orientation::Horizontal, 0.0, 100.0, 1.0);
        vol_slider.set_draw_value(false);
        vol_slider.set_hexpand(true);
        let vol_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        vol_row.append(&gtk4::Image::from_icon_name("audio-volume-high-symbolic"));
        vol_row.append(&vol_slider);
        sliders.append(&vol_row);
        let bright_slider = gtk4::Scale::with_range(gtk4::Orientation::Horizontal, 0.0, 100.0, 1.0);
        bright_slider.set_draw_value(false);
        bright_slider.set_hexpand(true);
        let bright_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        bright_row.append(&gtk4::Image::from_icon_name("display-brightness-symbolic"));
        bright_row.append(&bright_slider);
        sliders.append(&bright_row);

        let media_card = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
        media_card.add_css_class("media-card");
        media_card.set_visible(false);
        main.append(&media_card);
        let media_art = gtk4::Image::from_icon_name("audio-x-generic-symbolic");
        media_art.set_pixel_size(48);
        media_card.append(&media_art);
        let info = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
        info.set_hexpand(true);
        let media_title = gtk4::Label::new(Some("Unknown Title"));
        media_title.set_halign(gtk4::Align::Start);
        media_title.add_css_class("bold-label");
        let media_artist = gtk4::Label::new(Some("Unknown Artist"));
        media_artist.set_halign(gtk4::Align::Start);
        media_artist.add_css_class("dim-label");
        info.append(&media_title);
        info.append(&media_artist);
        media_card.append(&info);
        let controls = gtk4::Box::new(gtk4::Orientation::Horizontal, 4);
        let prev = gtk4::Button::from_icon_name("media-skip-backward-symbolic");
        let play_btn = gtk4::Button::from_icon_name("media-playback-start-symbolic");
        let next = gtk4::Button::from_icon_name("media-skip-forward-symbolic");
        for b in [&prev, &play_btn, &next] {
            b.add_css_class("flat");
            controls.append(b);
        }
        media_card.append(&controls);

        let session = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        session.add_css_class("card-box");
        main.append(&session);
        let lock = gtk4::Button::from_icon_name("system-lock-screen-symbolic");
        lock.set_hexpand(true);
        lock.add_css_class("power-action-btn");
        let power_menu = gtk4::Button::from_icon_name("system-shutdown-symbolic");
        power_menu.set_hexpand(true);
        power_menu.add_css_class("power-action-btn");
        session.append(&lock);
        session.append(&power_menu);

        let slot: Rc<RefCell<Option<Rc<Inner>>>> = Rc::new(RefCell::new(None));
        let volume_debouncer = {
            let slot = slot.clone();
            Debouncer::new(90, move |value| {
                if let Some(inner) = slot.borrow().clone() {
                    let audio = inner.audio.clone();
                    spawn_result(
                        move || audio.set_volume("@DEFAULT_AUDIO_SINK@", value),
                        |_| {},
                        |_| {},
                    );
                }
            })
        };
        let brightness_debouncer = {
            let slot = slot.clone();
            Debouncer::new(120, move |value| {
                if let Some(_inner) = slot.borrow().clone() {
                    let pct = format!("{value}%");
                    spawn_result(
                        move || {
                            command::run(&["brightnessctl", "set", &pct], true)?;
                            Ok(())
                        },
                        |_| {},
                        |_| {},
                    );
                }
            })
        };

        let inner = Rc::new(Inner {
            toast: toast.clone(),
            audio: AudioBackend::new(),
            media: MediaBackend::new(),
            network: NetworkBackend::new(),
            bluetooth: BluetoothBackend::new(),
            nightlight: NightLightBackend::new(),
            power: PowerBackend::new(),
            vol_slider: vol_slider.clone(),
            bright_slider: bright_slider.clone(),
            updating: Cell::new(false),
            wifi,
            bt,
            nl,
            dnd,
            media_card,
            media_art,
            media_title,
            media_artist,
            play_btn: play_btn.clone(),
            media_poll: Cell::new(None),
            volume_debouncer,
            brightness_debouncer,
        });
        *slot.borrow_mut() = Some(inner.clone());

        {
            let inner = inner.clone();
            vol_slider.connect_value_changed(move |scale| {
                if inner.updating.get() {
                    return;
                }
                inner.volume_debouncer.schedule(scale.value());
            });
        }
        {
            let inner = inner.clone();
            bright_slider.connect_value_changed(move |scale| {
                if inner.updating.get() {
                    return;
                }
                inner.brightness_debouncer.schedule(scale.value());
            });
        }
        {
            let inner = inner.clone();
            let btn = inner.wifi.button.clone();
            btn.connect_clicked(move |_| {
                let network = inner.network.clone();
                let enable = !inner.wifi.active.get();
                let inner = inner.clone();
                spawn_result(
                    move || network.set_radio(enable),
                    {
                        let inner = inner.clone();
                        move |_| refresh(&inner)
                    },
                    |_| {},
                );
            });
        }
        {
            let inner = inner.clone();
            let btn = inner.bt.button.clone();
            btn.connect_clicked(move |_| {
                let bt = inner.bluetooth.clone();
                let enable = !inner.bt.active.get();
                let inner = inner.clone();
                spawn_result(
                    move || bt.set_power(enable),
                    {
                        let inner = inner.clone();
                        move |_| refresh(&inner)
                    },
                    |_| {},
                );
            });
        }
        {
            let inner = inner.clone();
            let btn = inner.nl.button.clone();
            btn.connect_clicked(move |_| {
                let nl = inner.nightlight.clone();
                let inner = inner.clone();
                spawn_result(
                    move || nl.toggle(),
                    {
                        let inner = inner.clone();
                        move |_| refresh(&inner)
                    },
                    |_| {},
                );
            });
        }
        {
            let inner = inner.clone();
            let btn = inner.dnd.button.clone();
            btn.connect_clicked(move |_| {
                let enable = !inner.dnd.active.get();
                let arg = if enable { "true" } else { "false" };
                let inner = inner.clone();
                spawn_result(
                    move || {
                        command::run(&["dunstctl", "set-paused", arg], true)?;
                        Ok(())
                    },
                    {
                        let inner = inner.clone();
                        move |_| refresh(&inner)
                    },
                    |_| {},
                );
            });
        }
        {
            let media = inner.media.clone();
            prev.connect_clicked(move |_| {
                let media = media.clone();
                spawn_result(move || media.command("previous"), |_| {}, |_| {});
            });
        }
        {
            let media = inner.media.clone();
            play_btn.connect_clicked(move |_| {
                let media = media.clone();
                spawn_result(move || media.command("play-pause"), |_| {}, |_| {});
            });
        }
        {
            let media = inner.media.clone();
            next.connect_clicked(move |_| {
                let media = media.clone();
                spawn_result(move || media.command("next"), |_| {}, |_| {});
            });
        }
        {
            let power = inner.power.clone();
            lock.connect_clicked(move |_| {
                let _ = power.execute(PowerAction::Lock);
            });
        }
        power_menu.connect_clicked(move |_| {
            let _ = command::spawn(&["costa-utils", "--power-menu"]);
        });

        {
            let inner = inner.clone();
            window.connect_notify_local(Some("visible"), move |win, _| {
                if !win.is_visible() {
                    if let Some(id) = inner.media_poll.take() {
                        id.remove();
                    }
                    inner.volume_debouncer.cancel();
                    inner.brightness_debouncer.cancel();
                }
            });
        }

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
        refresh(&self.inner);
        start_media(&self.inner);
    }
}

fn make_toggle(icon: &str, title: &str, subtitle: &str) -> ToggleCard {
    let button = gtk4::Button::new();
    button.add_css_class("toggle-card");
    let box_ = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
    box_.append(&gtk4::Image::from_icon_name(icon));
    let labels = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
    labels.set_hexpand(true);
    let title_l = gtk4::Label::new(Some(title));
    title_l.add_css_class("toggle-title");
    title_l.set_halign(gtk4::Align::Start);
    let subtitle_l = gtk4::Label::new(Some(subtitle));
    subtitle_l.add_css_class("toggle-subtitle");
    subtitle_l.set_halign(gtk4::Align::Start);
    labels.append(&title_l);
    labels.append(&subtitle_l);
    box_.append(&labels);
    button.set_child(Some(&box_));
    ToggleCard {
        button,
        subtitle: subtitle_l,
        active: Cell::new(false),
    }
}

fn set_toggle(card: &ToggleCard, active: bool, subtitle: &str) {
    card.active.set(active);
    card.subtitle.set_label(subtitle);
    if active {
        card.button.add_css_class("suggested-action");
    } else {
        card.button.remove_css_class("suggested-action");
    }
}

fn refresh(inner: &Rc<Inner>) {
    let audio = inner.audio.clone();
    let network = inner.network.clone();
    let bluetooth = inner.bluetooth.clone();
    let nightlight = inner.nightlight.clone();
    let inner_c = inner.clone();
    spawn_result(
        move || {
            let (vol, _) = audio.get_default_volume("@DEFAULT_AUDIO_SINK@")?;
            let bright = command::run(&["brightnessctl", "--machine-readable"], false)
                .ok()
                .and_then(|r| {
                    r.stdout
                        .trim()
                        .split(',')
                        .nth(3)
                        .map(|s| s.trim_end_matches('%').parse::<u32>().unwrap_or(50))
                })
                .unwrap_or(50);
            let dnd = command::run(&["dunstctl", "is-paused"], false)
                .map(|r| r.stdout.trim() == "true")
                .unwrap_or(false);
            let wifi = network.active_status().unwrap_or((false, "Unavailable".into()));
            let bt = bluetooth.query().ok();
            let nl = nightlight.query().unwrap_or(false);
            Ok((vol, bright, dnd, wifi, bt, nl))
        },
        move |(vol, bright, dnd, wifi, bt, nl)| {
            inner_c.updating.set(true);
            inner_c.vol_slider.set_value(vol as f64);
            inner_c.bright_slider.set_value(bright as f64);
            inner_c.updating.set(false);
            set_toggle(
                &inner_c.wifi,
                wifi.0,
                if wifi.0 { &wifi.1 } else { "Disconnected" },
            );
            if let Some(state) = bt {
                let connected = state.devices.iter().filter(|d| d.connected).count();
                let subtitle = if !state.powered {
                    "Disabled".into()
                } else if connected > 0 {
                    format!("{connected} Connected")
                } else {
                    "On".into()
                };
                set_toggle(&inner_c.bt, state.powered, &subtitle);
            }
            set_toggle(
                &inner_c.nl,
                nl,
                if nl { "On" } else { "Off" },
            );
            set_toggle(
                &inner_c.dnd,
                dnd,
                if dnd { "On" } else { "Off" },
            );
        },
        {
            let inner = inner.clone();
            move |err| {
                inner
                    .toast
                    .add_toast(adw::Toast::new(&format!("Controls unavailable: {err}")));
            }
        },
    );
}

fn start_media(inner: &Rc<Inner>) {
    if let Some(id) = inner.media_poll.take() {
        id.remove();
    }
    poll_media(inner);
    let inner_c = inner.clone();
    let id = glib::timeout_add_seconds_local(2, move || {
        poll_media(&inner_c);
        glib::ControlFlow::Continue
    });
    inner.media_poll.set(Some(id));
}

fn poll_media(inner: &Rc<Inner>) {
    let media = inner.media.clone();
    let inner = inner.clone();
    spawn_result(
        move || media.current(),
        move |state| apply_media(&inner, state.unwrap_or_default()),
        |_| {},
    );
}

fn apply_media(inner: &Inner, media: MediaState) {
    if !media.has_track() {
        inner.media_card.set_visible(false);
        return;
    }
    inner.media_title.set_label(&media.title);
    inner.media_artist.set_label(&media.artist);
    inner.play_btn.set_icon_name(if media.playing() {
        "media-playback-pause-symbolic"
    } else {
        "media-playback-start-symbolic"
    });
    inner.media_card.set_visible(true);

    if media.artwork_url.is_empty() {
        inner
            .media_art
            .set_icon_name(Some("audio-x-generic-symbolic"));
        return;
    }
    let backend = inner.media.clone();
    let url = media.artwork_url.clone();
    let art = inner.media_art.clone();
    spawn_result(
        move || backend.fetch_artwork(&url),
        move |bytes| {
            if let Some(texture) = crate::artwork::texture_from_bytes(&bytes, 48) {
                art.set_paintable(Some(&texture));
            } else {
                art.set_icon_name(Some("audio-x-generic-symbolic"));
            }
        },
        {
            let art = inner.media_art.clone();
            move |_| art.set_icon_name(Some("audio-x-generic-symbolic"))
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
            .card-box {
                background: alpha(@view_fg_color, 0.04);
                border: 1px solid alpha(@view_fg_color, 0.08);
                border-radius: 12px; padding: 16px;
            }
            .toggle-card {
                padding: 14px; border-radius: 14px;
                background: alpha(@view_fg_color, 0.04);
                border: 1px solid alpha(@view_fg_color, 0.08);
            }
            .toggle-title { font-weight: bold; }
            .toggle-subtitle { font-size: 0.85em; opacity: 0.65; }
            .media-card {
                background: alpha(@window_bg_color, 0.5);
                border: 1px solid alpha(@view_fg_color, 0.1);
                border-radius: 14px; padding: 12px;
            }
            .bold-label { font-weight: bold; }
            .dim-label { opacity: 0.6; }
            .power-action-btn { min-height: 42px; }
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
