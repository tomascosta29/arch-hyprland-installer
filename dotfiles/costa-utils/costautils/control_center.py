#!/usr/bin/env python3
import os
import subprocess

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Adw, Gdk, Gtk, Pango

from .backends.jobs import Debouncer


class ControlCenterWindow(Adw.ApplicationWindow):
    def __init__(self, app):
        super().__init__(application=app, title="Control Menu")
        self.set_default_size(460, 560)
        self.set_resizable(False)
        self.set_modal(True)

        self.updating_sliders = False
        self.jobs = app.jobs
        self.audio = app.audio
        self.media = app.media
        self.nightlight = app.nightlight
        self.network = app.network
        self.bluetooth = app.bluetooth
        self.volume_debouncer = Debouncer(90, lambda value: self._set_volume(value))
        self.brightness_debouncer = Debouncer(120, lambda value: self._set_brightness(value))
        self.adapter_path = None

        self.build_ui()
        self.load_css()

        # Initial reload
        self.refresh_states()
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
            self.volume_debouncer.cancel()
            self.brightness_debouncer.cancel()
            self.hide()
            return True
        return False

    def on_close_request(self, win):
        self.stop_media_monitor()
        self.volume_debouncer.cancel()
        self.brightness_debouncer.cancel()
        self.hide()
        return True

    def on_is_active_changed(self, window, pspec):
        is_act = self.is_active() if hasattr(self, "is_active") else self.get_property("is-active")
        if not is_act:
            self.stop_media_monitor()
            self.volume_debouncer.cancel()
            self.brightness_debouncer.cancel()
            self.hide()

    def build_ui(self):
        self.toast_overlay = Adw.ToastOverlay()
        self.set_content(self.toast_overlay)

        view = Adw.ToolbarView()
        self.toast_overlay.set_child(view)

        header = Adw.HeaderBar()
        title = Gtk.Label(label="Control Center")
        title.set_markup("<b>Control Center</b>")
        header.set_title_widget(title)
        view.add_top_bar(header)

        # Main box
        main_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=16)
        main_box.set_margin_start(16)
        main_box.set_margin_end(16)
        main_box.set_margin_top(16)
        main_box.set_margin_bottom(16)
        view.set_content(main_box)

        # --- TOGGLES GRID ---
        grid = Gtk.FlowBox()
        grid.set_valign(Gtk.Align.START)
        grid.set_halign(Gtk.Align.FILL)
        grid.set_selection_mode(Gtk.SelectionMode.NONE)
        grid.set_max_children_per_line(2)
        grid.set_min_children_per_line(2)
        grid.set_column_spacing(12)
        grid.set_row_spacing(12)
        main_box.append(grid)

        # Wi-Fi Toggle Button
        self.wifi_btn = self.make_toggle_button(
            "network-wireless-symbolic", "Wi-Fi", "Disconnected", self.on_wifi_toggled
        )
        grid.append(self.wifi_btn)

        # Bluetooth Toggle Button
        self.bt_btn = self.make_toggle_button(
            "bluetooth-active-symbolic", "Bluetooth", "Disabled", self.on_bluetooth_toggled
        )
        grid.append(self.bt_btn)

        # Night Light Toggle Button
        self.nl_btn = self.make_toggle_button(
            "night-light-symbolic", "Night Light", "Off", self.on_nightlight_toggled
        )
        grid.append(self.nl_btn)

        # Do Not Disturb Toggle Button
        self.dnd_btn = self.make_toggle_button(
            "notifications-disabled-symbolic", "Do Not Disturb", "Off", self.on_dnd_toggled
        )
        grid.append(self.dnd_btn)

        # --- SLIDERS SECTION ---
        sliders_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=12)
        sliders_box.add_css_class("card-box")
        main_box.append(sliders_box)

        # Output Volume
        vol_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
        vol_icon = Gtk.Image.new_from_icon_name("audio-volume-high-symbolic")
        self.vol_slider = Gtk.Scale.new_with_range(Gtk.Orientation.HORIZONTAL, 0, 100, 1)
        self.vol_slider.set_draw_value(False)
        self.vol_slider.set_hexpand(True)
        self.vol_slider.connect("value-changed", self.on_vol_slider_changed)
        vol_box.append(vol_icon)
        vol_box.append(self.vol_slider)
        sliders_box.append(vol_box)

        # Brightness
        bright_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
        bright_icon = Gtk.Image.new_from_icon_name("display-brightness-symbolic")
        self.bright_slider = Gtk.Scale.new_with_range(Gtk.Orientation.HORIZONTAL, 0, 100, 1)
        self.bright_slider.set_draw_value(False)
        self.bright_slider.set_hexpand(True)
        self.bright_slider.connect("value-changed", self.on_bright_slider_changed)
        bright_box.append(bright_icon)
        bright_box.append(self.bright_slider)
        sliders_box.append(bright_box)

        # --- MEDIA PLAYER CARD ---
        self.media_card = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=12)
        self.media_card.add_css_class("media-card")
        self.media_card.set_visible(False)
        main_box.append(self.media_card)

        self.media_art = Gtk.Image()
        self.media_art.set_size_request(48, 48)
        self.media_art.set_pixel_size(48)
        self.media_art.add_css_class("media-art")
        self.media_card.append(self.media_art)

        info_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=2)
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

        controls_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=4)
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

        # --- SESSION QUICK ACTIONS ---
        session_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
        session_box.add_css_class("card-box")
        session_box.set_halign(Gtk.Align.FILL)
        main_box.append(session_box)

        def make_action_btn(icon_name, tooltip, cmd):
            btn = Gtk.Button(icon_name=icon_name)
            btn.set_hexpand(True)
            btn.set_tooltip_text(tooltip)
            btn.add_css_class("power-action-btn")
            btn.connect("clicked", lambda _: self.run_session_cmd(cmd))
            return btn

        session_box.append(make_action_btn("system-lock-screen-symbolic", "Lock Session", "lock"))
        session_box.append(
            make_action_btn(
                "system-shutdown-symbolic",
                "Open Power Menu",
                [os.path.expanduser("~/.local/bin/costa-utils"), "--power-menu"],
            )
        )

    def make_toggle_button(self, icon_name, title, subtitle, callback):
        btn = Gtk.Button()
        btn.add_css_class("toggle-card")
        btn.connect("clicked", callback)

        box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=12)
        box.set_valign(Gtk.Align.CENTER)

        icon = Gtk.Image.new_from_icon_name(icon_name)
        icon.set_pixel_size(24)
        box.append(icon)

        lbl_vbox = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=2)
        lbl_vbox.set_hexpand(True)
        lbl_vbox.set_halign(Gtk.Align.START)

        title_lbl = Gtk.Label(label=title)
        title_lbl.add_css_class("toggle-title")

        sub_lbl = Gtk.Label(label=subtitle)
        sub_lbl.add_css_class("toggle-subtitle")

        lbl_vbox.append(title_lbl)
        lbl_vbox.append(sub_lbl)
        box.append(lbl_vbox)

        btn.set_child(box)
        btn.icon = icon
        btn.title_lbl = title_lbl
        btn.sub_lbl = sub_lbl
        btn.active_state = False
        return btn

    def show_toast(self, text):
        toast = Adw.Toast.new(text)
        self.toast_overlay.add_toast(toast)

    def refresh_states(self):
        def audio_bright_worker():
            volume, _muted = self.audio.get_default_volume("@DEFAULT_AUDIO_SINK@")
            bright_val = 50
            result = subprocess.run(
                ["brightnessctl", "--machine-readable"],
                capture_output=True,
                text=True,
                timeout=5,
            )
            if result.returncode == 0:
                bright_val = int(result.stdout.strip().split(",")[3].rstrip("%"))
            return volume, bright_val

        def extras_worker():
            result = subprocess.run(
                ["dunstctl", "is-paused"],
                capture_output=True,
                text=True,
                timeout=5,
            )
            return result.stdout.strip() == "true", self.nightlight.query()

        self.jobs.submit(
            "control-sliders-refresh",
            audio_bright_worker,
            on_success=lambda values: self.update_sliders_ui(*values),
            on_error=lambda error: self.show_toast(f"Controls unavailable: {error}"),
        )
        self.jobs.submit(
            "control-network-refresh",
            self.network.active_status,
            on_success=lambda values: self.update_wifi_ui(*values),
            on_error=lambda error: self.show_toast(f"Wi-Fi status unavailable: {error}"),
        )
        self.jobs.submit(
            "control-bluetooth-refresh",
            self.bluetooth.query,
            on_success=self._update_bluetooth_state,
            on_error=lambda error: self.show_toast(f"Bluetooth unavailable: {error}"),
        )
        self.jobs.submit(
            "control-extras-refresh",
            extras_worker,
            on_success=lambda values: self.update_extras_ui(*values),
            on_error=lambda error: self.show_toast(f"Desktop state unavailable: {error}"),
        )

    def _update_bluetooth_state(self, state):
        self.adapter_path = state.adapter_path
        connected = sum(device["connected"] for device in state.devices)
        subtitle = "Disabled"
        if state.powered:
            subtitle = f"{connected} Connected" if connected else "On"
        self.update_bluetooth_ui(state.powered, subtitle)

    def update_sliders_ui(self, vol, bright):
        self.updating_sliders = True
        self.vol_slider.set_value(vol)
        self.bright_slider.set_value(bright)
        self.updating_sliders = False

    def update_wifi_ui(self, active, ssid):
        self.wifi_btn.active_state = active
        self.wifi_btn.sub_lbl.set_label(ssid)
        if active or ssid not in ("Disconnected", "Disabled"):
            self.wifi_btn.add_css_class("active")
        else:
            self.wifi_btn.remove_css_class("active")

    def update_bluetooth_ui(self, powered, subtitle):
        self.bt_btn.active_state = powered
        self.bt_btn.sub_lbl.set_label(subtitle)
        if powered:
            self.bt_btn.add_css_class("active")
        else:
            self.bt_btn.remove_css_class("active")

    def update_extras_ui(self, dnd, nl):
        self.dnd_btn.active_state = dnd
        self.dnd_btn.sub_lbl.set_label("On" if dnd else "Off")
        if dnd:
            self.dnd_btn.add_css_class("active")
        else:
            self.dnd_btn.remove_css_class("active")

        self.nl_btn.active_state = nl
        self.nl_btn.sub_lbl.set_label("On" if nl else "Off")
        if nl:
            self.nl_btn.add_css_class("active")
        else:
            self.nl_btn.remove_css_class("active")

    # --- SLIDER HANDLERS ---
    def on_vol_slider_changed(self, scale):
        if self.updating_sliders:
            return
        val = int(scale.get_value())

        self.volume_debouncer.schedule(val)

    def on_bright_slider_changed(self, scale):
        if self.updating_sliders:
            return
        val = int(scale.get_value())

        self.brightness_debouncer.schedule(val)

    def _set_volume(self, value):
        self.jobs.submit(
            "control-volume",
            self.audio.set_volume,
            "@DEFAULT_AUDIO_SINK@",
            value,
            on_error=lambda error: self.show_toast(f"Volume update failed: {error}"),
        )

    @staticmethod
    def _brightness_worker(value):
        subprocess.run(
            ["brightnessctl", "set", f"{value}%"],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            timeout=5,
            check=True,
        )

    def _set_brightness(self, value):
        self.jobs.submit(
            "control-brightness",
            self._brightness_worker,
            value,
            on_error=lambda error: self.show_toast(f"Brightness unavailable: {error}"),
        )

    # --- TOGGLE HANDLERS ---
    def on_wifi_toggled(self, btn):
        self.jobs.submit(
            "control-wifi-toggle",
            self.network.set_radio,
            not btn.active_state,
            on_success=lambda _result: self.refresh_states(),
            on_error=lambda error: self.show_toast(f"Wi-Fi toggle failed: {error}"),
        )

    def on_bluetooth_toggled(self, btn):
        if not self.adapter_path:
            self.show_toast("No Bluetooth adapter found")
            return
        self.jobs.submit(
            "control-bluetooth-toggle",
            self.bluetooth.set_power,
            self.adapter_path,
            not btn.active_state,
            on_success=lambda _result: self.refresh_states(),
            on_error=lambda error: self.show_toast(f"Bluetooth toggle failed: {error}"),
        )

    def on_nightlight_toggled(self, btn):
        requested = not btn.active_state
        self.jobs.submit(
            "control-nightlight",
            self.nightlight.set_enabled,
            requested,
            on_success=lambda enabled: self.update_extras_ui(self.dnd_btn.active_state, enabled),
            on_error=lambda error: self.show_toast(f"Night light failed: {error}"),
        )

    def on_dnd_toggled(self, btn):
        state = "false" if btn.active_state else "true"

        def worker():
            subprocess.run(
                ["dunstctl", "set-paused", state],
                timeout=5,
                check=True,
            )

        self.jobs.submit(
            "control-dnd-toggle",
            worker,
            on_success=lambda _result: self.refresh_states(),
            on_error=lambda error: self.show_toast(f"Do Not Disturb failed: {error}"),
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
            48,
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

    # --- SESSION ACTIONS ---
    def run_session_cmd(self, cmd):
        self.hide()
        if cmd == "lock":
            subprocess.Popen(["loginctl", "lock-session"])
        else:
            try:
                subprocess.Popen(cmd)
            except Exception as e:
                print(f"Error: {e}")

    def load_css(self):
        css = b"""
        window { background: alpha(@window_bg_color, 0.95); }
        .toggle-card {
            background: alpha(@view_fg_color, 0.04);
            border: 1px solid alpha(@view_fg_color, 0.08);
            border-radius: 12px;
            padding: 12px 14px;
            transition: all 150ms ease-in-out;
        }
        .toggle-card:hover {
            background: alpha(@view_fg_color, 0.08);
        }
        .toggle-card.active {
            background: @accent_bg_color;
            color: @accent_fg_color;
            border-color: @accent_bg_color;
        }
        .toggle-card.active .dim-label {
            color: @accent_fg_color;
            opacity: 0.8;
        }
        .toggle-title {
            font-size: 0.95em;
            font-weight: bold;
        }
        .toggle-subtitle {
            font-size: 0.8em;
            opacity: 0.6;
        }
        .toggle-card.active .toggle-subtitle {
            color: @accent_fg_color;
            opacity: 0.8;
        }
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
        .media-card {
            background: alpha(@window_bg_color, 0.5);
            border: 1px solid alpha(@view_fg_color, 0.1);
            border-radius: 14px;
            padding: 10px;
        }
        .media-art {
            border-radius: 6px;
            background: alpha(@view_fg_color, 0.1);
        }
        .power-action-btn {
            padding: 10px;
            border-radius: 10px;
            background: alpha(@view_fg_color, 0.04);
            border: 1px solid alpha(@view_fg_color, 0.08);
            transition: all 150ms;
        }
        .power-action-btn:hover {
            background: alpha(@accent_bg_color, 0.2);
            border-color: @accent_bg_color;
            color: @accent_color;
        }
        """
        provider = Gtk.CssProvider()
        provider.load_from_data(css)
        Gtk.StyleContext.add_provider_for_display(
            Gdk.Display.get_default(), provider, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION
        )
