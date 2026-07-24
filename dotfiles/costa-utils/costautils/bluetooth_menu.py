#!/usr/bin/env python3
import threading
import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Adw, GLib, Gtk, Gdk, Gio

class BluetoothWindow(Adw.ApplicationWindow):
    def __init__(self, app):
        super().__init__(application=app, title="Bluetooth Manager")
        self.set_default_size(480, 450)
        self.set_resizable(False)
        self.set_modal(True)
        
        self.devices = []
        self.connecting = False
        self.adapter_powered = True
        self.adapter_path = "/org/bluez/hci0"  # Default fallback
        self.bus = Gio.bus_get_sync(Gio.BusType.SYSTEM, None)
        
        self.build_ui()
        self.load_css()
        
        self.refresh_devices()
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
        title = Gtk.Label(label="Bluetooth")
        title.set_markup("<b>Bluetooth</b>")
        header.set_title_widget(title)
        view.add_top_bar(header)

        # Refresh button
        self.refresh_btn = Gtk.Button(icon_name="view-refresh-symbolic")
        self.refresh_btn.set_tooltip_text("Refresh Bluetooth devices")
        self.refresh_btn.connect("clicked", lambda _: self.refresh_devices())
        header.pack_end(self.refresh_btn)

        # Main box
        main_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=12)
        main_box.set_margin_start(16); main_box.set_margin_end(16)
        main_box.set_margin_top(16); main_box.set_margin_bottom(16)
        view.set_content(main_box)

        # Power Switch
        power_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL)
        power_box.add_css_class("card-box")
        
        power_icon = Gtk.Image.new_from_icon_name("bluetooth-active-symbolic")
        power_label = Gtk.Label(label="Bluetooth Enable")
        power_label.set_hexpand(True)
        power_label.set_halign(Gtk.Align.START)
        
        self.power_switch = Gtk.Switch()
        self.power_switch.connect("state-set", self.on_power_switch_toggled)
        
        power_box.append(power_icon)
        power_box.append(power_label)
        power_box.append(self.power_switch)
        main_box.append(power_box)

        # Stack
        self.stack = Gtk.Stack()
        self.stack.set_transition_type(Gtk.StackTransitionType.CROSSFADE)
        self.stack.set_vexpand(True)
        main_box.append(self.stack)

        # Device List
        self.listbox = Gtk.ListBox()
        self.listbox.add_css_class("device-list")
        self.listbox.set_selection_mode(Gtk.SelectionMode.NONE)
        self.listbox.connect("row-activated", self.on_device_row_activated)

        scrolled = Gtk.ScrolledWindow()
        scrolled.set_child(self.listbox)
        scrolled.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
        self.stack.add_named(scrolled, "list")

        # Bluetooth disabled page
        self.disabled_status = Adw.StatusPage()
        self.disabled_status.set_title("Bluetooth is Off")
        self.disabled_status.set_description("Enable Bluetooth to scan for devices")
        self.disabled_status.set_icon_name("bluetooth-disabled-symbolic")
        self.stack.add_named(self.disabled_status, "disabled")

        # Loading / Connecting page
        self.loading_status = Adw.StatusPage()
        self.loading_status.set_title("Connecting...")
        self.loading_status.set_icon_name("bluetooth-active-symbolic")
        self.stack.add_named(self.loading_status, "loading")

        self.stack.set_visible_child_name("list")

    def show_toast(self, text):
        toast = Adw.Toast.new(text)
        self.toast_overlay.add_toast(toast)

    def query_dbus_devices(self):
        try:
            # Call ObjectManager to get managed objects
            reply = self.bus.call_sync(
                "org.bluez",
                "/",
                "org.freedesktop.DBus.ObjectManager",
                "GetManagedObjects",
                None,
                None,
                Gio.DBusCallFlags.NONE,
                -1,
                None
            )
            objects = reply.unpack()[0]
            
            devices = []
            powered = True
            
            for path, interfaces in objects.items():
                if "org.bluez.Adapter1" in interfaces:
                    self.adapter_path = path
                    powered = interfaces["org.bluez.Adapter1"].get("Powered", True)
                    
                if "org.bluez.Device1" in interfaces:
                    dev_props = interfaces["org.bluez.Device1"]
                    name = dev_props.get("Alias", dev_props.get("Name", "Unknown Device"))
                    address = dev_props.get("Address", "")
                    connected = dev_props.get("Connected", False)
                    paired = dev_props.get("Paired", False)
                    icon = dev_props.get("Icon", "bluetooth-active-symbolic")
                    
                    devices.append({
                        "path": path,
                        "name": name,
                        "address": address,
                        "connected": connected,
                        "paired": paired,
                        "icon": icon
                    })
                    
            # Sort: Connected first, then Paired, then by Name
            devices.sort(key=lambda d: (not d["connected"], not d["paired"], d["name"].lower()))
            return powered, devices
        except Exception as e:
            print(f"D-Bus Bluez Query Error: {e}")
            return False, []

    def on_power_switch_toggled(self, switch, state):
        def worker():
            try:
                self.bus.call_sync(
                    "org.bluez",
                    self.adapter_path,
                    "org.freedesktop.DBus.Properties",
                    "Set",
                    GLib.Variant("(ssv)", ("org.bluez.Adapter1", "Powered", GLib.Variant("b", state))),
                    None,
                    Gio.DBusCallFlags.NONE,
                    -1,
                    None
                )
            except Exception as e:
                print(f"Error toggling Bluetooth power: {e}")
            GLib.idle_add(self.refresh_devices)
        threading.Thread(target=worker, daemon=True).start()
        return True

    def refresh_devices(self):
        self.refresh_btn.set_sensitive(False)
        
        def worker():
            # Trigger discoverable/scanning via bluetoothctl in background briefly if powered
            powered, devices = self.query_dbus_devices()
            
            if powered:
                # Run a short discover command in background
                subprocess.run(["bluetoothctl", "discoverable", "on"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            
            GLib.idle_add(self.update_list_ui, powered, devices)

        threading.Thread(target=worker, daemon=True).start()

    def update_list_ui(self, powered, devices):
        self.adapter_powered = powered
        self.devices = devices
        
        self.power_switch.set_active(powered)
        self.listbox.remove_all()
        
        if not powered:
            self.stack.set_visible_child_name("disabled")
            self.refresh_btn.set_sensitive(True)
            return

        for dev in self.devices:
            # We filter: only show devices that are paired, or if they are currently scanning
            # To keep the menu clean, we list paired/connected devices and recently discovered ones
            self.listbox.append(self.make_device_row(dev))
            
        self.refresh_btn.set_sensitive(True)
        self.stack.set_visible_child_name("list")

    def make_device_row(self, dev):
        row = Gtk.ListBoxRow()
        row.dev_path = dev["path"]
        row.dev_name = dev["name"]
        row.connected = dev["connected"]
        
        box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=12)
        box.add_css_class("device-row")
        
        # Device Icon based on class/icon properties
        icon_name = "bluetooth-active-symbolic"
        dev_icon = dev["icon"]
        if dev_icon == "audio-card" or dev_icon == "audio-headphones" or dev_icon == "audio-headset":
            icon_name = "audio-headphones-symbolic"
        elif dev_icon == "input-keyboard":
            icon_name = "input-keyboard-symbolic"
        elif dev_icon == "input-mouse" or dev_icon == "input-gaming":
            icon_name = "input-mouse-symbolic"
        
        icon = Gtk.Image.new_from_icon_name(icon_name)
        if dev["connected"]:
            icon.add_css_class("accent-icon")
        box.append(icon)
        
        # Name
        label = Gtk.Label(label=dev["name"])
        label.set_hexpand(True)
        label.set_halign(Gtk.Align.START)
        if dev["connected"]:
            label.add_css_class("bold-label")
        box.append(label)
        
        # State Indicators
        if dev["connected"]:
            status = Gtk.Label(label="Connected")
            status.add_css_class("status-connected")
            box.append(status)
        elif dev["paired"]:
            status = Gtk.Label(label="Paired")
            status.add_css_class("dim-label")
            box.append(status)
            
        row.set_child(box)
        return row

    def on_device_row_activated(self, listbox, row):
        if self.connecting: return
        
        path = row.dev_path
        name = row.dev_name
        connected = row.connected
        
        self.connecting = True
        action_title = "Disconnecting..." if connected else "Connecting..."
        self.loading_status.set_title(action_title)
        self.loading_status.set_description(f"{action_title[:-3]} {name}")
        self.stack.set_visible_child_name("loading")
        
        def connection_worker():
            method = "Disconnect" if connected else "Connect"
            try:
                self.bus.call_sync(
                    "org.bluez",
                    path,
                    "org.bluez.Device1",
                    method,
                    None,
                    None,
                    Gio.DBusCallFlags.NONE,
                    -1,
                    None
                )
                success = True
                msg = f"Connected to {name}" if not connected else f"Disconnected from {name}"
            except Exception as e:
                success = False
                msg = str(e).split(":")[-1].strip()
                
            GLib.idle_add(self.on_connection_complete, success, msg)
            
        threading.Thread(target=connection_worker, daemon=True).start()

    def on_connection_complete(self, success, message):
        self.connecting = False
        self.show_toast(message)
        self.stack.set_visible_child_name("list")
        self.refresh_devices()
        if success and "Connected" in message:
            # Close menu upon successful connection
            self.hide()

    def load_css(self):
        css = b"""
        window { background: alpha(@window_bg_color, 0.95); }
        .card-box {
            background: alpha(@view_fg_color, 0.04);
            border: 1px solid alpha(@view_fg_color, 0.08);
            border-radius: 12px;
            padding: 12px 16px;
        }
        .device-list {
            background: alpha(@view_fg_color, 0.02);
            border: 1px solid alpha(@view_fg_color, 0.08);
            border-radius: 12px;
            padding: 4px;
        }
        .device-row {
            padding: 12px 16px;
            border-radius: 8px;
            transition: all 150ms;
        }
        .device-list row:hover {
            background: alpha(@accent_bg_color, 0.1);
        }
        .accent-icon {
            color: @accent_color;
        }
        .bold-label {
            font-weight: bold;
            color: @accent_color;
        }
        .status-connected {
            background: alpha(@accent_bg_color, 0.15);
            color: @accent_color;
            padding: 2px 8px;
            border-radius: 6px;
            font-size: 0.8em;
            font-weight: bold;
        }
        .dim-label {
            font-size: 0.8em;
            opacity: 0.6;
        }
        """
        provider = Gtk.CssProvider()
        provider.load_from_data(css)
        Gtk.StyleContext.add_provider_for_display(Gdk.Display.get_default(), provider, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION)
