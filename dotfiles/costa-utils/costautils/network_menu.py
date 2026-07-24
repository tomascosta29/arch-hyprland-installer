#!/usr/bin/env python3
import os
import subprocess
import threading
import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Adw, GLib, Gtk, Gdk, Gio

class NetworkWindow(Adw.ApplicationWindow):
    def __init__(self, app):
        super().__init__(application=app, title="Network Manager")
        self.set_default_size(480, 450)
        self.set_resizable(False)
        self.set_modal(True)
        
        self.networks = []
        self.connecting = False
        self.wifi_enabled = True
        
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
            self.hide()
            return True
        return False

    def on_close_request(self, win):
        self.hide()
        return True

    def on_is_active_changed(self, window, pspec):
        if not self.is_active() and not self.connecting:
            self.hide()

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
        main_box.set_margin_start(16); main_box.set_margin_end(16)
        main_box.set_margin_top(16); main_box.set_margin_bottom(16)
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

    def is_wifi_powered(self):
        try:
            res = subprocess.run(["nmcli", "radio", "wifi"], capture_output=True, text=True)
            return res.stdout.strip() == "enabled"
        except:
            return True

    def on_wifi_switch_toggled(self, switch, state):
        def worker():
            action = "on" if state else "off"
            subprocess.run(["nmcli", "radio", "wifi", action])
            GLib.idle_add(self.refresh_networks)
        threading.Thread(target=worker, daemon=True).start()
        return True

    def refresh_networks(self):
        self.wifi_enabled = self.is_wifi_powered()
        self.wifi_switch.set_active(self.wifi_enabled)

        if not self.wifi_enabled:
            self.stack.set_visible_child_name("disabled")
            return

        self.refresh_btn.set_sensitive(False)
        
        def worker():
            # Trigger rescan
            subprocess.run(["nmcli", "device", "wifi", "rescan"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            
            proc = subprocess.run(
                ["nmcli", "-t", "-f", "SSID,SIGNAL,ACTIVE,BARS", "device", "wifi", "list"],
                capture_output=True, text=True
            )
            
            networks = {}
            if proc.returncode == 0:
                for line in proc.stdout.splitlines():
                    parts = line.split(":")
                    if len(parts) >= 3:
                        ssid = parts[0]
                        if not ssid: continue
                        try: signal = int(parts[1])
                        except: signal = 0
                        active = parts[2] == "yes"
                        bars = parts[3] if len(parts) > 3 else ""
                        
                        if ssid not in networks or signal > networks[ssid]["signal"]:
                            networks[ssid] = {
                                "ssid": ssid,
                                "signal": signal,
                                "active": active,
                                "bars": bars
                            }
            
            network_list = list(networks.values())
            network_list.sort(key=lambda x: (not x["active"], -x["signal"]))
            
            GLib.idle_add(self.update_list_ui, network_list)

        threading.Thread(target=worker, daemon=True).start()

    def update_list_ui(self, network_list):
        self.networks = network_list
        self.listbox.remove_all()
        
        for net in self.networks:
            self.listbox.append(self.make_network_row(net))
            
        self.refresh_btn.set_sensitive(True)
        self.stack.set_visible_child_name("list")

    def make_network_row(self, net):
        row = Gtk.ListBoxRow()
        row.ssid = net["ssid"]
        row.active = net["active"]
        
        box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=12)
        box.add_css_class("network-row")
        
        # Signal Icon
        icon_name = "network-wireless-signal-none-symbolic"
        sig = net["signal"]
        if sig >= 80: icon_name = "network-wireless-signal-excellent-symbolic"
        elif sig >= 60: icon_name = "network-wireless-signal-good-symbolic"
        elif sig >= 40: icon_name = "network-wireless-signal-ok-symbolic"
        elif sig >= 20: icon_name = "network-wireless-signal-weak-symbolic"
        
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
        
        # Connected Checkmark
        if net["active"]:
            check = Gtk.Image.new_from_icon_name("object-select-symbolic")
            check.add_css_class("accent-icon")
            box.append(check)
            
        row.set_child(box)
        return row

    def on_network_row_activated(self, listbox, row):
        if self.connecting or row.active: return
        
        ssid = row.ssid
        self.selected_ssid = ssid
        
        # Check if connection already exists
        def check_conn_and_connect():
            proc = subprocess.run(["nmcli", "-t", "-f", "NAME", "connection", "show"], capture_output=True, text=True)
            saved_connections = [line.strip() for line in proc.stdout.splitlines()]
            
            if ssid in saved_connections:
                # Connection profile exists, connect directly
                GLib.idle_add(self.start_connection, ssid)
            else:
                # Connection profile doesn't exist, prompt for password
                GLib.idle_add(self.prompt_for_password, ssid)
                
        threading.Thread(target=check_conn_and_connect, daemon=True).start()

    def prompt_for_password(self, ssid):
        self.password_title.set_markup(f"Connect to <b>{ssid}</b>")
        self.password_entry.set_text("")
        self.stack.set_visible_child_name("password")
        self.password_entry.grab_focus()

    def cancel_password_prompt(self):
        self.stack.set_visible_child_name("list")
        self.selected_ssid = None

    def on_connect_with_password_clicked(self):
        password = self.password_entry.get_text().strip()
        ssid = self.selected_ssid
        if not ssid: return
        self.start_connection(ssid, password)

    def start_connection(self, ssid, password=None):
        self.connecting = True
        self.loading_status.set_description(f"Connecting to {ssid}...")
        self.stack.set_visible_child_name("loading")
        
        def connect_worker():
            if password is not None:
                cmd = ["nmcli", "device", "wifi", "connect", ssid, "password", password]
            else:
                cmd = ["nmcli", "connection", "up", "id", ssid]
                
            res = subprocess.run(cmd, capture_output=True, text=True)
            success = res.returncode == 0
            
            GLib.idle_add(self.on_connection_complete, success, ssid, res.stderr.strip() or res.stdout.strip())
            
        threading.Thread(target=connect_worker, daemon=True).start()

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
        Gtk.StyleContext.add_provider_for_display(Gdk.Display.get_default(), provider, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION)
