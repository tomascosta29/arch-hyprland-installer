#!/usr/bin/env python3
import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Adw, Gdk, Gtk, Pango

from .backends.audio import channel_volume_percent
from .backends.jobs import Debouncer


class VolumeWindow(Adw.ApplicationWindow):
    def __init__(self, app):
        super().__init__(application=app, title="Volume Menu")
        self.set_default_size(480, 520)
        self.set_resizable(False)
        self.set_modal(True)

        self.sinks = []
        self.sources = []
        self.jobs = app.jobs
        self.audio = app.audio
        self.media = app.media
        self.updating_sliders = False
        self.output_debouncer = Debouncer(
            90, lambda value: self._set_volume("@DEFAULT_AUDIO_SINK@", value)
        )
        self.input_debouncer = Debouncer(
            90, lambda value: self._set_volume("@DEFAULT_AUDIO_SOURCE@", value)
        )

        self.build_ui()
        self.load_css()

        # Initial reload
        self.refresh_audio_devices()
        self.start_media_monitor()
        self.setup_keyboard()

        self.connect("close-request", self.on_close_request)
        self.connect("notify::is-active", self.on_is_active_changed)

    def setup_keyboard(self):
        ctrl = Gtk.EventControllerKey()
        ctrl.connect("key-pressed", self.on_key_pressed)
        self.add_controller(ctrl)

    def on_key_pressed(self, _, keyval, keycode, state):
        name = Gtk.accelerator_name(keyval, state)
        if name == "Escape":
            self.stop_media_monitor()
            self.output_debouncer.cancel()
            self.input_debouncer.cancel()
            self.hide()
            return True
        return False

    def on_close_request(self, win):
        self.stop_media_monitor()
        self.output_debouncer.cancel()
        self.input_debouncer.cancel()
        self.jobs.invalidate("volume-refresh")
        self.hide()
        return True

    def on_is_active_changed(self, window, pspec):
        is_act = self.is_active() if hasattr(self, "is_active") else self.get_property("is-active")
        if not is_act:
            self.stop_media_monitor()
            self.output_debouncer.cancel()
            self.input_debouncer.cancel()
            self.hide()

    def build_ui(self):
        self.toast_overlay = Adw.ToastOverlay()
        self.set_content(self.toast_overlay)

        view = Adw.ToolbarView()
        self.toast_overlay.set_child(view)

        header = Adw.HeaderBar()
        title = Gtk.Label(label="Audio & Media")
        title.set_markup("<b>Audio & Media</b>")
        header.set_title_widget(title)
        view.add_top_bar(header)

        # Main box
        main_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=16)
        main_box.set_margin_start(16)
        main_box.set_margin_end(16)
        main_box.set_margin_top(16)
        main_box.set_margin_bottom(16)
        view.set_content(main_box)

        # --- SLIDERS SECTION ---
        sliders_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=12)
        sliders_box.add_css_class("card-box")
        main_box.append(sliders_box)

        # Output Volume
        out_lbl_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL)
        out_lbl = Gtk.Label(label="Output Volume")
        out_lbl.add_css_class("bold-label")
        out_lbl.set_hexpand(True)
        out_lbl.set_halign(Gtk.Align.START)
        self.out_vol_val = Gtk.Label(label="0%")
        self.out_vol_val.add_css_class("dim-label")
        out_lbl_box.append(out_lbl)
        out_lbl_box.append(self.out_vol_val)
        sliders_box.append(out_lbl_box)

        out_slider_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
        self.out_mute_btn = Gtk.Button(icon_name="audio-volume-high-symbolic")
        self.out_mute_btn.add_css_class("flat")
        self.out_mute_btn.connect("clicked", self.on_output_mute_clicked)

        self.out_slider = Gtk.Scale.new_with_range(Gtk.Orientation.HORIZONTAL, 0, 150, 1)
        self.out_slider.set_draw_value(False)
        self.out_slider.set_hexpand(True)
        self.out_slider.connect("value-changed", self.on_output_slider_changed)

        out_slider_box.append(self.out_mute_btn)
        out_slider_box.append(self.out_slider)
        sliders_box.append(out_slider_box)

        # Input Volume
        in_lbl_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL)
        in_lbl = Gtk.Label(label="Input Volume")
        in_lbl.add_css_class("bold-label")
        in_lbl.set_hexpand(True)
        in_lbl.set_halign(Gtk.Align.START)
        self.in_vol_val = Gtk.Label(label="0%")
        self.in_vol_val.add_css_class("dim-label")
        in_lbl_box.append(in_lbl)
        in_lbl_box.append(self.in_vol_val)
        sliders_box.append(in_lbl_box)

        in_slider_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
        self.in_mute_btn = Gtk.Button(icon_name="audio-input-microphone-symbolic")
        self.in_mute_btn.add_css_class("flat")
        self.in_mute_btn.connect("clicked", self.on_input_mute_clicked)

        self.in_slider = Gtk.Scale.new_with_range(Gtk.Orientation.HORIZONTAL, 0, 150, 1)
        self.in_slider.set_draw_value(False)
        self.in_slider.set_hexpand(True)
        self.in_slider.connect("value-changed", self.on_input_slider_changed)

        in_slider_box.append(self.in_mute_btn)
        in_slider_box.append(self.in_slider)
        sliders_box.append(in_slider_box)

        # --- DEVICE SELECTION TABS ---
        device_stack = Gtk.Stack()
        device_stack.set_transition_type(Gtk.StackTransitionType.CROSSFADE)
        device_stack.set_vexpand(True)

        self.sink_listbox = Gtk.ListBox()
        self.sink_listbox.add_css_class("device-list-sub")
        self.sink_listbox.connect("row-activated", self.on_sink_activated)
        sink_scrolled = Gtk.ScrolledWindow()
        sink_scrolled.set_child(self.sink_listbox)
        device_stack.add_titled(sink_scrolled, "output", "Outputs")

        self.source_listbox = Gtk.ListBox()
        self.source_listbox.add_css_class("device-list-sub")
        self.source_listbox.connect("row-activated", self.on_source_activated)
        source_scrolled = Gtk.ScrolledWindow()
        source_scrolled.set_child(self.source_listbox)
        device_stack.add_titled(source_scrolled, "input", "Inputs")

        switcher = Gtk.StackSwitcher()
        switcher.set_stack(device_stack)
        switcher.set_halign(Gtk.Align.CENTER)
        main_box.append(switcher)
        main_box.append(device_stack)

        # --- MEDIA PLAYER CARD ---
        self.media_card = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=12)
        self.media_card.add_css_class("media-card")
        self.media_card.set_visible(False)
        main_box.append(self.media_card)

        self.media_art = Gtk.Image()
        self.media_art.set_size_request(64, 64)
        self.media_art.set_pixel_size(64)
        self.media_art.add_css_class("media-art")
        self.media_card.append(self.media_art)

        info_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4)
        info_box.set_hexpand(True)
        self.media_title = Gtk.Label(label="Unknown Title", xalign=0)
        self.media_title.add_css_class("bold-label")
        self.media_title.set_ellipsize(Pango.EllipsizeMode.END)
        self.media_artist = Gtk.Label(label="Unknown Artist", xalign=0)
        self.media_artist.add_css_class("dim-label")
        self.media_artist.set_ellipsize(Pango.EllipsizeMode.END)
        info_box.append(self.media_title)
        info_box.append(self.media_artist)
        self.media_card.append(info_box)

        controls_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=6)
        controls_box.set_valign(Gtk.Align.CENTER)

        prev_btn = Gtk.Button(icon_name="media-skip-backward-symbolic")
        prev_btn.add_css_class("flat")
        prev_btn.connect("clicked", lambda _: self.run_player_cmd("previous"))

        self.play_btn = Gtk.Button(icon_name="media-playback-start-symbolic")
        self.play_btn.add_css_class("flat")
        self.play_btn.connect("clicked", lambda _: self.run_player_cmd("play-pause"))

        next_btn = Gtk.Button(icon_name="media-skip-forward-symbolic")
        next_btn.add_css_class("flat")
        next_btn.connect("clicked", lambda _: self.run_player_cmd("next"))

        controls_box.append(prev_btn)
        controls_box.append(self.play_btn)
        controls_box.append(next_btn)
        self.media_card.append(controls_box)

    def show_toast(self, text):
        toast = Adw.Toast.new(text)
        self.toast_overlay.add_toast(toast)

    def refresh_audio_devices(self):
        self.jobs.submit(
            "volume-refresh",
            self.audio.list_devices,
            on_success=lambda result: self.update_devices_ui(
                result[0],
                [source for source in result[1] if ".monitor" not in source.get("name", "")],
                result[2],
                result[3],
            ),
            on_error=lambda error: self.show_toast(f"Audio devices unavailable: {error}"),
        )

    def update_devices_ui(self, sinks, sources, def_sink, def_source):
        self.sinks = sinks
        self.sources = sources

        # Load lists
        self.sink_listbox.remove_all()
        self.source_listbox.remove_all()

        active_sink_obj = None
        for s in sinks:
            active = s["name"] == def_sink
            row = self.make_device_row(s, active)
            row.dev_name = s["name"]
            row.is_sink = True
            self.sink_listbox.append(row)
            if active:
                active_sink_obj = s

        active_source_obj = None
        for src in sources:
            active = src["name"] == def_source
            row = self.make_device_row(src, active)
            row.dev_name = src["name"]
            row.is_sink = False
            self.source_listbox.append(row)
            if active:
                active_source_obj = src

        # Update sliders. Always reset the guard flag, even on error,
        # otherwise the value-changed handler short-circuits forever.
        self.updating_sliders = True
        try:
            if active_sink_obj:
                vol_pct = channel_volume_percent(active_sink_obj)
                self.out_slider.set_value(round(vol_pct))
                self.out_vol_val.set_label(f"{vol_pct:.0f}%")
                self.out_mute_btn.set_icon_name(
                    "audio-volume-muted-symbolic"
                    if active_sink_obj["mute"]
                    else "audio-volume-high-symbolic"
                )
                self.active_sink_name = def_sink

            if active_source_obj:
                vol_pct = channel_volume_percent(active_source_obj)
                self.in_slider.set_value(round(vol_pct))
                self.in_vol_val.set_label(f"{vol_pct:.0f}%")
                self.in_mute_btn.set_icon_name(
                    "audio-input-microphone-muted-symbolic"
                    if active_source_obj["mute"]
                    else "audio-input-microphone-symbolic"
                )
                self.active_source_name = def_source
        finally:
            self.updating_sliders = False

    def make_device_row(self, device, active):
        row = Gtk.ListBoxRow()
        box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=12)
        box.add_css_class("device-row")

        icon_name = (
            "audio-card-symbolic"
            if "sink" in device.get("media.class", "").lower()
            else "audio-input-microphone-symbolic"
        )
        icon = Gtk.Image.new_from_icon_name(icon_name)
        if active:
            icon.add_css_class("accent-icon")
        box.append(icon)

        lbl = Gtk.Label(label=device.get("description", device.get("name", "Unknown")))
        lbl.set_hexpand(True)
        lbl.set_halign(Gtk.Align.START)
        if active:
            lbl.add_css_class("bold-label")
        box.append(lbl)

        if active:
            check = Gtk.Image.new_from_icon_name("object-select-symbolic")
            check.add_css_class("accent-icon")
            box.append(check)

        row.set_child(box)
        return row

    def on_sink_activated(self, listbox, row):
        if not row:
            return

        self.jobs.submit(
            "volume-default-sink",
            self.audio.set_default_sink,
            row.dev_name,
            on_success=lambda _result: self.refresh_audio_devices(),
            on_error=lambda error: self.show_toast(f"Output selection failed: {error}"),
        )

    def on_source_activated(self, listbox, row):
        if not row:
            return

        self.jobs.submit(
            "volume-default-source",
            self.audio.set_default_source,
            row.dev_name,
            on_success=lambda _result: self.refresh_audio_devices(),
            on_error=lambda error: self.show_toast(f"Input selection failed: {error}"),
        )

    def on_output_slider_changed(self, scale):
        if self.updating_sliders:
            return
        val = int(scale.get_value())
        self.out_vol_val.set_label(f"{val}%")

        self.output_debouncer.schedule(val)

    def on_input_slider_changed(self, scale):
        if self.updating_sliders:
            return
        val = int(scale.get_value())
        self.in_vol_val.set_label(f"{val}%")

        self.input_debouncer.schedule(val)

    def _set_volume(self, target, value):
        self.jobs.submit(
            f"volume-set-{target}",
            self.audio.set_volume,
            target,
            value,
            on_error=lambda error: self.show_toast(f"Volume update failed: {error}"),
        )

    def on_output_mute_clicked(self, _):
        self.jobs.submit(
            "volume-mute-output",
            self.audio.toggle_mute,
            "@DEFAULT_AUDIO_SINK@",
            on_success=lambda _result: self.refresh_audio_devices(),
            on_error=lambda error: self.show_toast(f"Mute toggle failed: {error}"),
        )

    def on_input_mute_clicked(self, _):
        self.jobs.submit(
            "volume-mute-input",
            self.audio.toggle_mute,
            "@DEFAULT_AUDIO_SOURCE@",
            on_success=lambda _result: self.refresh_audio_devices(),
            on_error=lambda error: self.show_toast(f"Mute toggle failed: {error}"),
        )

    # --- MEDIA MONITORING ---
    def start_media_monitor(self):
        self.media.subscribe(self, self.update_media_ui)

    def stop_media_monitor(self):
        self.media.unsubscribe(self)

    def update_media_ui(self, state):
        status, title, artist, art_url = (
            state.status,
            state.title,
            state.artist,
            state.artwork_url,
        )
        if not title and not artist:
            self.media_card.set_visible(False)
            return

        self.media_title.set_label(title)
        self.media_artist.set_label(artist)
        self.play_btn.set_icon_name(
            "media-playback-pause-symbolic"
            if status == "Playing"
            else "media-playback-start-symbolic"
        )

        # Load art
        if art_url:
            self.load_art_async(art_url)
        else:
            self.media_art.set_from_icon_name("audio-x-generic-symbolic")

        self.media_card.set_visible(True)

    def load_art_async(self, url):
        self.media.load_artwork(
            self,
            url,
            64,
            lambda texture: (
                self.media_art.set_from_paintable(texture)
                if texture is not None
                else self.media_art.set_from_icon_name("audio-x-generic-symbolic")
            ),
        )

    def run_player_cmd(self, action):
        self.media.command(
            action,
            lambda error: self.show_toast(f"Media control failed: {error}"),
        )

    def load_css(self):
        css = b"""
        window { background: alpha(@window_bg_color, 0.95); }
        .card-box {
            background: alpha(@view_fg_color, 0.04);
            border: 1px solid alpha(@view_fg_color, 0.08);
            border-radius: 12px;
            padding: 16px;
        }
        .bold-label {
            font-weight: bold;
            color: @view_fg_color;
        }
        .dim-label {
            font-size: 0.85em;
            opacity: 0.6;
        }
        .device-list-sub {
            background: alpha(@view_fg_color, 0.02);
            border: 1px solid alpha(@view_fg_color, 0.08);
            border-radius: 12px;
            padding: 4px;
        }
        .device-row {
            padding: 10px 14px;
            border-radius: 8px;
            transition: all 150ms;
        }
        .device-list-sub row:hover {
            background: alpha(@accent_bg_color, 0.1);
        }
        .accent-icon {
            color: @accent_color;
        }
        .media-card {
            background: alpha(@window_bg_color, 0.5);
            border: 1px solid alpha(@view_fg_color, 0.1);
            border-radius: 14px;
            padding: 12px;
        }
        .media-art {
            border-radius: 8px;
            background: alpha(@view_fg_color, 0.1);
        }
        """
        provider = Gtk.CssProvider()
        provider.load_from_data(css)
        Gtk.StyleContext.add_provider_for_display(
            Gdk.Display.get_default(), provider, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION
        )
