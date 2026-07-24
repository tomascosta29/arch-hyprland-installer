#!/usr/bin/env python3
import subprocess

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Adw, Gdk, GLib, Gtk

from .backends.network import parse_nmcli_terse

__all__ = ["NetworkWindow", "parse_nmcli_terse"]


class NetworkWindow(Adw.ApplicationWindow):
    def __init__(self, app):
        super().__init__(application=app, title="Network Manager")
        self.set_default_size(480, 450)
        self.set_resizable(False)
        self.set_modal(True)

        self.networks = []
        self.connecting = False
        self.wifi_enabled = True
        self.jobs = app.jobs
        self.backend = app.network
        self.selected_network = None

        self.build_ui()
        self.load_css()

        # Initial reload
        self.refresh_networks()
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
            self._cancel_jobs()
            self.hide()
            return True
        return False

    def on_close_request(self, win):
        self._cancel_jobs()
        self.hide()
        return True

    def on_is_active_changed(self, window, pspec):
        if not self.is_active() and not self.connecting:
            self._cancel_jobs()
            self.hide()

    def _cancel_jobs(self):
        for key in ("network-scan", "network-profiles", "network-connect"):
            self.jobs.invalidate(key)

    def build_ui(self):
        self.toast_overlay = Adw.ToastOverlay()
        self.set_content(self.toast_overlay)

        view = Adw.ToolbarView()
        self.toast_overlay.set_child(view)

        header = Adw.HeaderBar()
        title = Gtk.Label(label="Network")
        title.set_markup("<b>Network</b>")
        header.set_title_widget(title)
        view.add_top_bar(header)

        # Refresh button
        self.refresh_btn = Gtk.Button(icon_name="view-refresh-symbolic")
        self.refresh_btn.set_tooltip_text("Refresh Wi-Fi list")
        self.refresh_btn.connect("clicked", lambda _: self.refresh_networks())
        header.pack_end(self.refresh_btn)

        # Main box
        main_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=12)
        main_box.set_margin_start(16)
        main_box.set_margin_end(16)
        main_box.set_margin_top(16)
        main_box.set_margin_bottom(16)
        view.set_content(main_box)

        # Wi-Fi Power Switch
        power_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL)
        power_box.add_css_class("card-box")

        power_icon = Gtk.Image.new_from_icon_name("network-wireless-symbolic")
        power_label = Gtk.Label(label="Wi-Fi Enable")
        power_label.set_hexpand(True)
        power_label.set_halign(Gtk.Align.START)

        self.wifi_switch = Gtk.Switch()
        self.wifi_switch.connect("state-set", self.on_wifi_switch_toggled)

        power_box.append(power_icon)
        power_box.append(power_label)
        power_box.append(self.wifi_switch)
        main_box.append(power_box)

        # Stack to switch between Wi-Fi list and overlays (Connecting/Password prompt)
        self.stack = Gtk.Stack()
        self.stack.set_transition_type(Gtk.StackTransitionType.CROSSFADE)
        self.stack.set_vexpand(True)
        main_box.append(self.stack)

        # Wi-Fi list scrolled window
        self.listbox = Gtk.ListBox()
        self.listbox.add_css_class("network-list")
        self.listbox.set_selection_mode(Gtk.SelectionMode.NONE)
        self.listbox.connect("row-activated", self.on_network_row_activated)

        scrolled = Gtk.ScrolledWindow()
        scrolled.set_child(self.listbox)
        scrolled.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
        self.stack.add_named(scrolled, "list")

        # Wi-Fi disabled status page
        self.disabled_status = Adw.StatusPage()
        self.disabled_status.set_title("Wi-Fi is Off")
        self.disabled_status.set_description("Enable Wi-Fi to scan for networks")
        self.disabled_status.set_icon_name("network-wireless-offline-symbolic")
        self.stack.add_named(self.disabled_status, "disabled")

        # Loading / Connecting page
        self.loading_status = Adw.StatusPage()
        self.loading_status.set_title("Connecting...")
        self.loading_status.set_icon_name("network-wireless-acquiring-symbolic")
        self.stack.add_named(self.loading_status, "loading")

        # Password Entry overlay
        self.password_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=16)
        self.password_box.set_valign(Gtk.Align.CENTER)
        self.password_box.set_halign(Gtk.Align.CENTER)
        self.password_box.add_css_class("password-card")

        self.password_title = Gtk.Label()
        self.password_title.add_css_class("password-title")
        self.password_box.append(self.password_title)

        self.password_entry = Gtk.Entry()
        self.password_entry.set_placeholder_text("Enter password")
        self.password_entry.set_visibility(False)
        self.password_entry.connect("activate", lambda _: self.on_connect_with_password_clicked())
        self.password_box.append(self.password_entry)

        actions_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=12)
        actions_box.set_halign(Gtk.Align.CENTER)

        cancel_btn = Gtk.Button(label="Cancel")
        cancel_btn.connect("clicked", lambda _: self.cancel_password_prompt())

        connect_btn = Gtk.Button(label="Connect")
        connect_btn.add_css_class("suggested-action")
        connect_btn.connect("clicked", lambda _: self.on_connect_with_password_clicked())

        actions_box.append(cancel_btn)
        actions_box.append(connect_btn)
        self.password_box.append(actions_box)
        self.stack.add_named(self.password_box, "password")

        self.stack.set_visible_child_name("list")

    def show_toast(self, text):
        toast = Adw.Toast.new(text)
        self.toast_overlay.add_toast(toast)

    def on_wifi_switch_toggled(self, switch, state):
        self.jobs.submit(
            "network-radio",
            self.backend.set_radio,
            state,
            on_success=lambda _result: self.refresh_networks(),
            on_error=lambda error: self.show_toast(f"Wi-Fi toggle failed: {error}"),
        )
        return True

    def refresh_networks(self):
        self.refresh_btn.set_sensitive(False)
        self.jobs.submit(
            "network-scan",
            self.backend.scan,
            on_success=self.update_list_ui,
            on_error=self.on_scan_error,
        )

    def on_scan_error(self, error):
        self.refresh_btn.set_sensitive(True)
        self.show_toast(f"Wi-Fi scan failed: {error}")

    def update_list_ui(self, state):
        self.wifi_enabled = state.enabled
        self.wifi_switch.set_active(state.enabled)
        self.networks = list(state.networks)
        self.listbox.remove_all()

        if not state.enabled:
            self.refresh_btn.set_sensitive(True)
            self.stack.set_visible_child_name("disabled")
            return

        for net in self.networks:
            self.listbox.append(self.make_network_row(net))

        self.refresh_btn.set_sensitive(True)
        self.stack.set_visible_child_name("list")

    def make_network_row(self, net):
        row = Gtk.ListBoxRow()
        row.ssid = net["ssid"]
        row.network = net
        row.active = net["active"]

        box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=12)
        box.add_css_class("network-row")

        # Signal Icon
        icon_name = "network-wireless-signal-none-symbolic"
        sig = net["signal"]
        if sig >= 80:
            icon_name = "network-wireless-signal-excellent-symbolic"
        elif sig >= 60:
            icon_name = "network-wireless-signal-good-symbolic"
        elif sig >= 40:
            icon_name = "network-wireless-signal-ok-symbolic"
        elif sig >= 20:
            icon_name = "network-wireless-signal-weak-symbolic"

        icon = Gtk.Image.new_from_icon_name(icon_name)
        if net["active"]:
            icon.add_css_class("accent-icon")

        box.append(icon)

        # SSID Label
        label = Gtk.Label(label=net["ssid"])
        label.set_hexpand(True)
        label.set_halign(Gtk.Align.START)
        if net["active"]:
            label.add_css_class("bold-label")
        box.append(label)

        if net["security"] not in ("", "--"):
            lock = Gtk.Image.new_from_icon_name("network-wireless-encrypted-symbolic")
            lock.set_tooltip_text(net["security"])
            box.append(lock)

        # Connected Checkmark
        if net["active"]:
            check = Gtk.Image.new_from_icon_name("object-select-symbolic")
            check.add_css_class("accent-icon")
            box.append(check)

        row.set_child(box)
        return row

    def on_network_row_activated(self, listbox, row):
        if self.connecting or row.active:
            return

        self.selected_network = row.network
        self.jobs.submit(
            "network-profiles",
            self.backend.saved_profiles,
            on_success=lambda profiles: self._choose_connection(row.network, profiles),
            on_error=lambda error: self.show_toast(f"Saved connections unavailable: {error}"),
        )

    def _choose_connection(self, network, profiles):
        exact = next(
            (
                profile
                for profile in profiles
                if profile["ssid"] == network["ssid"]
                and profile["bssid"]
                and profile["bssid"].casefold() == network["bssid"].casefold()
            ),
            None,
        )
        fallback = next(
            (
                profile
                for profile in profiles
                if profile["ssid"] == network["ssid"] and not profile["bssid"]
            ),
            None,
        )
        profile = exact or fallback
        if profile is not None:
            self.start_connection(network, profile_uuid=profile["uuid"])
            return

        security = network["security"].upper()
        if security in ("", "--"):
            self.start_connection(network)
        elif "802.1X" in security or "EAP" in security:
            self.show_toast("Enterprise Wi-Fi requires NetworkManager's full editor")
            self.hide()
            try:
                subprocess.Popen(["kitty", "--class", "nmtui", "-e", "nmtui-connect"])
            except OSError as error:
                self.show_toast(f"Unable to open nmtui: {error}")
        else:
            self.prompt_for_password(network)

    def prompt_for_password(self, network):
        escaped_ssid = GLib.markup_escape_text(network["ssid"])
        self.password_title.set_markup(f"Connect to <b>{escaped_ssid}</b>")
        self.password_entry.set_text("")
        self.stack.set_visible_child_name("password")
        self.password_entry.grab_focus()

    def cancel_password_prompt(self):
        self.stack.set_visible_child_name("list")
        self.selected_network = None

    def on_connect_with_password_clicked(self):
        password = self.password_entry.get_text()
        network = self.selected_network
        if not network:
            return
        if not password:
            self.show_toast("Enter the network password")
            return
        self.start_connection(network, password=password)

    def start_connection(self, network, password=None, profile_uuid=None):
        ssid = network["ssid"]
        self.connecting = True
        self.loading_status.set_description(f"Connecting to {ssid}...")
        self.stack.set_visible_child_name("loading")

        if profile_uuid:
            operation = self.backend.connect_saved
            args = (profile_uuid,)
        elif password is not None:
            operation = self.backend.connect_personal
            args = (ssid, network["bssid"], password)
        else:
            operation = self.backend.connect_open
            args = (ssid, network["bssid"])
        self.jobs.submit(
            "network-connect",
            operation,
            *args,
            on_success=lambda output: self.on_connection_complete(True, ssid, output),
            on_error=lambda error: self.on_connection_complete(False, ssid, str(error)),
        )

    def on_connection_complete(self, success, ssid, output):
        self.connecting = False
        if success:
            self.show_toast(f"Successfully connected to {ssid}")
            self.hide()
        else:
            self.show_toast(f"Connection failed: {output}")
            self.stack.set_visible_child_name("list")
            self.refresh_networks()

    def load_css(self):
        css = b"""
        window { background: alpha(@window_bg_color, 0.95); }
        .card-box {
            background: alpha(@view_fg_color, 0.04);
            border: 1px solid alpha(@view_fg_color, 0.08);
            border-radius: 12px;
            padding: 12px 16px;
        }
        .network-list {
            background: alpha(@view_fg_color, 0.02);
            border: 1px solid alpha(@view_fg_color, 0.08);
            border-radius: 12px;
            padding: 4px;
        }
        .network-row {
            padding: 12px 16px;
            border-radius: 8px;
            transition: all 150ms;
        }
        .network-list row:hover {
            background: alpha(@accent_bg_color, 0.1);
        }
        .accent-icon {
            color: @accent_color;
        }
        .bold-label {
            font-weight: bold;
            color: @accent_color;
        }
        .password-card {
            background: alpha(@window_bg_color, 0.5);
            border: 1px solid alpha(@view_fg_color, 0.1);
            border-radius: 16px;
            padding: 32px;
            min-width: 320px;
        }
        .password-title {
            font-size: 1.1em;
            margin-bottom: 8px;
        }
        """
        provider = Gtk.CssProvider()
        provider.load_from_data(css)
        Gtk.StyleContext.add_provider_for_display(
            Gdk.Display.get_default(), provider, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION
        )
