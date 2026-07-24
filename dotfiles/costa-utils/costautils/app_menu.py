#!/usr/bin/env python3
import json
import math
import os
import re
import subprocess
import threading

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Adw, Gdk, Gio, GLib, Gtk, Pango

try:
    from .dispatch import dispatch_to_main
except ImportError:
    from dispatch import dispatch_to_main

HISTORY_FILE = os.path.join(
    os.environ.get("XDG_STATE_HOME", os.path.expanduser("~/.local/state")),
    "costa-utils",
    "runner_history.json",
)

# Desktop IDs that duplicate Costa Utils roles or are noise on this stack.
HIDDEN_APP_IDS = {
    id_.lower()
    for id_ in {
        "org.fcosta.CostaUtils",
        "avahi-discover",
        "bssh",
        "bvnc",
        "qv4l2",
        "qvidcap",
        "htop",
        "nvim",
        "nvim-qt",
        "cmake-gui",
        "electron",
        "nm-connection-editor",
        "pavucontrol",
        "org.pulseaudio.pavucontrol",
        "blueman-manager",
        "blueman-adapters",
        "chromium",
        "chromium-browser",
        "org.chromium.Chromium",
        "google-chrome",
        "com.google.Chrome",
        "brave-browser",
        "org.brave.Browser",
        "microsoft-edge",
        "opera",
        "vivaldi-stable",
        "librewolf",
        "org.gnome.Software",
        "org.freedesktop.Xwayland",
        "xdvi",
        "lstopo",
        "xgps",
        "xgpsspeed",
        "rofi",
        "rofi-theme-selector",
    }
}

HIDDEN_APP_ID_PREFIXES = (
    "org.gnome.Settings",
    "org.gnome.SystemMonitor",
    "gnome-system-monitor",
    "qv4l2",
)


def normalize_app_id(app_id):
    if not app_id:
        return ""
    return app_id.removesuffix(".desktop").lower()


def should_list_app(app_info):
    """Return True when an application belongs in the Costa launcher."""
    if not app_info.should_show():
        return False

    app_id = normalize_app_id(app_info.get_id())
    if not app_id:
        return False
    if app_id in HIDDEN_APP_IDS:
        return False
    if any(app_id.startswith(prefix.lower()) for prefix in HIDDEN_APP_ID_PREFIXES):
        return False

    # Keep Firefox as the only browser; hide any other WebBrowser entries.
    categories = set()
    if isinstance(app_info, Gio.DesktopAppInfo):
        raw_categories = app_info.get_categories() or ""
        categories = {item.lower() for item in raw_categories.split(";") if item}
    if "webbrowser" in categories and app_id not in {"firefox", "org.mozilla.firefox"}:
        return False

    return True


def load_runner_history():
    try:
        with open(HISTORY_FILE, "r") as f:
            return json.load(f)
    except Exception:
        return []


def save_runner_history(history):
    try:
        os.makedirs(os.path.dirname(HISTORY_FILE), exist_ok=True)
        with open(HISTORY_FILE, "w") as f:
            json.dump(history[:50], f)
    except Exception:
        pass


class AppMenuWindow(Adw.ApplicationWindow):
    def __init__(self, app, runner_mode=False):
        self.runner_mode = runner_mode
        super().__init__(application=app, title="Runner" if runner_mode else "AppMenu")
        self.set_default_size(720, 520)
        self.set_modal(True)
        self.set_resizable(False)

        # Data
        self.apps = []
        self.filtered_apps = []
        self.selected_index = 0

        if self.runner_mode:
            self.history = load_runner_history()
            self.filtered_history = list(self.history)

        self.load_css()
        if not self.runner_mode:
            self.load_apps()
            # Monitor system app changes
            self.app_monitor = Gio.AppInfoMonitor.get()
            self.app_monitor.connect("changed", self.on_apps_changed)
        self.build_ui()
        self.setup_keyboard()

        # Initial filter
        self.showing_output = False
        self.on_search_changed(self.search_entry)

        # Override default close to hide
        self.connect("close-request", self.on_close_request)
        self.connect("notify::is-active", self.on_is_active_changed)

    def on_close_request(self, win):
        self.hide_window()
        return True

    def on_is_active_changed(self, window, pspec):
        GLib.timeout_add(150, self._check_focus_loss)

    def _check_focus_loss(self):
        is_act = self.is_active() if hasattr(self, "is_active") else self.get_property("is-active")
        if self.get_visible() and not is_act:
            self.hide_window()
        return False

    def load_apps(self):
        # Use Gio to get all desktop apps
        all_apps = Gio.AppInfo.get_all()
        for app in all_apps:
            if not should_list_app(app):
                continue

            name = app.get_name()
            icon = app.get_icon()
            desc = app.get_description() or ""
            keywords = app.get_keywords() or []

            # Create a search string for fuzzy matching
            search_text = f"{name} {desc} {' '.join(keywords)}".lower()

            self.apps.append(
                {"app_info": app, "name": name, "icon": icon, "search_text": search_text}
            )

        # Sort alphabetically
        self.apps.sort(key=lambda x: x["name"].lower())

    def on_apps_changed(self, monitor):
        self.apps.clear()
        self.load_apps()
        self.on_search_changed(self.search_entry)

    def build_ui(self):
        main_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=16)
        main_box.set_margin_top(24)
        main_box.set_margin_bottom(24)
        main_box.set_margin_start(24)
        main_box.set_margin_end(24)
        self.set_content(main_box)

        # Search Bar
        self.search_entry = Gtk.SearchEntry()
        if self.runner_mode:
            self.search_entry.set_placeholder_text("Run command...")
        else:
            self.search_entry.set_placeholder_text("Search applications...")
        self.search_entry.set_hexpand(True)
        self.search_entry.connect("search-changed", self.on_search_changed)
        self.search_entry.connect("activate", self.on_search_activate)
        main_box.append(self.search_entry)

        # Live Result Box
        self.live_result_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=16)
        self.live_result_box.add_css_class("live-result")
        self.live_result_box.set_visible(False)
        self.live_result_box.set_cursor(Gdk.Cursor.new_from_name("pointer", None))

        # Add a gesture to make it clickable
        click_gesture = Gtk.GestureClick()
        click_gesture.connect("released", lambda *_: self.on_live_result_clicked())
        self.live_result_box.add_controller(click_gesture)

        self.live_result_icon = Gtk.Image()
        self.live_result_icon.set_pixel_size(32)
        self.live_result_box.append(self.live_result_icon)

        labels_vbox = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=2)
        self.live_result_title = Gtk.Label(xalign=0)
        self.live_result_title.add_css_class("dim-label")
        self.live_result_value = Gtk.Label(xalign=0)
        self.live_result_value.add_css_class("live-result-value")

        labels_vbox.append(self.live_result_title)
        labels_vbox.append(self.live_result_value)
        self.live_result_box.append(labels_vbox)

        main_box.append(self.live_result_box)
        self.live_callback = None

        # Content Widget
        if self.runner_mode:
            self.history_listbox = Gtk.ListBox()
            self.history_listbox.add_css_class("history-list")
            self.history_listbox.set_selection_mode(Gtk.SelectionMode.SINGLE)
            self.history_listbox.connect("row-activated", self.on_history_activated)
        else:
            self.flowbox = Gtk.FlowBox()
            self.flowbox.set_valign(Gtk.Align.START)
            self.flowbox.set_halign(Gtk.Align.FILL)
            self.flowbox.set_selection_mode(Gtk.SelectionMode.NONE)
            self.flowbox.set_max_children_per_line(5)
            self.flowbox.set_min_children_per_line(5)
            self.flowbox.set_column_spacing(12)
            self.flowbox.set_row_spacing(12)
            self.flowbox.set_activate_on_single_click(True)
            self.flowbox.connect("child-activated", self.on_app_activated)

        # Stack for Content
        self.stack = Gtk.Stack()
        self.stack.set_transition_type(Gtk.StackTransitionType.CROSSFADE)
        self.stack.set_vexpand(True)
        main_box.append(self.stack)

        # 1. Grid/List Page
        scrolled = Gtk.ScrolledWindow()
        if self.runner_mode:
            scrolled.set_child(self.history_listbox)
        else:
            scrolled.set_child(self.flowbox)
        scrolled.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
        self.stack.add_named(scrolled, "grid")

        # 2. Empty Page
        self.empty_page = Adw.StatusPage()
        self.empty_page.set_icon_name("system-search-symbolic")
        self.empty_page.set_title("No Results")
        self.empty_page.set_description("Try a different search query")
        self.stack.add_named(self.empty_page, "empty")

        self.stack.set_visible_child_name("grid")

    def evaluate_math(self, query):
        if not re.match(r"^[\d\s\+\-\*\/\.\(\)%]+$", query):
            return None
        if not any(c in query for c in "+-*/%"):
            return None
        if "**" in query:
            return None
        try:
            res = eval(query, {"__builtins__": None, "math": math}, {})
            if isinstance(res, (int, float)):
                return f"{res:g}"
        except Exception:
            pass
        return None

    def run_terminal(self, cmd):
        def worker():
            try:
                result = subprocess.run(cmd, shell=True, capture_output=True, text=True, timeout=10)
                output = (
                    result.stdout.strip() or result.stderr.strip() or "Command finished (no output)"
                )
                GLib.idle_add(
                    self.show_live_result,
                    "Command Output",
                    output,
                    "utilities-terminal-symbolic",
                    lambda: self.copy_to_clipboard(output),
                    True,
                )
            except subprocess.TimeoutExpired:
                GLib.idle_add(
                    self.show_live_result,
                    "Error",
                    "Command timed out",
                    "dialog-error-symbolic",
                    None,
                )
            except Exception as e:
                GLib.idle_add(self.show_live_result, "Error", str(e), "dialog-error-symbolic", None)

        self.showing_output = True
        self.show_live_result("Running...", cmd, "view-refresh-symbolic", None)
        threading.Thread(target=worker, daemon=True).start()

    def hide_window(self):
        self.set_visible(False)
        self.search_entry.set_text("")
        self.hide_live_result()

    def copy_to_clipboard(self, text):
        clipboard = Gdk.Display.get_default().get_clipboard()
        clipboard.set(text)
        self.hide_window()

    def on_live_result_clicked(self):
        if self.live_callback:
            self.live_callback()

    def show_live_result(self, title, value, icon_name, callback, is_output=False):
        self.live_result_title.set_label(title)
        self.live_result_value.set_label(value)
        self.live_result_icon.set_from_icon_name(icon_name)
        self.live_callback = callback

        if is_output:
            self.live_result_value.add_css_class("live-result-output")
            self.live_result_value.remove_css_class("live-result-value")
        else:
            self.live_result_value.add_css_class("live-result-value")
            self.live_result_value.remove_css_class("live-result-output")

        self.live_result_box.set_visible(True)

    def hide_live_result(self):
        self.live_result_box.set_visible(False)
        self.live_callback = None
        self.showing_output = False

    def on_search_activate(self, entry):
        if self.live_callback:
            self.on_live_result_clicked()
            return

        if self.runner_mode:
            query = entry.get_text().strip()
            if query:
                self.run_command_line(query)
            return

        child = self.flowbox.get_child_at_index(0)
        if child:
            self.on_app_activated(self.flowbox, child)

    def on_search_changed(self, entry):
        query = entry.get_text().strip()
        lower_query = query.lower()

        if self.showing_output:
            if not query.startswith(">"):
                self.hide_live_result()

        if not self.showing_output:
            if self.runner_mode:
                self.history_listbox.remove_all()
            else:
                self.flowbox.remove_all()
            self.hide_live_result()

        has_results = False

        # 1. Math Solver
        if query:
            math_res = self.evaluate_math(query)
            if math_res:
                self.show_live_result(
                    "Calculator",
                    math_res,
                    "accessories-calculator-symbolic",
                    lambda: self.copy_to_clipboard(math_res),
                )
                has_results = True
                self.showing_output = False

        # 2. Terminal Runner
        if query.startswith(">"):
            cmd = query[1:].strip()
            if cmd:
                if self.showing_output:
                    has_results = True
                else:
                    self.show_live_result(
                        "Run Command",
                        cmd,
                        "utilities-terminal-symbolic",
                        lambda: self.run_terminal(cmd),
                    )
                    has_results = True
        else:
            self.showing_output = False

        # 3. Content Filtering
        if self.runner_mode:
            if not query.startswith(">"):
                if not lower_query:
                    self.filtered_history = self.history
                else:
                    self.filtered_history = [h for h in self.history if lower_query in h.lower()]
                    self.show_live_result(
                        "Run Command",
                        query,
                        "utilities-terminal-symbolic",
                        lambda: self.run_command_line(query),
                    )
                    has_results = True

                for cmd in self.filtered_history[:10]:
                    self.history_listbox.append(self.make_history_row(cmd))
                    has_results = True
        else:
            if not query.startswith(">"):
                if not lower_query:
                    self.filtered_apps = self.apps
                else:
                    self.filtered_apps = [
                        app for app in self.apps if lower_query in app["search_text"]
                    ]

                    def get_match_score(app_item):
                        name_l = app_item["name"].lower()
                        if name_l == lower_query:
                            return 0
                        if name_l.startswith(lower_query):
                            return 1
                        if f" {lower_query}" in name_l:
                            return 2
                        if lower_query in name_l:
                            return 3
                        return 4

                    self.filtered_apps.sort(key=lambda x: (get_match_score(x), x["name"].lower()))

                for _i, app in enumerate(self.filtered_apps[:50]):
                    self.flowbox.append(self.make_app_button(app))
                    has_results = True

        if has_results:
            self.stack.set_visible_child_name("grid")
        else:
            self.stack.set_visible_child_name("empty")

    def make_app_button(self, app_data):
        content_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
        content_box.set_halign(Gtk.Align.CENTER)
        content_box.add_css_class("app-card")
        content_box.app_info = app_data["app_info"]

        if app_data["icon"]:
            icon_img = Gtk.Image.new_from_gicon(app_data["icon"])
        else:
            icon_img = Gtk.Image.new_from_icon_name("application-x-executable")
        icon_img.set_pixel_size(48)
        content_box.append(icon_img)

        lbl = Gtk.Label(label=app_data["name"])
        lbl.add_css_class("app-label")
        lbl.set_ellipsize(Pango.EllipsizeMode.END)
        lbl.set_max_width_chars(12)
        lbl.set_wrap(False)
        content_box.append(lbl)

        return content_box

    def make_history_row(self, cmd_text):
        row = Gtk.ListBoxRow()
        box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=12)
        box.set_margin_start(12)
        box.set_margin_end(12)
        box.set_margin_top(8)
        box.set_margin_bottom(8)

        icon = Gtk.Image.new_from_icon_name("document-open-recent-symbolic")
        icon.add_css_class("dim-label")

        lbl = Gtk.Label(label=cmd_text)
        lbl.set_halign(Gtk.Align.START)

        box.append(icon)
        box.append(lbl)
        row.set_child(box)
        row.cmd_text = cmd_text
        return row

    def on_history_activated(self, listbox, row):
        if row and hasattr(row, "cmd_text"):
            self.run_command_line(row.cmd_text)

    def run_command_line(self, cmd):
        cmd = cmd.strip()
        if not cmd:
            return

        if cmd in self.history:
            self.history.remove(cmd)
        self.history.insert(0, cmd)
        save_runner_history(self.history)

        try:
            subprocess.Popen(cmd, shell=True, start_new_session=True)
        except Exception as e:
            print(f"Error running command: {e}")

        self.hide_window()

    def run_in_terminal(self, cmd):
        cmd = cmd.strip()
        if not cmd:
            return
        if cmd in self.history:
            self.history.remove(cmd)
        self.history.insert(0, cmd)
        save_runner_history(self.history)
        subprocess.Popen(
            ["kitty", "--hold", "sh", "-lc", cmd],
            start_new_session=True,
        )
        self.hide_window()

    def setup_keyboard(self):
        ctrl = Gtk.EventControllerKey()
        ctrl.set_propagation_phase(Gtk.PropagationPhase.CAPTURE)
        ctrl.connect("key-pressed", self.on_key_pressed)
        self.add_controller(ctrl)

    def on_key_pressed(self, _, keyval, keycode, state):
        if keyval == Gdk.KEY_Escape:
            self.hide_window()
            return True

        name = Gtk.accelerator_name(keyval, state)

        if self.runner_mode:
            if name == "Down":
                if self.search_entry.has_focus():
                    row = self.history_listbox.get_row_at_index(0)
                    if row:
                        row.grab_focus()
                        self.history_listbox.select_row(row)
                    return True
                else:
                    selected = self.history_listbox.get_selected_row()
                    if selected:
                        idx = selected.get_index()
                        next_row = self.history_listbox.get_row_at_index(idx + 1)
                        if next_row:
                            next_row.grab_focus()
                            self.history_listbox.select_row(next_row)
                        return True
            elif name == "Up":
                if not self.search_entry.has_focus():
                    selected = self.history_listbox.get_selected_row()
                    if selected:
                        idx = selected.get_index()
                        if idx == 0:
                            self.search_entry.grab_focus()
                            self.history_listbox.unselect_all()
                        else:
                            prev_row = self.history_listbox.get_row_at_index(idx - 1)
                            if prev_row:
                                prev_row.grab_focus()
                                self.history_listbox.select_row(prev_row)
                        return True
            elif name == "<Shift>Return":
                query = self.search_entry.get_text().strip()
                if not query:
                    selected = self.history_listbox.get_selected_row()
                    if selected and hasattr(selected, "cmd_text"):
                        query = selected.cmd_text
                if query:
                    self.run_in_terminal(query)
                return True
            elif name in ("Return", "KP_Enter"):
                if not self.search_entry.has_focus():
                    selected = self.history_listbox.get_selected_row()
                    if selected and hasattr(selected, "cmd_text"):
                        self.run_command_line(selected.cmd_text)
                        return True
        else:
            if name == "Down":
                if self.search_entry.has_focus():
                    child = self.flowbox.get_child_at_index(0)
                    if child:
                        child.grab_focus()
                    return True
            elif name == "Up":
                if not self.search_entry.has_focus():
                    self.search_entry.grab_focus()
                    return True

        if len(name) == 1 and not state & (
            Gdk.ModifierType.CONTROL_MASK | Gdk.ModifierType.ALT_MASK
        ):
            if not self.search_entry.has_focus():
                self.search_entry.grab_focus()
        return False

    def on_app_activated(self, flowbox, child):
        content = child.get_child()
        if hasattr(content, "callback"):
            content.callback()
            return

        if hasattr(content, "app_info"):
            try:
                context = Gdk.Display.get_default().get_app_launch_context()
                content.app_info.launch([], context)
                self.hide_window()
            except Exception as e:
                print(f"Error launching app: {e}")

    def load_css(self):
        css = b"""
        window { background: alpha(@window_bg_color, 0.95); }
        flowboxchild {
            padding: 12px;
            border-radius: 12px;
            background: transparent;
            min-width: 100px;
            min-height: 100px;
            transition: all 200ms;
        }
        flowboxchild:hover, flowboxchild:focus {
            background: alpha(@accent_bg_color, 0.1);
        }
        flowboxchild:selected {
            background: alpha(@accent_bg_color, 0.2);
        }
        .history-list row {
            border-radius: 8px;
            margin: 2px 0;
            padding: 2px;
            transition: all 150ms;
        }
        .history-list row:hover, .history-list row:focus {
            background: alpha(@accent_bg_color, 0.1);
        }
        .history-list row:selected {
            background: @accent_bg_color;
            color: @accent_fg_color;
        }
        .app-label {
            font-size: 0.9em;
            font-weight: 600;
            color: @view_fg_color;
        }
        .dim-label {
            font-size: 0.9em;
            opacity: 0.6;
        }
        .live-result {
            background: alpha(@window_bg_color, 0.4);
            border-radius: 12px;
            padding: 16px;
            border: 1px solid alpha(@window_fg_color, 0.1);
            transition: all 200ms;
        }
        .live-result:hover {
            background: alpha(@accent_bg_color, 0.1);
            border: 1px solid alpha(@accent_bg_color, 0.3);
        }
        .live-result-value {
            font-size: 1.5em;
            font-weight: bold;
            color: @accent_color;
        }
        .live-result-output {
            font-family: monospace;
            font-size: 1.1em;
            color: @view_fg_color;
        }
        """
        provider = Gtk.CssProvider()
        provider.load_from_data(css)
        Gtk.StyleContext.add_provider_for_display(
            Gdk.Display.get_default(), provider, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION
        )


class AppMenu(Adw.Application):
    def __init__(self):
        super().__init__(application_id="org.fcosta.AppMenu", flags=Gio.ApplicationFlags.FLAGS_NONE)
        self.hold()
        self.win = None

    def do_activate(self):
        if not self.win:
            self.win = AppMenuWindow(self)

        self.win.present()
        self.win.search_entry.grab_focus()


def run():
    if dispatch_to_main("--app-menu"):
        return

    app = AppMenu()
    app.run([])


if __name__ == "__main__":
    run()
