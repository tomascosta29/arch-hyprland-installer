#!/usr/bin/env python3
import os
import subprocess
import gi
import json
import time
import shutil
import threading
import re

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
gi.require_version("Gdk", "4.0")
from gi.repository import Adw, GLib, Gdk, Gtk, Gio, GdkPixbuf

try:
    from .dispatch import dispatch_to_main
except ImportError:
    from dispatch import dispatch_to_main

SCREENSHOTS_DIR = os.path.expanduser("~/Pictures/Screenshots")

REQUIRED_DEPS = {
    "grim": "grim",
    "slurp": "slurp",
    "hyprctl": "hyprctl",
    "wl-copy": "wl-copy",
}

def check_dependencies():
    missing = []
    for cmd, name in REQUIRED_DEPS.items():
        if shutil.which(cmd) is None:
            missing.append(name)
    return missing

def load_config():
    config_file = os.path.expanduser("~/.config/blinker/settings.json")
    default_config = {
        "screenshot_dir": "~/Pictures/Screenshots",
        "naming_pattern": "Screenshot_%Y%m%d_%H%M%S",
        "copy_to_clipboard": True,
        "show_notification": True,
        "open_manager_after_capture": True,
    }
    try:
        with open(config_file, "r") as f:
            return {**default_config, **json.load(f)}
    except:
        return default_config

def get_recent_screenshots(count=4):
    config = load_config()
    screenshot_dir = os.path.expanduser(config.get("screenshot_dir", SCREENSHOTS_DIR))
    if not os.path.exists(screenshot_dir):
        return []
    files = [os.path.join(screenshot_dir, f) for f in os.listdir(screenshot_dir) 
             if f.lower().endswith((".png", ".jpg", ".jpeg", ".webp"))]
    files.sort(key=lambda x: os.path.getmtime(x), reverse=True)
    return files[:count]

class BlinkerLauncher(Adw.ApplicationWindow):
    def __init__(self, app):
        super().__init__(application=app, title="Blinker")
        self.set_default_size(420, 240)
        self.set_resizable(False)
        self.set_decorated(True)
        self.set_deletable(True)
        self.connect("close-request", self.on_close_request)
        self.connect("notify::is-active", self.on_is_active_changed)
        
        self.capturing = False
        self.selected_index = 0
        self.capture_rows = []
        
        self.build_ui()
        self.load_css()
        self.update_selection()

    def build_ui(self):
        self.toast_overlay = Adw.ToastOverlay()
        self.set_content(self.toast_overlay)

        view = Adw.ToolbarView()
        self.toast_overlay.set_child(view)

        header = Adw.HeaderBar()
        title = Gtk.Label(label="Blinker")
        title.set_markup("<b>Blinker</b>")
        header.set_title_widget(title)
        view.add_top_bar(header)

        settings_btn = Gtk.Button(icon_name="settings-symbolic")
        settings_btn.set_tooltip_text("Settings (Ctrl+,)")
        settings_btn.connect("clicked", lambda _: self.open_manager())
        header.pack_end(settings_btn)

        main_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
        main_box.set_valign(Gtk.Align.CENTER)
        main_box.set_margin_start(12)
        main_box.set_margin_end(12)
        main_box.set_margin_top(8)
        main_box.set_margin_bottom(12)
        view.set_content(main_box)

        capture_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4)
        capture_box.set_halign(Gtk.Align.FILL)
        main_box.append(capture_box)

        self.capture_options = [
            ("full", "Full Screen", "screen.svg", "F1", "Capture entire screen"),
            ("area", "Select Area", "area.svg", "F2", "Draw to select region"),
            ("window", "Active Window", "window.svg", "F3", "Capture focused window"),
        ]

        base_dir = os.path.dirname(os.path.realpath(__file__))
        icons_dir = os.path.join(base_dir, "../icons")

        self.capture_rows = []
        
        for mode, label, icon_file, shortcut, subtitle in self.capture_options:
            row = Adw.ActionRow(title=label, subtitle=subtitle)
            row.add_css_class("capture-row")
            row._capture_mode = mode
            
            # Shortcut Badge (Left)
            shortcut_label = Gtk.Label(label=shortcut)
            shortcut_label.add_css_class("shortcut-badge")
            shortcut_label.set_halign(Gtk.Align.CENTER)
            shortcut_label.set_valign(Gtk.Align.CENTER)
            
            shortcut_box = Gtk.Box()
            shortcut_box.set_size_request(34, 34)
            shortcut_box.set_valign(Gtk.Align.CENTER)
            shortcut_box.set_halign(Gtk.Align.CENTER)
            shortcut_box.set_margin_end(8)
            shortcut_box.append(shortcut_label)
            row.add_prefix(shortcut_box)
            
            # Icon Button (Right)
            icon_path = os.path.join(icons_dir, icon_file)
            img = Gtk.Image.new_from_file(icon_path)
            img.set_pixel_size(24)
            
            btn = Gtk.Button()
            btn.set_size_request(40, 40)
            btn.set_valign(Gtk.Align.CENTER)
            btn.set_halign(Gtk.Align.CENTER)
            btn.set_child(img)
            btn.add_css_class("flat")
            btn.add_css_class("capture-icon-btn")
            btn.connect("clicked", lambda _, m=mode: self.take_screenshot(m))
            row.add_suffix(btn)
            
            capture_box.append(row)
            self.capture_rows.append(row)

        separator = Gtk.Separator()
        separator.set_margin_top(8)
        separator.set_margin_bottom(8)
        main_box.append(separator)

        recent_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
        recent_box.set_halign(Gtk.Align.CENTER)
        
        recent_label = Gtk.Label(label="Recent:")
        recent_label.add_css_class("dim-label")
        recent_label.set_margin_end(8)
        recent_box.append(recent_label)
        
        self.recent_thumbs = []
        recent = get_recent_screenshots(4)
        for path in recent:
            thumb = self.make_thumbnail(path)
            if thumb:
                thumb.add_css_class("recent-thumbnail")
                btn = Gtk.Button(valign=Gtk.Align.CENTER)
                btn.set_size_request(44, 44)
                btn.set_child(thumb)
                btn.add_css_class("flat")
                btn.add_css_class("recent-thumbnail-btn")
                btn.connect("clicked", lambda _, p=path: self.copy_image(p))
                self.recent_thumbs.append(btn)
                recent_box.append(btn)
        
        if recent:
            main_box.append(recent_box)

        ctrl = Gtk.EventControllerKey()
        ctrl.connect("key-pressed", self.on_key_pressed)
        self.add_controller(ctrl)

    def load_css(self):
        css = b"""
        window { background: alpha(@window_bg_color, 0.95); }
        .shortcut-badge {
            font-size: 0.8em;
            font-weight: bold;
            background: alpha(@view_fg_color, 0.1);
            color: @view_fg_color;
            padding: 2px 6px;
            border-radius: 6px;
        }
        .capture-row {
            border-radius: 10px;
            margin: 4px 0;
            transition: all 150ms ease-in-out;
        }
        .capture-row.selected {
            background: alpha(@accent_bg_color, 0.15);
        }
        .capture-icon-btn {
            border-radius: 50%;
            transition: all 200ms;
        }
        .capture-icon-btn:hover {
            background: alpha(@accent_bg_color, 0.2);
        }
        .recent-thumbnail-btn {
            padding: 0;
            border-radius: 8px;
            overflow: hidden;
            transition: all 200ms;
        }
        .recent-thumbnail-btn:hover {
            transform: scale(1.1);
            box-shadow: 0 4px 12px rgba(0,0,0,0.15);
        }
        .recent-thumbnail {
            border-radius: 8px;
        }
        .dim-label {
            opacity: 0.6;
            font-size: 0.9em;
        }
        """
        provider = Gtk.CssProvider()
        provider.load_from_data(css)
        Gtk.StyleContext.add_provider_for_display(Gdk.Display.get_default(), provider, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION)

    def update_selection(self):
        for i, row in enumerate(self.capture_rows):
            if i == self.selected_index:
                row.add_css_class("selected")
            else:
                row.remove_css_class("selected")

    def show_toast(self, message):
        toast = Adw.Toast.new(message)
        self.toast_overlay.add_toast(toast)

    def make_thumbnail(self, path):
        if not os.path.exists(path):
            return None
        try:
            pix = GdkPixbuf.Pixbuf.new_from_file_at_scale(path, 44, 44, False)
            texture = Gdk.Texture.new_for_pixbuf(pix)
            img = Gtk.Image.new_from_paintable(texture)
            return img
        except:
            img = Gtk.Image.new_from_icon_name("image-missing-symbolic")
            img.set_pixel_size(44)
            return img

    def copy_image(self, path):
        if os.path.exists(path):
            mime = "image/png" if path.lower().endswith(".png") else "image/jpeg"
            subprocess.run(["wl-copy", "-t", mime], stdin=open(path, "rb"), check=False)
            self.show_toast("Copied to clipboard")

    def open_manager(self):
        subprocess.Popen(["costa-utils", "--blinker-manager"])
        self.hide()

    def take_screenshot(self, mode):
        if self.capturing:
            return
        self.capturing = True
        
        # Hide the launcher window to avoid capturing it
        self.hide()
        
        def worker():
            # Wait for hide animation to complete
            time.sleep(0.25)
            
            config = load_config()
            dir_path = os.path.expanduser(config.get("screenshot_dir", SCREENSHOTS_DIR))
            os.makedirs(dir_path, exist_ok=True)
            
            naming_pattern = config.get("naming_pattern", "Screenshot_%Y%m%d_%H%M%S")
            timestamp = time.strftime(naming_pattern)
            filename = os.path.join(dir_path, f"{timestamp}.png")
            
            cmd = []
            if mode == "full":
                cmd = ["grim", filename]
            elif mode == "area":
                try:
                    # Run slurp to get geometry (with colors matching theme)
                    slurp_proc = subprocess.run(
                        ["slurp", "-b", "21293699", "-c", "7FB0DEff", "-s", "7FB0DE0D", "-w", "2"],
                        capture_output=True, text=True
                    )
                    if slurp_proc.returncode == 0:
                        geometry = slurp_proc.stdout.strip()
                        cmd = ["grim", "-g", geometry, filename]
                    else:
                        GLib.idle_add(self.on_capture_done, False, "Selection cancelled")
                        return
                except Exception as e:
                    GLib.idle_add(self.on_capture_done, False, f"Slurp failed: {e}")
                    return
            elif mode == "window":
                try:
                    hypr_out = subprocess.check_output(["hyprctl", "activewindow"], text=True)
                    at_match = re.search(r"at:\s*(\d+),\s*(\d+)", hypr_out)
                    size_match = re.search(r"size:\s*(\d+),\s*(\d+)", hypr_out)
                    if at_match and size_match:
                        geometry = f"{at_match.group(1)},{at_match.group(2)} {size_match.group(1)}x{size_match.group(2)}"
                        cmd = ["grim", "-g", geometry, filename]
                    else:
                        cmd = ["grim", filename]
                except Exception:
                    cmd = ["grim", filename]
            
            # Execute grim
            try:
                res = subprocess.run(cmd)
                if res.returncode == 0 and os.path.exists(filename):
                    # Copy to clipboard
                    if config.get("copy_to_clipboard", True):
                        subprocess.run(["wl-copy", "-t", "image/png"], stdin=open(filename, "rb"))
                    
                    # Play sound
                    subprocess.run(["paplay", "/usr/share/sounds/freedesktop/stereo/screen-capture.oga"], stderr=subprocess.DEVNULL, stdout=subprocess.DEVNULL)
                    
                    # Show notification
                    if config.get("show_notification", True):
                        subprocess.run(["notify-send", "-i", filename, "Screenshot Saved", f"Saved to {os.path.basename(filename)}"])
                    
                    # Open manager
                    if config.get("open_manager_after_capture", True):
                        subprocess.Popen(["costa-utils", "--blinker-manager"])
                    
                    GLib.idle_add(self.on_capture_done, True, "Screenshot captured")
                else:
                    GLib.idle_add(self.on_capture_done, False, "Grim failed to capture")
            except Exception as e:
                GLib.idle_add(self.on_capture_done, False, f"Capture error: {e}")
                
        threading.Thread(target=worker, daemon=True).start()

    def on_capture_done(self, success, message):
        self.capturing = False
        if not success:
            self.present()
            self.show_toast(message)
        else:
            self.get_application().quit()

    def on_key_pressed(self, _, keyval, keycode, state):
        if self.capturing:
            return False
        
        keyname = Gdk.keyval_name(keyval)
        
        if keyname == "F1":
            self.take_screenshot("full")
            return True
        elif keyname == "F2":
            self.take_screenshot("area")
            return True
        elif keyname == "F3":
            self.take_screenshot("window")
            return True
        elif keyname == "Up":
            self.selected_index = (self.selected_index - 1) % len(self.capture_rows)
            self.update_selection()
            return True
        elif keyname == "Down":
            self.selected_index = (self.selected_index + 1) % len(self.capture_rows)
            self.update_selection()
            return True
        elif keyname in ("Return", "KP_Enter"):
            mode = self.capture_rows[self.selected_index]._capture_mode
            self.take_screenshot(mode)
            return True
        elif keyname == "Escape":
            self.hide()
            return True
            
        return False

    def on_close_request(self, win):
        self.hide()
        return True

    def on_is_active_changed(self, window, pspec):
        if not self.is_active():
            self.hide()

class BlinkerApp(Adw.Application):
    def __init__(self):
        super().__init__(application_id="org.fcosta.Blinker")
        self.hold()
        Adw.StyleManager.get_default().set_color_scheme(Adw.ColorScheme.PREFER_DARK)
    
    def do_activate(self):
        win = self.props.active_window or BlinkerLauncher(self)
        if not win.get_application():
             win.set_application(self)
        win.present()

def run():
    if dispatch_to_main("--blinker"):
        return

    BlinkerApp().run([])

if __name__ == "__main__":
    run()
