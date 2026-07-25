use crate::focus_guard::FocusLossGuard;
use crate::popup::{install_popup_dismiss, present_popup};
use crate::task::{spawn_result, Debouncer};
use adw::prelude::*;
use costa_core::backends::audio::{AudioBackend, AudioDevice, AudioSnapshot};
use costa_core::backends::media::{MediaBackend, MediaState};
use gtk4::{gdk, glib, CssProvider, STYLE_PROVIDER_PRIORITY_APPLICATION};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

struct VolumeWidgets {
    toast: adw::ToastOverlay,
    out_vol_val: gtk4::Label,
    in_vol_val: gtk4::Label,
    out_mute_btn: gtk4::Button,
    in_mute_btn: gtk4::Button,
    out_slider: gtk4::Scale,
    in_slider: gtk4::Scale,
    sink_list: gtk4::ListBox,
    source_list: gtk4::ListBox,
    media_card: gtk4::Box,
    media_art: gtk4::Image,
    media_title: gtk4::Label,
    media_artist: gtk4::Label,
    play_btn: gtk4::Button,
}

struct VolumeState {
    widgets: VolumeWidgets,
    audio: AudioBackend,
    media: MediaBackend,
    sinks: RefCell<Vec<AudioDevice>>,
    sources: RefCell<Vec<AudioDevice>>,
    updating_sliders: Cell<bool>,
    media_poll: Cell<Option<glib::SourceId>>,
    output_debouncer: Debouncer,
    input_debouncer: Debouncer,
}

pub struct VolumeWindow {
    window: adw::ApplicationWindow,
    focus_guard: Rc<RefCell<FocusLossGuard>>,
    state: Rc<VolumeState>,
}

impl VolumeWindow {
    pub fn new(app: &adw::Application) -> Self {
        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title("Volume Menu")
            .default_width(480)
            .default_height(520)
            .resizable(false)
            .build();

        let toast_overlay = adw::ToastOverlay::new();
        window.set_content(Some(&toast_overlay));

        let view = adw::ToolbarView::new();
        toast_overlay.set_child(Some(&view));

        let header = adw::HeaderBar::new();
        let title = gtk4::Label::new(None);
        title.set_markup("<b>Audio &amp; Media</b>");
        header.set_title_widget(Some(&title));
        view.add_top_bar(&header);

        let main_box = gtk4::Box::new(gtk4::Orientation::Vertical, 16);
        main_box.set_margin_start(16);
        main_box.set_margin_end(16);
        main_box.set_margin_top(16);
        main_box.set_margin_bottom(16);
        view.set_content(Some(&main_box));

        let sliders = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
        sliders.add_css_class("card-box");
        main_box.append(&sliders);

        let (out_lbl_box, out_vol_val) = labeled_row("Output Volume");
        sliders.append(&out_lbl_box);
        let out_mute_btn = gtk4::Button::from_icon_name("audio-volume-high-symbolic");
        out_mute_btn.add_css_class("flat");
        let out_slider = gtk4::Scale::with_range(gtk4::Orientation::Horizontal, 0.0, 150.0, 1.0);
        out_slider.set_draw_value(false);
        out_slider.set_hexpand(true);
        let out_slider_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        out_slider_box.append(&out_mute_btn);
        out_slider_box.append(&out_slider);
        sliders.append(&out_slider_box);

        let (in_lbl_box, in_vol_val) = labeled_row("Input Volume");
        sliders.append(&in_lbl_box);
        let in_mute_btn = gtk4::Button::from_icon_name("audio-input-microphone-symbolic");
        in_mute_btn.add_css_class("flat");
        let in_slider = gtk4::Scale::with_range(gtk4::Orientation::Horizontal, 0.0, 150.0, 1.0);
        in_slider.set_draw_value(false);
        in_slider.set_hexpand(true);
        let in_slider_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        in_slider_box.append(&in_mute_btn);
        in_slider_box.append(&in_slider);
        sliders.append(&in_slider_box);

        let device_stack = gtk4::Stack::new();
        device_stack.set_transition_type(gtk4::StackTransitionType::Crossfade);
        device_stack.set_vexpand(true);

        let sink_list = gtk4::ListBox::new();
        sink_list.add_css_class("device-list-sub");
        let sink_scrolled = gtk4::ScrolledWindow::new();
        sink_scrolled.set_child(Some(&sink_list));
        device_stack.add_titled(&sink_scrolled, Some("output"), "Outputs");

        let source_list = gtk4::ListBox::new();
        source_list.add_css_class("device-list-sub");
        let source_scrolled = gtk4::ScrolledWindow::new();
        source_scrolled.set_child(Some(&source_list));
        device_stack.add_titled(&source_scrolled, Some("input"), "Inputs");

        let switcher = gtk4::StackSwitcher::new();
        switcher.set_stack(Some(&device_stack));
        switcher.set_halign(gtk4::Align::Center);
        main_box.append(&switcher);
        main_box.append(&device_stack);

        let media_card = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
        media_card.add_css_class("media-card");
        media_card.set_visible(false);
        main_box.append(&media_card);

        let media_art = gtk4::Image::from_icon_name("audio-x-generic-symbolic");
        media_art.set_pixel_size(64);
        media_card.append(&media_art);

        let info = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
        info.set_hexpand(true);
        let media_title = gtk4::Label::new(Some("Unknown Title"));
        media_title.set_halign(gtk4::Align::Start);
        media_title.add_css_class("bold-label");
        media_title.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        let media_artist = gtk4::Label::new(Some("Unknown Artist"));
        media_artist.set_halign(gtk4::Align::Start);
        media_artist.add_css_class("dim-label");
        media_artist.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        info.append(&media_title);
        info.append(&media_artist);
        media_card.append(&info);

        let controls = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
        controls.set_valign(gtk4::Align::Center);
        let prev_btn = gtk4::Button::from_icon_name("media-skip-backward-symbolic");
        prev_btn.add_css_class("flat");
        let play_btn = gtk4::Button::from_icon_name("media-playback-start-symbolic");
        play_btn.add_css_class("flat");
        let next_btn = gtk4::Button::from_icon_name("media-skip-forward-symbolic");
        next_btn.add_css_class("flat");
        controls.append(&prev_btn);
        controls.append(&play_btn);
        controls.append(&next_btn);
        media_card.append(&controls);

        let widgets = VolumeWidgets {
            toast: toast_overlay.clone(),
            out_vol_val,
            in_vol_val,
            out_mute_btn: out_mute_btn.clone(),
            in_mute_btn: in_mute_btn.clone(),
            out_slider: out_slider.clone(),
            in_slider: in_slider.clone(),
            sink_list: sink_list.clone(),
            source_list: source_list.clone(),
            media_card,
            media_art,
            media_title,
            media_artist,
            play_btn: play_btn.clone(),
        };

        // Debouncers created after we have a state Rc — placeholder then rebuild.
        let state_slot: Rc<RefCell<Option<Rc<VolumeState>>>> = Rc::new(RefCell::new(None));

        let output_debouncer = {
            let slot = state_slot.clone();
            Debouncer::new(90, move |value| {
                if let Some(state) = slot.borrow().clone() {
                    set_volume(&state, "@DEFAULT_AUDIO_SINK@", value);
                }
            })
        };
        let input_debouncer = {
            let slot = state_slot.clone();
            Debouncer::new(90, move |value| {
                if let Some(state) = slot.borrow().clone() {
                    set_volume(&state, "@DEFAULT_AUDIO_SOURCE@", value);
                }
            })
        };

        let state = Rc::new(VolumeState {
            widgets,
            audio: AudioBackend::new(),
            media: MediaBackend::new(),
            sinks: RefCell::new(Vec::new()),
            sources: RefCell::new(Vec::new()),
            updating_sliders: Cell::new(false),
            media_poll: Cell::new(None),
            output_debouncer,
            input_debouncer,
        });
        *state_slot.borrow_mut() = Some(state.clone());

        {
            let state = state.clone();
            out_slider.connect_value_changed(move |scale| {
                if state.updating_sliders.get() {
                    return;
                }
                let val = scale.value().round();
                state.widgets.out_vol_val.set_label(&format!("{val:.0}%"));
                state.output_debouncer.schedule(val);
            });
        }
        {
            let state = state.clone();
            in_slider.connect_value_changed(move |scale| {
                if state.updating_sliders.get() {
                    return;
                }
                let val = scale.value().round();
                state.widgets.in_vol_val.set_label(&format!("{val:.0}%"));
                state.input_debouncer.schedule(val);
            });
        }
        {
            let state = state.clone();
            out_mute_btn.connect_clicked(move |_| toggle_mute(&state, "@DEFAULT_AUDIO_SINK@"));
        }
        {
            let state = state.clone();
            in_mute_btn.connect_clicked(move |_| toggle_mute(&state, "@DEFAULT_AUDIO_SOURCE@"));
        }
        {
            let state = state.clone();
            sink_list.connect_row_activated(move |_, row| {
                let index = row.index() as usize;
                let Some(device) = state.sinks.borrow().get(index).cloned() else {
                    return;
                };
                let state = state.clone();
                let audio = state.audio.clone();
                spawn_result(
                    move || audio.set_default_sink(&device.name),
                    {
                        let state = state.clone();
                        move |_| refresh_devices(&state)
                    },
                    {
                        let state = state.clone();
                        move |err| show_toast(&state, &format!("Output selection failed: {err}"))
                    },
                );
            });
        }
        {
            let state = state.clone();
            source_list.connect_row_activated(move |_, row| {
                let index = row.index() as usize;
                let Some(device) = state.sources.borrow().get(index).cloned() else {
                    return;
                };
                let state = state.clone();
                let audio = state.audio.clone();
                spawn_result(
                    move || audio.set_default_source(&device.name),
                    {
                        let state = state.clone();
                        move |_| refresh_devices(&state)
                    },
                    {
                        let state = state.clone();
                        move |err| show_toast(&state, &format!("Input selection failed: {err}"))
                    },
                );
            });
        }
        {
            let state = state.clone();
            prev_btn.connect_clicked(move |_| media_command(&state, "previous"));
        }
        {
            let state = state.clone();
            play_btn.connect_clicked(move |_| media_command(&state, "play-pause"));
        }
        {
            let state = state.clone();
            next_btn.connect_clicked(move |_| media_command(&state, "next"));
        }

        let focus_guard = Rc::new(RefCell::new(FocusLossGuard::new()));
        install_popup_dismiss(&window, focus_guard.clone());
        {
            let state = state.clone();
            window.connect_notify_local(Some("visible"), move |win, _| {
                if !win.is_visible() {
                    stop_media_poll(&state);
                    state.output_debouncer.cancel();
                    state.input_debouncer.cancel();
                }
            });
        }

        load_css();

        Self {
            window,
            focus_guard,
            state,
        }
    }

    pub fn present(&self) {
        present_popup(&self.window, &self.focus_guard);
        refresh_devices(&self.state);
        start_media_poll(&self.state);
    }
}

fn labeled_row(title: &str) -> (gtk4::Box, gtk4::Label) {
    let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    let label = gtk4::Label::new(Some(title));
    label.add_css_class("bold-label");
    label.set_hexpand(true);
    label.set_halign(gtk4::Align::Start);
    let value = gtk4::Label::new(Some("0%"));
    value.add_css_class("dim-label");
    row.append(&label);
    row.append(&value);
    (row, value)
}

fn show_toast(state: &VolumeState, text: &str) {
    state.widgets.toast.add_toast(adw::Toast::new(text));
}

fn set_volume(state: &Rc<VolumeState>, target: &str, value: f64) {
    let audio = state.audio.clone();
    let target = target.to_string();
    let state = state.clone();
    spawn_result(
        move || audio.set_volume(&target, value),
        |_| {},
        move |err| show_toast(&state, &format!("Volume update failed: {err}")),
    );
}

fn toggle_mute(state: &Rc<VolumeState>, target: &str) {
    let audio = state.audio.clone();
    let target = target.to_string();
    let state = state.clone();
    spawn_result(
        move || audio.toggle_mute(&target),
        {
            let state = state.clone();
            move |_| refresh_devices(&state)
        },
        move |err| show_toast(&state, &format!("Mute toggle failed: {err}")),
    );
}

fn media_command(state: &Rc<VolumeState>, action: &str) {
    let media = state.media.clone();
    let action = action.to_string();
    let state = state.clone();
    spawn_result(
        move || media.command(&action),
        {
            let state = state.clone();
            move |_| poll_media_once(&state)
        },
        move |err| show_toast(&state, &format!("Media control failed: {err}")),
    );
}

fn refresh_devices(state: &Rc<VolumeState>) {
    let audio = state.audio.clone();
    let state = state.clone();
    spawn_result(
        move || audio.list_devices(),
        {
            let state = state.clone();
            move |snapshot| apply_snapshot(&state, snapshot)
        },
        move |err| show_toast(&state, &format!("Audio devices unavailable: {err}")),
    );
}

fn apply_snapshot(state: &VolumeState, snapshot: AudioSnapshot) {
    *state.sinks.borrow_mut() = snapshot.sinks.clone();
    *state.sources.borrow_mut() = snapshot.sources.clone();

    while let Some(row) = state.widgets.sink_list.row_at_index(0) {
        state.widgets.sink_list.remove(&row);
    }
    while let Some(row) = state.widgets.source_list.row_at_index(0) {
        state.widgets.source_list.remove(&row);
    }

    let mut active_sink = None;
    for sink in &snapshot.sinks {
        let active = sink.name == snapshot.default_sink;
        state
            .widgets
            .sink_list
            .append(&device_row(sink, active, true));
        if active {
            active_sink = Some(sink.clone());
        }
    }
    let mut active_source = None;
    for source in &snapshot.sources {
        let active = source.name == snapshot.default_source;
        state
            .widgets
            .source_list
            .append(&device_row(source, active, false));
        if active {
            active_source = Some(source.clone());
        }
    }

    state.updating_sliders.set(true);
    if let Some(sink) = active_sink {
        state.widgets.out_slider.set_value(sink.volume_percent.round());
        state
            .widgets
            .out_vol_val
            .set_label(&format!("{:.0}%", sink.volume_percent));
        state.widgets.out_mute_btn.set_icon_name(if sink.mute {
            "audio-volume-muted-symbolic"
        } else {
            "audio-volume-high-symbolic"
        });
    }
    if let Some(source) = active_source {
        state
            .widgets
            .in_slider
            .set_value(source.volume_percent.round());
        state
            .widgets
            .in_vol_val
            .set_label(&format!("{:.0}%", source.volume_percent));
        state.widgets.in_mute_btn.set_icon_name(if source.mute {
            "audio-input-microphone-muted-symbolic"
        } else {
            "audio-input-microphone-symbolic"
        });
    }
    state.updating_sliders.set(false);
}

fn device_row(device: &AudioDevice, active: bool, is_sink: bool) -> gtk4::ListBoxRow {
    let row = gtk4::ListBoxRow::new();
    let box_ = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
    box_.add_css_class("device-row");

    let icon_name = if is_sink || device.media_class.to_ascii_lowercase().contains("sink") {
        "audio-card-symbolic"
    } else {
        "audio-input-microphone-symbolic"
    };
    let icon = gtk4::Image::from_icon_name(icon_name);
    if active {
        icon.add_css_class("accent-icon");
    }
    box_.append(&icon);

    let label = gtk4::Label::new(Some(&device.description));
    label.set_hexpand(true);
    label.set_halign(gtk4::Align::Start);
    if active {
        label.add_css_class("bold-label");
    }
    box_.append(&label);

    if active {
        let check = gtk4::Image::from_icon_name("object-select-symbolic");
        check.add_css_class("accent-icon");
        box_.append(&check);
    }

    row.set_child(Some(&box_));
    row
}

fn start_media_poll(state: &Rc<VolumeState>) {
    stop_media_poll(state);
    poll_media_once(state);
    let state_c = state.clone();
    let id = glib::timeout_add_seconds_local(2, move || {
        poll_media_once(&state_c);
        glib::ControlFlow::Continue
    });
    state.media_poll.set(Some(id));
}

fn stop_media_poll(state: &VolumeState) {
    if let Some(id) = state.media_poll.take() {
        id.remove();
    }
}

fn poll_media_once(state: &Rc<VolumeState>) {
    let media = state.media.clone();
    let state = state.clone();
    spawn_result(
        move || media.current(),
        move |current| apply_media(&state, current.unwrap_or_default()),
        |_| {},
    );
}

fn apply_media(state: &VolumeState, media: MediaState) {
    if !media.has_track() {
        state.widgets.media_card.set_visible(false);
        return;
    }
    state.widgets.media_title.set_label(&media.title);
    state.widgets.media_artist.set_label(&media.artist);
    state.widgets.play_btn.set_icon_name(if media.playing() {
        "media-playback-pause-symbolic"
    } else {
        "media-playback-start-symbolic"
    });
    state.widgets.media_card.set_visible(true);

    if media.artwork_url.is_empty() {
        return;
    }
    let backend = state.media.clone();
    let url = media.artwork_url.clone();
    let art = state.widgets.media_art.clone();
    crate::task::spawn_result(
        move || backend.fetch_artwork(&url),
        move |bytes| {
            if let Some(texture) = crate::artwork::texture_from_bytes(&bytes, 64) {
                art.set_paintable(Some(&texture));
            } else {
                art.set_icon_name(Some("audio-x-generic-symbolic"));
            }
        },
        {
            let art = state.widgets.media_art.clone();
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
                border-radius: 12px;
                padding: 16px;
            }
            .bold-label { font-weight: bold; color: @view_fg_color; }
            .dim-label { font-size: 0.85em; opacity: 0.6; }
            .device-list-sub {
                background: alpha(@view_fg_color, 0.02);
                border: 1px solid alpha(@view_fg_color, 0.08);
                border-radius: 12px;
                padding: 4px;
            }
            .device-row { padding: 10px 14px; border-radius: 8px; }
            .device-list-sub row:hover { background: alpha(@accent_bg_color, 0.1); }
            .accent-icon { color: @accent_color; }
            .media-card {
                background: alpha(@window_bg_color, 0.5);
                border: 1px solid alpha(@view_fg_color, 0.1);
                border-radius: 14px;
                padding: 12px;
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
