#!/usr/bin/env python3
import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Adw, Gdk, Gio, GLib, Gtk

AGENT_XML = """
<node>
  <interface name="org.bluez.Agent1">
    <method name="Release"/>
    <method name="RequestPinCode">
      <arg name="device" direction="in" type="o"/>
      <arg name="pincode" direction="out" type="s"/>
    </method>
    <method name="DisplayPinCode">
      <arg name="device" direction="in" type="o"/>
      <arg name="pincode" direction="in" type="s"/>
    </method>
    <method name="RequestPasskey">
      <arg name="device" direction="in" type="o"/>
      <arg name="passkey" direction="out" type="u"/>
    </method>
    <method name="DisplayPasskey">
      <arg name="device" direction="in" type="o"/>
      <arg name="passkey" direction="in" type="u"/>
      <arg name="entered" direction="in" type="q"/>
    </method>
    <method name="RequestConfirmation">
      <arg name="device" direction="in" type="o"/>
      <arg name="passkey" direction="in" type="u"/>
    </method>
    <method name="RequestAuthorization">
      <arg name="device" direction="in" type="o"/>
    </method>
    <method name="AuthorizeService">
      <arg name="device" direction="in" type="o"/>
      <arg name="uuid" direction="in" type="s"/>
    </method>
    <method name="Cancel"/>
  </interface>
</node>
"""


class PairingAgent:
    """BlueZ Agent1 with explicit user confirmation inside the Bluetooth UI."""

    PATH = "/org/fcosta/CostaUtils/BluetoothAgent"

    def __init__(self, window, bus):
        self.window = window
        self.bus = bus
        self.dialog = None
        node = Gio.DBusNodeInfo.new_for_xml(AGENT_XML)
        self.registration_id = bus.register_object(
            self.PATH,
            node.interfaces[0],
            self._on_method_call,
            None,
            None,
        )
        try:
            bus.call_sync(
                "org.bluez",
                "/org/bluez",
                "org.bluez.AgentManager1",
                "RegisterAgent",
                GLib.Variant("(os)", (self.PATH, "KeyboardDisplay")),
                None,
                Gio.DBusCallFlags.NONE,
                5000,
                None,
            )
            bus.call_sync(
                "org.bluez",
                "/org/bluez",
                "org.bluez.AgentManager1",
                "RequestDefaultAgent",
                GLib.Variant("(o)", (self.PATH,)),
                None,
                Gio.DBusCallFlags.NONE,
                5000,
                None,
            )
        except GLib.Error as error:
            self.window.show_toast(f"Bluetooth pairing agent unavailable: {error.message}")

    @staticmethod
    def _complete(invocation, value=None):
        invocation.return_value(value)

    @staticmethod
    def _reject(invocation):
        invocation.return_dbus_error("org.bluez.Error.Rejected", "Pairing rejected")

    def _question(self, title, body, invocation, result=None, entry_type=None):
        dialog = Adw.MessageDialog.new(self.window, title, body)
        dialog.add_response("cancel", "Cancel")
        dialog.add_response("accept", "Confirm")
        dialog.set_response_appearance("accept", Adw.ResponseAppearance.SUGGESTED)
        dialog.set_default_response("accept")
        dialog.set_close_response("cancel")
        entry = None
        if entry_type:
            entry = Gtk.Entry()
            entry.set_input_purpose(
                Gtk.InputPurpose.PIN if entry_type == "pin" else Gtk.InputPurpose.DIGITS
            )
            entry.set_max_length(16 if entry_type == "pin" else 6)
            dialog.set_extra_child(entry)

        def responded(_dialog, response):
            self.dialog = None
            if response != "accept":
                self._reject(invocation)
                return
            if entry_type == "pin":
                value = entry.get_text().strip()
                if not value:
                    self._reject(invocation)
                    return
                self._complete(invocation, GLib.Variant("(s)", (value,)))
            elif entry_type == "passkey":
                value = entry.get_text().strip()
                if not value.isdigit() or int(value) > 999999:
                    self._reject(invocation)
                    return
                self._complete(invocation, GLib.Variant("(u)", (int(value),)))
            else:
                self._complete(invocation, result)

        dialog.connect("response", responded)
        self.dialog = dialog
        dialog.present()

    def _on_method_call(
        self,
        _connection,
        _sender,
        _object_path,
        _interface,
        method,
        parameters,
        invocation,
    ):
        values = parameters.unpack()
        if method == "Release":
            self._complete(invocation)
        elif method == "RequestPinCode":
            self._question("Bluetooth PIN", "Enter the device PIN", invocation, entry_type="pin")
        elif method == "RequestPasskey":
            self._question(
                "Bluetooth passkey",
                "Enter the six-digit device passkey",
                invocation,
                entry_type="passkey",
            )
        elif method == "RequestConfirmation":
            self._question(
                "Confirm Bluetooth pairing",
                f"Does the device show {values[1]:06d}?",
                invocation,
            )
        elif method in ("RequestAuthorization", "AuthorizeService"):
            self._question(
                "Authorize Bluetooth device",
                "Allow this device to connect?",
                invocation,
            )
        elif method == "DisplayPinCode":
            self.window.show_toast(f"Enter PIN {values[1]} on the Bluetooth device")
            self._complete(invocation)
        elif method == "DisplayPasskey":
            self.window.show_toast(f"Enter passkey {values[1]:06d} on the Bluetooth device")
            self._complete(invocation)
        elif method == "Cancel":
            if self.dialog:
                self.dialog.close()
                self.dialog = None
            self._complete(invocation)
        else:
            invocation.return_dbus_error(
                "org.bluez.Error.NotSupported", f"Unsupported agent method: {method}"
            )


class BluetoothWindow(Adw.ApplicationWindow):
    def __init__(self, app):
        super().__init__(application=app, title="Bluetooth Manager")
        self.set_default_size(480, 450)
        self.set_resizable(False)
        self.set_modal(True)

        self.devices = []
        self.connecting = False
        self.adapter_powered = False
        self.adapter_path = None
        self.updating_power = False
        self.discovery_timeout = None
        self.active_device_path = None
        self.jobs = app.jobs
        self.backend = app.bluetooth
        self.bus = self.backend.bus

        self.build_ui()
        self.load_css()
        self.agent = PairingAgent(self, self.bus) if self.bus is not None else None

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
            if self.active_device_path:
                self.cancel_connection()
            self.stop_discovery()
            self.hide()
            return True
        return False

    def on_close_request(self, win):
        self.stop_discovery()
        self.hide()
        return True

    def on_is_active_changed(self, window, pspec):
        if not self.is_active() and not self.connecting:
            self.stop_discovery()
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
        main_box.set_margin_start(16)
        main_box.set_margin_end(16)
        main_box.set_margin_top(16)
        main_box.set_margin_bottom(16)
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
        cancel_pairing = Gtk.Button(label="Cancel")
        cancel_pairing.set_halign(Gtk.Align.CENTER)
        cancel_pairing.connect("clicked", lambda _button: self.cancel_connection())
        self.loading_status.set_child(cancel_pairing)
        self.stack.add_named(self.loading_status, "loading")

        self.stack.set_visible_child_name("list")

    def show_toast(self, text):
        toast = Adw.Toast.new(text)
        self.toast_overlay.add_toast(toast)

    def on_power_switch_toggled(self, switch, state):
        if self.updating_power:
            return False
        if not self.adapter_path:
            self.show_toast("No Bluetooth adapter found")
            return True
        self.jobs.submit(
            "bluetooth-power",
            self.backend.set_power,
            self.adapter_path,
            state,
            on_success=lambda _result: self.refresh_devices(),
            on_error=lambda error: self.show_toast(f"Bluetooth toggle failed: {error}"),
        )
        return True

    def refresh_devices(self, scan=True):
        self.backend.subscribe(self, lambda: self.refresh_devices(scan=False))
        self.refresh_btn.set_sensitive(False)
        self.jobs.submit(
            "bluetooth-query",
            self.backend.query,
            on_success=lambda state: self.update_list_ui(state, scan),
            on_error=self.on_query_error,
        )

    def on_query_error(self, error):
        self.refresh_btn.set_sensitive(True)
        self.show_toast(f"Bluetooth unavailable: {error}")

    def update_list_ui(self, state, scan):
        self.adapter_path = state.adapter_path
        self.adapter_powered = state.powered
        self.devices = list(state.devices)

        self.updating_power = True
        self.power_switch.set_active(state.powered)
        self.updating_power = False
        self.listbox.remove_all()

        if not state.adapter_path:
            self.disabled_status.set_title("No Bluetooth Adapter")
            self.disabled_status.set_description("No BlueZ adapter is available")
            self.stack.set_visible_child_name("disabled")
            self.refresh_btn.set_sensitive(True)
            return
        if not state.powered:
            self.disabled_status.set_title("Bluetooth is Off")
            self.disabled_status.set_description("Enable Bluetooth to scan for devices")
            self.stack.set_visible_child_name("disabled")
            self.refresh_btn.set_sensitive(True)
            return

        for dev in self.devices:
            self.listbox.append(self.make_device_row(dev))

        self.refresh_btn.set_sensitive(True)
        self.stack.set_visible_child_name("list")
        if scan and self.adapter_path:
            self.jobs.submit(
                "bluetooth-discovery-start",
                self.backend.start_discovery,
                self.adapter_path,
                on_success=lambda _result: self._schedule_scan_update(),
                on_error=lambda error: self.show_toast(f"Bluetooth discovery failed: {error}"),
            )

    def _schedule_scan_update(self):
        GLib.timeout_add(1800, self._refresh_after_discovery)
        if self.discovery_timeout is not None:
            GLib.source_remove(self.discovery_timeout)
        self.discovery_timeout = GLib.timeout_add_seconds(12, self._stop_discovery_timeout)

    def _refresh_after_discovery(self):
        self.refresh_devices(scan=False)
        return GLib.SOURCE_REMOVE

    def _stop_discovery_timeout(self):
        self.discovery_timeout = None
        self.stop_discovery()
        return GLib.SOURCE_REMOVE

    def stop_discovery(self):
        if self.discovery_timeout is not None:
            GLib.source_remove(self.discovery_timeout)
            self.discovery_timeout = None
        if self.adapter_path:
            self.jobs.submit(
                "bluetooth-discovery-stop",
                self.backend.stop_discovery,
                self.adapter_path,
                replace=False,
            )
        self.backend.unsubscribe(self)

    def make_device_row(self, dev):
        row = Gtk.ListBoxRow()
        row.dev_path = dev["path"]
        row.dev_name = dev["name"]
        row.connected = dev["connected"]
        row.paired = dev["paired"]

        box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=12)
        box.add_css_class("device-row")

        # Device Icon based on class/icon properties
        icon_name = "bluetooth-active-symbolic"
        dev_icon = dev["icon"]
        if (
            dev_icon == "audio-card"
            or dev_icon == "audio-headphones"
            or dev_icon == "audio-headset"
        ):
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

        if dev["paired"]:
            forget = Gtk.Button(icon_name="edit-delete-symbolic")
            forget.add_css_class("flat")
            forget.set_tooltip_text(f"Forget {dev['name']}")
            forget.connect("clicked", lambda button: self.forget_device(dev))
            box.append(forget)

        row.set_child(box)
        return row

    def on_device_row_activated(self, listbox, row):
        if self.connecting:
            return

        path = row.dev_path
        name = row.dev_name
        connected = row.connected
        paired = row.paired

        self.connecting = True
        self.active_device_path = path
        action_title = "Disconnecting..." if connected else "Connecting..."
        self.loading_status.set_title(action_title)
        self.loading_status.set_description(f"{action_title[:-3]} {name}")
        self.stack.set_visible_child_name("loading")

        operation = self.backend.disconnect if connected else self.backend.connect
        args = (path,) if connected else (path, paired)
        self.jobs.submit(
            "bluetooth-connection",
            operation,
            *args,
            on_success=lambda _result: self.on_connection_complete(
                True,
                f"Disconnected from {name}" if connected else f"Connected to {name}",
            ),
            on_error=lambda error: self.on_connection_complete(False, str(error)),
        )

    def forget_device(self, device):
        if not self.adapter_path:
            return
        self.jobs.submit(
            "bluetooth-forget",
            self.backend.remove,
            self.adapter_path,
            device["path"],
            on_success=lambda _result: (
                self.show_toast(f"Forgot {device['name']}"),
                self.refresh_devices(),
            ),
            on_error=lambda error: self.show_toast(f"Unable to forget device: {error}"),
        )

    def on_connection_complete(self, success, message):
        self.connecting = False
        self.active_device_path = None
        self.show_toast(message)
        self.stack.set_visible_child_name("list")
        self.refresh_devices()
        if success and "Connected" in message:
            # Close menu upon successful connection
            self.hide()

    def cancel_connection(self):
        path = self.active_device_path
        if not path:
            return
        self.jobs.invalidate("bluetooth-connection")
        self.jobs.submit(
            "bluetooth-cancel-pairing",
            self.backend.cancel_pairing,
            path,
            on_success=lambda _result: self._finish_cancel(),
            on_error=lambda _error: self._finish_cancel(),
        )

    def _finish_cancel(self):
        self.connecting = False
        self.active_device_path = None
        self.show_toast("Bluetooth connection cancelled")
        self.refresh_devices()

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
        Gtk.StyleContext.add_provider_for_display(
            Gdk.Display.get_default(), provider, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION
        )
