#!/usr/bin/env python3
import os
import subprocess
import gi
import sys

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
gi.require_version("Gdk", "4.0")
from gi.repository import Adw, GLib, Gdk, Gtk, Gio

try:
    from .dispatch import dispatch_to_main
except ImportError:
    from dispatch import dispatch_to_main

class PowerMenuApp(Adw.Application):
    def __init__(self):
        super().__init__(application_id="org.fcosta.Power", flags=Gio.ApplicationFlags.FLAGS_NONE)
        self.hold()

    def do_activate(self):
        win = self.props.active_window or PowerWindow(self)
        win.present()

class PowerWindow(Adw.ApplicationWindow):
    def __init__(self, app):
        super().__init__(application=app, title="Power Menu")
        self.set_default_size(480, 420)
        self.set_resizable(False)
        self.set_modal(True)
        
        # Actions definition
        self.actions = [
            {"id": "lock", "label": "Lock", "icon": "system-lock-screen-symbolic", "cmd": self.cmd_lock},
            {"id": "suspend", "label": "Suspend", "icon": "system-suspend-symbolic", "cmd": ["systemctl", "suspend"]},
            {"id": "logout", "label": "Log Out", "icon": "system-log-out-symbolic", "cmd": ["loginctl", "terminate-user", os.environ.get("USER", "")]},
            {"id": "hibernate", "label": "Hibernate", "icon": "system-hibernate-symbolic", "cmd": ["systemctl", "hibernate"]},
            {"id": "reboot", "label": "Reboot", "icon": "system-reboot-symbolic", "cmd": ["systemctl", "reboot"]},
            {"id": "shutdown", "label": "Shutdown", "icon": "application-exit-symbolic", "cmd": ["systemctl", "poweroff"]},
        ]

        self.build_ui()
        self.setup_keyboard()
        self.load_css()
        self.connect("close-request", self.on_close_request)
        self.connect("notify::is-active", self.on_is_active_changed)

    def on_close_request(self, win):
        self.hide()
        return True

    def on_is_active_changed(self, window, pspec):
        if not self.is_active():
            self.hide()

    def build_ui(self):
        # Main container
        main_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=24)
        main_box.set_halign(Gtk.Align.CENTER)
        main_box.set_valign(Gtk.Align.CENTER)
        main_box.set_margin_top(32)
        main_box.set_margin_bottom(32)
        main_box.set_margin_start(32)
        main_box.set_margin_end(32)
        self.set_content(main_box)

        # Title
        title_label = Gtk.Label(label="Power Menu")
        title_label.add_css_class("title-label")
        main_box.append(title_label)

        # Grid of buttons
        self.flowbox = Gtk.FlowBox()
        self.flowbox.set_valign(Gtk.Align.CENTER)
        self.flowbox.set_halign(Gtk.Align.CENTER)
        self.flowbox.set_selection_mode(Gtk.SelectionMode.NONE)
        self.flowbox.set_max_children_per_line(3)
        self.flowbox.set_min_children_per_line(3)
        self.flowbox.set_column_spacing(20)
        self.flowbox.set_row_spacing(20)
        self.flowbox.set_activate_on_single_click(True)
        self.flowbox.connect("child-activated", self.on_item_activated)
        
        for action in self.actions:
            self.flowbox.append(self.make_button(action))
            
        main_box.append(self.flowbox)

        # Cancel hint
        hint = Gtk.Label(label="Press Esc to cancel")
        hint.add_css_class("dim-label")
        main_box.append(hint)

    def make_button(self, action):
        btn = Gtk.Button()
        btn.add_css_class("power-btn")
        btn.action_cmd = action["cmd"]
        
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=12)
        box.set_halign(Gtk.Align.CENTER)
        box.set_valign(Gtk.Align.CENTER)
        
        icon = Gtk.Image.new_from_icon_name(action["icon"])
        icon.set_pixel_size(48)
        
        label = Gtk.Label(label=action["label"])
        label.add_css_class("btn-label")
        
        box.append(icon)
        box.append(label)
        btn.set_child(box)
        return btn

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

    def on_item_activated(self, flowbox, child):
        btn = child.get_child()
        cmd = btn.action_cmd
        
        self.hide()
        
        if callable(cmd):
            cmd()
        else:
            try:
                subprocess.Popen(cmd)
            except Exception as e:
                print(f"Error executing command: {e}")

    def cmd_lock(self):
        # Try to detect lock command
        for cmd in ["hyprlock", "swaylock", "gtklock"]:
            if subprocess.run(["which", cmd], capture_output=True).returncode == 0:
                subprocess.Popen([cmd])
                return
        subprocess.Popen(["loginctl", "lock-session"])

    def load_css(self):
        css = b"""
        window { background: alpha(@window_bg_color, 0.95); }
        .title-label { font-size: 1.8em; font-weight: 800; margin-bottom: 12px; opacity: 0.8; }
        .power-btn { 
            padding: 16px; 
            border-radius: 16px; 
            min-width: 120px; 
            min-height: 120px;
            background: alpha(@view_fg_color, 0.05);
            border: 1px solid alpha(@view_fg_color, 0.08);
            transition: all 200ms cubic-bezier(0.25, 0.46, 0.45, 0.94);
        }
        .power-btn:hover { 
            background: alpha(@accent_bg_color, 0.15); 
            border-color: @accent_bg_color;
            transform: scale(1.05);
        }
        .power-btn:active {
            background: @accent_bg_color;
            color: @accent_fg_color;
            transform: scale(0.95);
        }
        .btn-label { font-size: 1.1em; font-weight: bold; }
        .dim-label { opacity: 0.5; font-size: 0.9em; margin-top: 12px; }
        """
        provider = Gtk.CssProvider()
        provider.load_from_data(css)
        Gtk.StyleContext.add_provider_for_display(Gdk.Display.get_default(), provider, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION)

def run():
    if dispatch_to_main("--power-menu"):
        return

    app = PowerMenuApp()
    app.run([])

if __name__ == "__main__":
    run()
