#!/usr/bin/env python3
import json
import os
import shutil
import subprocess
from datetime import datetime

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
gi.require_version("Gdk", "4.0")
from gi.repository import Adw, Gdk, GdkPixbuf, Gio, GLib, GObject, Gtk, Pango

try:
    from .dispatch import dispatch_to_main
except ImportError:
    from dispatch import dispatch_to_main

VERSION = "1.0.0"
CONFIG_DIR = os.path.expanduser("~/.config/blinker")
PINS_FILE = os.path.join(CONFIG_DIR, "pins")
STATE_FILE = os.path.join(CONFIG_DIR, "state.json")
CONFIG_FILE = os.path.join(CONFIG_DIR, "settings.json")
DEFAULT_CONFIG = {
    "screenshot_dir": "~/Pictures/Screenshots",
    "naming_pattern": "Screenshot_%Y%m%d_%H%M%S",
    "copy_to_clipboard": True,
    "show_notification": True,
    "open_manager_after_capture": True,
}


def load_config():
    try:
        with open(CONFIG_FILE, "r") as f:
            config = json.load(f)
            return {**DEFAULT_CONFIG, **config}
    except Exception:
        return DEFAULT_CONFIG.copy()


def save_config(config):
    os.makedirs(CONFIG_DIR, exist_ok=True)
    with open(CONFIG_FILE, "w") as f:
        json.dump(config, f, indent=2)


def get_screenshots_dir():
    return os.path.abspath(os.path.expanduser(load_config()["screenshot_dir"]))


def unique_destination(directory, filename):
    """Choose a destination without overwriting an existing screenshot."""
    stem, extension = os.path.splitext(filename)
    candidate = os.path.join(directory, filename)
    suffix = 1
    while os.path.exists(candidate):
        candidate = os.path.join(directory, f"{stem}_{suffix}{extension}")
        suffix += 1
    return candidate


class SettingsDialog(Adw.Window):
    def __init__(self, parent):
        super().__init__(title="Settings", transient_for=parent)
        self.set_default_size(450, 400)
        self.config = load_config()

        view = Adw.ToolbarView()
        self.set_content(view)

        header = Adw.HeaderBar()
        header.set_title("Settings")
        view.add_top_bar(header)

        main_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=12)
        main_box.set_margin_top(12)
        main_box.set_margin_bottom(12)
        main_box.set_margin_start(12)
        main_box.set_margin_end(12)
        view.set_content(main_box)

        # Screenshot directory
        dir_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=6)
        dir_box.append(Gtk.Label(label="Screenshot Directory"))
        dir_row = Gtk.Box(spacing=8)
        self.dir_entry = Gtk.Entry()
        self.dir_entry.set_text(self.config.get("screenshot_dir", DEFAULT_CONFIG["screenshot_dir"]))
        self.dir_entry.set_hexpand(True)
        dir_browse = Gtk.Button(label="Browse")
        dir_browse.connect("clicked", self.on_browse_dir)
        dir_row.append(self.dir_entry)
        dir_row.append(dir_browse)
        dir_box.append(dir_row)
        main_box.append(dir_box)

        # Naming pattern
        pattern_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=6)
        pattern_box.append(Gtk.Label(label="Naming Pattern"))
        self.pattern_entry = Gtk.Entry()
        self.pattern_entry.set_text(
            self.config.get("naming_pattern", DEFAULT_CONFIG["naming_pattern"])
        )
        self.pattern_entry.set_hexpand(True)
        pattern_box.append(self.pattern_entry)
        pattern_help = Gtk.Label(
            label="Use strftime format: %Y=year, %m=month, %d=day, %H=hour, %M=minute, %S=second"
        )
        pattern_help.add_css_class("dim-label")
        pattern_box.append(pattern_help)
        main_box.append(pattern_box)

        # Toggles
        self.clipboard_switch = Gtk.Switch()
        self.clipboard_switch.set_active(self.config.get("copy_to_clipboard", True))
        clipboard_row = Adw.ActionRow(
            title="Copy to Clipboard", subtitle="Automatically copy screenshot to clipboard"
        )
        clipboard_row.add_suffix(self.clipboard_switch)
        main_box.append(clipboard_row)

        self.notification_switch = Gtk.Switch()
        self.notification_switch.set_active(self.config.get("show_notification", True))
        notification_row = Adw.ActionRow(
            title="Show Notification", subtitle="Display notification after capture"
        )
        notification_row.add_suffix(self.notification_switch)
        main_box.append(notification_row)

        self.open_manager_switch = Gtk.Switch()
        self.open_manager_switch.set_active(self.config.get("open_manager_after_capture", True))
        open_manager_row = Adw.ActionRow(
            title="Open Manager After Capture", subtitle="Launch manager after taking screenshot"
        )
        open_manager_row.add_suffix(self.open_manager_switch)
        main_box.append(open_manager_row)

        # Save button
        save_btn = Gtk.Button(label="Save")
        save_btn.add_css_class("suggested-action")
        save_btn.set_halign(Gtk.Align.CENTER)
        save_btn.set_margin_top(12)
        save_btn.connect("clicked", self.on_save)
        main_box.append(save_btn)

    def on_browse_dir(self, _):
        dialog = Gtk.FileDialog()
        dialog.set_initial_file(
            Gio.File.new_for_path(os.path.expanduser(self.dir_entry.get_text()))
        )
        dialog.select_folder(self, None, lambda d, r: self.on_dir_selected(d, r))

    def on_dir_selected(self, dialog, result):
        try:
            folder = dialog.select_folder_finish(result)
            if folder:
                self.dir_entry.set_text(folder.get_path())
        except Exception:
            pass

    def on_save(self, _):
        self.config["screenshot_dir"] = self.dir_entry.get_text()
        self.config["naming_pattern"] = self.pattern_entry.get_text()
        self.config["copy_to_clipboard"] = self.clipboard_switch.get_active()
        self.config["show_notification"] = self.notification_switch.get_active()
        self.config["open_manager_after_capture"] = self.open_manager_switch.get_active()
        save_config(self.config)
        if hasattr(self.get_transient_for(), "refresh_screenshot_directory"):
            self.get_transient_for().refresh_screenshot_directory()
        self.close()


class BlinkerManagerWindow(Adw.ApplicationWindow):
    def __init__(self, app):
        super().__init__(application=app, title="Blinker Manager")
        self.set_default_size(1100, 750)

        self.files = []
        self.filtered = []
        self.pinned = self.load_pins()
        self.current_file = None
        self.thumb_cache = {}
        self.row_cache = {}
        self.monitor = None
        self.screenshots_dir = get_screenshots_dir()

        self.load_css()
        self.build_ui()
        self.reload()
        self.setup_monitor()

        self.connect("close-request", self.on_close_request)

    def load_state(self):
        try:
            with open(STATE_FILE, "r") as f:
                return json.load(f)
        except Exception:
            return {}

    def on_close_request(self, *args):
        width, height = self.get_width(), self.get_height()
        os.makedirs(CONFIG_DIR, exist_ok=True)
        with open(STATE_FILE, "w") as f:
            json.dump({"width": width, "height": height}, f)
        self.hide()
        return True

    def build_ui(self):
        view = Adw.ToolbarView()
        self.set_content(view)

        # Header Bar
        # Header Bar
        header = Adw.HeaderBar()
        view.add_top_bar(header)

        # Sidebar Toggle
        self.sidebar_button = Gtk.ToggleButton(icon_name="view-sidebar-symbolic")
        self.sidebar_button.set_active(True)
        self.sidebar_button.set_tooltip_text("Toggle Sidebar")
        header.pack_start(self.sidebar_button)

        # View Toggle
        self.view_toggle = Gtk.Button(icon_name="view-grid-symbolic")
        self.view_toggle.set_tooltip_text("Toggle Grid View")
        self.view_toggle.connect("clicked", self.on_view_toggle_clicked)
        header.pack_start(self.view_toggle)

        self.search = Gtk.SearchEntry()
        self.search.set_placeholder_text("Search screenshots...")
        self.search.set_hexpand(True)
        self.search.connect("search-changed", self.on_search_changed)
        header.set_title_widget(self.search)

        menu_button = Gtk.MenuButton()
        menu_button.set_icon_name("open-menu-symbolic")
        header.pack_end(menu_button)

        menu = Gio.Menu.new()

        settings_item = Gio.MenuItem.new("Settings", "app.settings")
        menu.append_item(settings_item)

        menu.append("About Blinker Manager", "app.about")
        menu_button.set_menu_model(menu)

        # Main Layout
        main_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
        main_box.set_margin_start(12)
        main_box.set_margin_end(12)
        main_box.set_margin_top(12)
        main_box.set_margin_bottom(12)
        view.set_content(main_box)

        # Actions
        actions_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=6)
        main_box.append(actions_box)

        # Image Actions Group
        img_actions = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL)
        img_actions.add_css_class("linked")

        self.copy_btn = Gtk.Button(icon_name="edit-copy-symbolic")
        self.copy_btn.set_tooltip_text("Copy Image")
        self.copy_btn.connect("clicked", lambda _: self.copy_current())
        img_actions.append(self.copy_btn)

        self.ocr_btn = Gtk.Button(icon_name="insert-text-symbolic")
        self.ocr_btn.set_tooltip_text("Extract Text (OCR)")
        self.ocr_btn.connect("clicked", self.on_ocr_clicked)
        self.has_tesseract = shutil.which("tesseract") is not None
        self.ocr_btn.set_visible(self.has_tesseract)
        img_actions.append(self.ocr_btn)

        self.edit_btn = Gtk.Button(icon_name="document-edit-symbolic")
        self.edit_btn.set_tooltip_text("Open in Editor")
        self.edit_btn.connect("clicked", self.on_edit_clicked)
        img_actions.append(self.edit_btn)

        actions_box.append(img_actions)

        # Management Actions Group
        mgmt_actions = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL)
        mgmt_actions.add_css_class("linked")

        self.pin_btn = Gtk.Button(icon_name="user-bookmarks-symbolic")
        self.pin_btn.set_tooltip_text("Pin (Ctrl+P)")
        self.pin_btn.connect("clicked", self.on_pin_clicked)
        mgmt_actions.append(self.pin_btn)

        self.move_btn = Gtk.Button(icon_name="document-save-as-symbolic")
        self.move_btn.set_tooltip_text("Move to Folder")
        self.move_btn.connect("clicked", self.on_move_clicked)
        mgmt_actions.append(self.move_btn)

        actions_box.append(mgmt_actions)

        self.info_btn = Gtk.ToggleButton(icon_name="info-symbolic")
        self.info_btn.set_tooltip_text("Show Info (I)")
        self.info_btn.set_active(True)
        self.info_btn.connect("toggled", self.on_info_toggled)
        actions_box.append(self.info_btn)

        spacer = Gtk.Box()
        spacer.set_hexpand(True)
        actions_box.append(spacer)

        self.delete_btn = Gtk.Button(icon_name="user-trash-symbolic")
        self.delete_btn.add_css_class("destructive-action")
        self.delete_btn.connect("clicked", self.on_delete_clicked)
        actions_box.append(self.delete_btn)

        # Split View
        self.split_view = Adw.OverlaySplitView()
        self.split_view.set_vexpand(True)
        self.split_view.set_hexpand(True)
        self.split_view.set_min_sidebar_width(320)
        self.split_view.set_sidebar_width_fraction(0.35)
        self.sidebar_button.bind_property(
            "active", self.split_view, "show-sidebar", GObject.BindingFlags.BIDIRECTIONAL
        )
        main_box.append(self.split_view)

        # Status bar
        self.status_bar = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
        self.status_bar.set_margin_top(6)

        self.total_count_label = Gtk.Label(label="0 screenshots")
        self.total_count_label.add_css_class("dim-label")

        spacer = Gtk.Box()
        spacer.set_hexpand(True)

        self.selected_count_label = Gtk.Label(label="")
        self.selected_count_label.add_css_class("dim-label")

        self.status_bar.append(self.total_count_label)
        self.status_bar.append(spacer)
        self.status_bar.append(self.selected_count_label)
        main_box.append(self.status_bar)

        self.view_stack = Gtk.Stack()
        self.view_stack.set_transition_type(Gtk.StackTransitionType.CROSSFADE)

        self.listbox = Gtk.ListBox()
        self.listbox.add_css_class("history-list")
        self.listbox.set_selection_mode(Gtk.SelectionMode.SINGLE)
        self.listbox.connect("selected-rows-changed", self.on_selection_changed)

        self.last_selected_row = None
        self.listbox_controller = Gtk.GestureClick()
        self.listbox_controller.connect("released", self.on_listbox_click)
        self.listbox.add_controller(self.listbox_controller)

        self.grid_view = Gtk.FlowBox()
        self.grid_view.set_valign(Gtk.Align.START)
        self.grid_view.set_max_children_per_line(15)
        self.grid_view.set_selection_mode(Gtk.SelectionMode.SINGLE)
        self.grid_view.connect("selected-children-changed", self.on_grid_selection_changed)
        self.grid_view.add_css_class("history-grid")

        self.view_stack.add_titled(self.listbox, "list", "List")

        grid_scroll = Gtk.ScrolledWindow()
        grid_scroll.set_child(self.grid_view)
        self.view_stack.add_titled(grid_scroll, "grid", "Grid")

        left_scroll = Gtk.ScrolledWindow()
        left_scroll.add_css_class("sidebar-scroll")
        left_scroll.set_min_content_width(320)
        left_scroll.set_vexpand(True)
        left_scroll.set_child(self.view_stack)
        self.split_view.set_sidebar(left_scroll)

        self.preview_stack = Gtk.Stack()
        self.preview_stack.set_transition_type(Gtk.StackTransitionType.CROSSFADE)
        preview_frame = Gtk.Frame()
        preview_frame.add_css_class("preview-pane")
        preview_frame.set_child(self.preview_stack)
        self.split_view.set_content(preview_frame)

        self.preview_image = Gtk.Picture()
        self.preview_image.set_can_shrink(True)
        self.preview_image.set_content_fit(Gtk.ContentFit.CONTAIN)

        self.zoom_level = 1.0
        self.info_visible = True

        drag_source = Gtk.DragSource()
        drag_source.connect("prepare", self.on_drag_prepare)
        self.preview_image.add_controller(drag_source)

        self.preview_viewport = Gtk.Viewport()
        self.preview_viewport.set_child(self.preview_image)
        self.preview_viewport.set_hscroll_policy(Gtk.ScrollablePolicy.NATURAL)
        self.preview_viewport.set_vscroll_policy(Gtk.ScrollablePolicy.NATURAL)

        img_scroller = Gtk.ScrolledWindow()
        img_scroller.set_child(self.preview_viewport)

        scroll_controller = Gtk.EventControllerScroll.new(
            Gtk.EventControllerScrollFlags.VERTICAL | Gtk.EventControllerScrollFlags.DISCRETE
        )
        scroll_controller.connect("scroll", self.on_preview_scroll)
        img_scroller.add_controller(scroll_controller)

        self.preview_overlay = Gtk.Overlay()
        self.preview_overlay.set_child(img_scroller)

        # Metadata Pill
        self.info_pill = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=16)
        self.info_pill.add_css_class("preview-info-pill")
        self.info_pill.set_halign(Gtk.Align.CENTER)
        self.info_pill.set_valign(Gtk.Align.END)
        self.info_pill.set_margin_bottom(24)

        def make_info_item(icon_name):
            box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=6)
            icon = Gtk.Image.new_from_icon_name(icon_name)
            icon.add_css_class("dim-label")
            label = Gtk.Label()
            label.add_css_class("info-label")
            box.append(icon)
            box.append(label)
            return box, label

        self.dims_item, self.metadata_dims = make_info_item("view-fullscreen-symbolic")
        self.size_item, self.metadata_size = make_info_item("folder-symbolic")
        self.date_item, self.metadata_date = make_info_item("calendar-symbolic")

        self.info_pill.append(self.dims_item)
        self.info_pill.append(self.size_item)
        self.info_pill.append(self.date_item)

        self.preview_overlay.add_overlay(self.info_pill)

        self.selection_label = Gtk.Label(label="")
        self.selection_label.add_css_class("selection-badge")
        self.selection_label.set_halign(Gtk.Align.END)
        self.selection_label.set_valign(Gtk.Align.START)
        self.selection_label.set_margin_top(12)
        self.selection_label.set_margin_end(12)
        self.preview_overlay.add_overlay(self.selection_label)

        self.empty_status = Adw.StatusPage()
        self.empty_status.set_title("No Selection")
        self.empty_status.set_description("Select a screenshot to view details")
        self.empty_status.set_icon_name("camera-photo-symbolic")

        self.preview_stack.add_titled(self.empty_status, "empty", "Empty")
        self.preview_stack.add_titled(self.preview_overlay, "image", "Image")
        self.preview_stack.set_visible_child_name("empty")

        # Selection Bar
        self.selection_bar = Gtk.ActionBar()
        self.selection_bar.set_revealed(False)

        delete_btn = Gtk.Button(icon_name="user-trash-symbolic")
        delete_btn.add_css_class("destructive-action")
        delete_btn.connect("clicked", self.on_delete_clicked)
        self.selection_bar.pack_start(delete_btn)

        copy_btn = Gtk.Button(icon_name="edit-copy-symbolic")
        copy_btn.connect("clicked", lambda _: self.copy_current())
        self.selection_bar.pack_start(copy_btn)

        move_btn = Gtk.Button(icon_name="folder-open-symbolic")
        move_btn.connect("clicked", self.on_move_clicked)
        self.selection_bar.pack_start(move_btn)

        self.selection_bar_label = Gtk.Label()
        self.selection_bar_label.set_margin_start(12)
        self.selection_bar.set_center_widget(self.selection_bar_label)

        close_selection = Gtk.Button(icon_name="window-close-symbolic")
        close_selection.add_css_class("flat")
        close_selection.connect("clicked", lambda _: self.listbox.unselect_all())
        self.selection_bar.pack_end(close_selection)

        main_box.append(self.selection_bar)

        ctrl = Gtk.EventControllerKey()
        ctrl.connect("key-pressed", self.on_key_pressed)
        self.add_controller(ctrl)

    def load_css(self):
        css = b"""
        .history-list {
            background: transparent;
        }
        .sidebar-scroll {
            background: alpha(@view_fg_color, 0.01);
            border-right: 1px solid alpha(@view_fg_color, 0.05);
        }
        .history-list row { 
            margin: 4px 8px; 
            border-radius: 10px; 
            transition: background 150ms ease-in-out;
            background: transparent;
        }
        .history-list row:hover { 
            background: alpha(@view_fg_color, 0.02); 
        }
        .history-list row:selected { 
            background: @accent_bg_color;
            color: @accent_fg_color;
        }
        .history-list row:selected:hover {
            background: alpha(@accent_bg_color, 0.9);
        }
        .history-row { padding: 8px 10px; }
        .thumb-frame { min-width: 64px; min-height: 48px; border-radius: 6px; background: alpha(@view_fg_color, 0.08); }
        .preview-pane { border-radius: 14px; padding: 8px; background: alpha(@view_fg_color, 0.04); }
        .destructive-action { color: @error_color; }
        .selection-count { font-size: 0.85em; font-weight: bold; background: alpha(@accent_bg_color, 0.8); color: @accent_fg_color; padding: 4px 8px; border-radius: 8px; }
        .dim-label { font-size: 0.75em; opacity: 0.7; }
        .grid-thumb-frame {
            border-radius: 8px;
            background: alpha(@view_fg_color, 0.05);
            padding: 4px;
        }
        .history-grid flowboxchild {
            padding: 6px;
            border-radius: 10px;
        }
        .history-grid flowboxchild:selected {
            background: alpha(@accent_bg_color, 0.3);
        }
        .history-grid flowboxchild:selected .grid-thumb-frame {
            background: @accent_bg_color;
        }
        .preview-info-pill {
            background: alpha(@window_bg_color, 0.85);
            padding: 8px 18px;
            border-radius: 24px;
            border: 1px solid alpha(@view_fg_color, 0.1);
            box-shadow: 0 4px 12px alpha(black, 0.15);
        }
        .info-label {
            font-size: 0.85em;
            font-weight: bold;
            color: @view_fg_color;
        }
        .selection-badge {
            background: @accent_bg_color;
            color: @accent_fg_color;
            padding: 4px 10px;
            border-radius: 12px;
            font-size: 0.8em;
            font-weight: bold;
            box-shadow: 0 2px 6px alpha(black, 0.2);
        }
        """
        provider = Gtk.CssProvider()
        provider.load_from_data(css)
        Gtk.StyleContext.add_provider_for_display(
            Gdk.Display.get_default(), provider, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION
        )

    def setup_monitor(self):
        self._reload_pending = False
        if self.monitor is not None:
            self.monitor.cancel()
            self.monitor = None
        if os.path.exists(self.screenshots_dir):
            gio_file = Gio.File.new_for_path(self.screenshots_dir)
            self.monitor = gio_file.monitor_directory(Gio.FileMonitorFlags.NONE, None)
            self.monitor.connect("changed", self._on_file_changed)

    def refresh_screenshot_directory(self):
        self.screenshots_dir = get_screenshots_dir()
        os.makedirs(self.screenshots_dir, exist_ok=True)
        self.setup_monitor()
        self.reload()

    def _on_file_changed(self, *args):
        if self._reload_pending:
            return
        self._reload_pending = True
        GLib.timeout_add(300, self._do_reload)

    def _do_reload(self):
        self._reload_pending = False
        self.reload()

    def on_info_toggled(self, btn):
        self.info_visible = btn.get_active()
        if self.current_file:
            self.info_pill.set_visible(self.info_visible)

    def reload(self):
        if not os.path.exists(self.screenshots_dir):
            self.files = []
        else:
            files = [
                os.path.join(self.screenshots_dir, f)
                for f in os.listdir(self.screenshots_dir)
                if f.lower().endswith((".png", ".jpg", ".jpeg", ".webp"))
            ]
            files.sort(key=lambda x: os.path.getmtime(x), reverse=True)
            self.files = files

        if hasattr(self, "total_count_label"):
            total = len(self.files)
            self.total_count_label.set_label(f"{total} screenshot{'s' if total != 1 else ''}")

        self.apply_filter()

    def get_date_label(self, path):
        mtime = datetime.fromtimestamp(os.path.getmtime(path))
        now = datetime.now()
        diff = now.date() - mtime.date()

        if diff.days == 0:
            return "Today"
        if diff.days == 1:
            return "Yesterday"
        if diff.days < 7:
            return mtime.strftime("%A")
        return mtime.strftime("%B %d, %Y")

    def on_view_toggle_clicked(self, btn):
        current = self.view_stack.get_visible_child_name()
        if current == "list":
            self.view_stack.set_visible_child_name("grid")
            btn.set_icon_name("view-list-symbolic")
        else:
            self.view_stack.set_visible_child_name("list")
            btn.set_icon_name("view-grid-symbolic")

    def on_grid_selection_changed(self, flowbox):
        selected = flowbox.get_selected_children()
        if selected:
            path = selected[0]._file_path
            self.current_file = path
            GLib.idle_add(self._load_preview, path)
        else:
            if self.view_stack.get_visible_child_name() == "grid":
                self.current_file = None
                self.preview_stack.set_visible_child_name("empty")

    def get_selected_paths(self):
        if self.view_stack.get_visible_child_name() == "grid":
            return [
                child._file_path
                for child in self.grid_view.get_selected_children()
                if hasattr(child, "_file_path")
            ]

        return [
            row._file_path for row in self.listbox.get_selected_rows() if hasattr(row, "_file_path")
        ]

    def make_grid_item(self, path):
        item = Gtk.FlowBoxChild()
        item._file_path = path
        item.set_tooltip_text(os.path.basename(path))

        img = Gtk.Image()
        img.set_pixel_size(100)
        GLib.idle_add(self.load_row_thumb, path, img)

        frame = Gtk.Frame()
        frame.add_css_class("grid-thumb-frame")
        frame.set_child(img)
        item.set_child(frame)
        return item

    def apply_filter(self):
        query = self.search.get_text().strip()
        self.filtered = [
            f for f in self.files if not query or query.lower() in os.path.basename(f).lower()
        ]
        self.filtered.sort(key=lambda f: (f not in self.pinned, -os.path.getmtime(f)))

        # Clear views
        while row := self.listbox.get_first_child():
            self.listbox.remove(row)
        while child := self.grid_view.get_first_child():
            self.grid_view.remove(child)

        visible_paths = set(self.filtered)
        stale_paths = set(self.row_cache.keys()) - visible_paths
        for path in stale_paths:
            del self.row_cache[path]

        last_label = None
        for i, path in enumerate(self.filtered[:100]):
            # List View
            label = "Pinned" if path in self.pinned else self.get_date_label(path)
            if label != last_label:
                header_row = Gtk.ListBoxRow()
                header_row.set_selectable(False)
                lbl = Gtk.Label(label=label)
                lbl.add_css_class("heading")
                lbl.set_halign(Gtk.Align.START)
                lbl.set_margin_start(16)
                lbl.set_margin_top(12)
                lbl.set_margin_bottom(6)
                header_row.set_child(lbl)
                self.listbox.append(header_row)
                last_label = label

            if path not in self.row_cache:
                self.row_cache[path] = self.make_row(path, i + 1)
            self.listbox.append(self.row_cache[path])

            # Grid View
            self.grid_view.append(self.make_grid_item(path))

    def make_row(self, path, index):
        row = Gtk.ListBoxRow()
        row._file_path = path
        box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=10)
        box.add_css_class("history-row")
        row.set_child(box)
        thumb_img = Gtk.Image()
        thumb_img.set_pixel_size(60)

        # Check cache first
        if path in self.thumb_cache:
            thumb_img.set_from_paintable(self.thumb_cache[path])
        else:
            GLib.idle_add(self.load_row_thumb, path, thumb_img)

        thumb_frame = Gtk.Frame()
        thumb_frame.add_css_class("thumb-frame")
        thumb_frame.set_child(thumb_img)
        box.append(thumb_frame)
        lbl = Gtk.Label(label=os.path.basename(path))
        lbl.set_ellipsize(Pango.EllipsizeMode.END)
        box.append(lbl)
        return row

    def load_row_thumb(self, path, img):
        if path in self.thumb_cache:
            img.set_from_paintable(self.thumb_cache[path])
            return False
        if not os.path.exists(path):
            return False
        try:
            import warnings

            with warnings.catch_warnings():
                warnings.simplefilter("ignore", DeprecationWarning)
                pix = GdkPixbuf.Pixbuf.new_from_file_at_scale(path, 120, 90, True)
                texture = Gdk.Texture.new_for_pixbuf(pix)
            self.thumb_cache[path] = texture
            img.set_from_paintable(texture)
        except Exception:
            img.set_from_icon_name("image-missing-symbolic")
        return False

    def on_listbox_click(self, controller, n_press, x, y):
        event = controller.get_current_event()
        if event:
            state = event.get_modifier_state()
        else:
            state = 0

        row = self.listbox.get_row_at_y(y)
        if not row:
            return False

        if state & Gdk.ModifierType.SHIFT_MASK and self.last_selected_row:
            self.listbox.set_selection_mode(Gtk.SelectionMode.MULTIPLE)
            self.listbox.unselect_all()

            rows = []
            child = self.listbox.get_first_child()
            while child:
                rows.append(child)
                child = child.get_next_sibling()

            try:
                start_idx = rows.index(self.last_selected_row)
                end_idx = rows.index(row)
                for i in range(min(start_idx, end_idx), max(start_idx, end_idx) + 1):
                    self.listbox.select_row(rows[i])
            except ValueError:
                pass
        elif state & Gdk.ModifierType.CONTROL_MASK:
            self.listbox.set_selection_mode(Gtk.SelectionMode.MULTIPLE)
            selected = self.listbox.get_selected_rows()
            if row in selected:
                self.listbox.unselect_row(row)
            else:
                self.listbox.select_row(row)
            self.last_selected_row = row
        else:
            self.listbox.set_selection_mode(Gtk.SelectionMode.SINGLE)
            self.last_selected_row = row

        return False

    def _load_preview(self, path):
        if self.current_file == path:
            self.preview_image.set_file(Gio.File.new_for_path(path))
            self.preview_stack.set_visible_child_name("image")
            self.info_pill.set_visible(self.info_visible)

            try:
                stat = os.stat(path)
                file_size = stat.st_size
                mtime = datetime.fromtimestamp(stat.st_mtime)

                size_str = self._format_size(file_size)
                date_str = mtime.strftime("%Y-%m-%d %H:%M:%S")

                self.metadata_size.set_label(size_str)
                self.metadata_date.set_label(date_str)

                try:
                    pix = GdkPixbuf.Pixbuf.new_from_file(path)
                    self.img_width = pix.get_width()
                    self.img_height = pix.get_height()
                    self.metadata_dims.set_label(f"{self.img_width} x {self.img_height}")
                except Exception:
                    self.img_width = None
                    self.img_height = None
                    self.metadata_dims.set_label("Unknown")
            except Exception:
                self.metadata_size.set_label("")
                self.metadata_date.set_label("")
                self.metadata_dims.set_label("")

            self.zoom_level = 1.0
            self._apply_zoom()

        return False

    def _apply_zoom(self):
        w_orig = getattr(self, "img_width", None)
        h_orig = getattr(self, "img_height", None)

        if w_orig is None or h_orig is None:
            return

        z = self.zoom_level
        if z <= 1.0:
            self.preview_image.set_content_fit(Gtk.ContentFit.CONTAIN)
            self.preview_image.set_size_request(-1, -1)
        else:
            w = int(w_orig * z)
            h = int(h_orig * z)
            self.preview_image.set_content_fit(Gtk.ContentFit.FILL)
            self.preview_image.set_size_request(w, h)

    def _format_size(self, size):
        for unit in ["B", "KB", "MB", "GB"]:
            if size < 1024:
                return f"{size:.1f} {unit}"
            size /= 1024
        return f"{size:.1f} TB"

    def on_preview_scroll(self, controller, dx, dy):
        # Use Ctrl + Wheel to zoom, standard wheel to scroll
        event = controller.get_current_event()
        state = event.get_modifier_state() if event else 0

        if state & Gdk.ModifierType.CONTROL_MASK:
            if dy < 0:
                self.zoom_level = min(self.zoom_level * 1.1, 10.0)
            elif dy > 0:
                self.zoom_level = max(self.zoom_level / 1.1, 0.1)

            self._apply_zoom()
            return True

        return False  # Let the ScrolledWindow handle normal scrolling

    def on_drag_prepare(self, source, x, y):
        if self.current_file and os.path.exists(self.current_file):
            file = Gio.File.new_for_path(self.current_file)
            return Gdk.ContentProvider.new_for_value(file)
        return None

    def on_selection_changed(self, listbox):
        selected_rows = listbox.get_selected_rows()
        count = len(selected_rows)

        if count == 0:
            self.current_file = None
            self.preview_stack.set_visible_child_name("empty")
            self.selection_label.set_label("")
            self.selected_count_label.set_label("")
            self.selection_bar.set_revealed(False)
            self.info_pill.set_visible(False)
            return

        self.selection_bar.set_revealed(count > 1)
        if count > 1:
            self.selection_label.set_label(f"{count} selected")
            self.selection_label.set_visible(True)
            self.selected_count_label.set_label(f"{count} selected")
            self.selection_bar_label.set_label(f"{count} screenshots selected")
        else:
            self.selection_label.set_label("")
            self.selection_label.set_visible(False)
            self.selected_count_label.set_label("")

        # Filter out header rows
        valid_rows = [r for r in selected_rows if hasattr(r, "_file_path")]
        if not valid_rows:
            return

        self.current_file = valid_rows[-1]._file_path
        GLib.idle_add(self._load_preview, self.current_file)

    def copy_current(self):
        selected_paths = self.get_selected_paths()
        if not selected_paths and self.current_file:
            selected_paths = [self.current_file]
        if not selected_paths:
            return

        if len(selected_paths) > 1:
            uris = [Gio.File.new_for_path(path).get_uri() for path in selected_paths]
            uri_list = "\n".join(uris)
            subprocess.run(
                ["wl-copy", "--type", "text/uri-list"], input=uri_list.encode(), check=False
            )
        else:
            path = selected_paths[0]
            mime = "image/png" if path.lower().endswith(".png") else "image/jpeg"
            with open(path, "rb") as image_file:
                subprocess.run(
                    ["wl-copy", "--type", mime],
                    stdin=image_file,
                    check=False,
                )

    def on_ocr_clicked(self, _):
        if not self.current_file:
            return

        def run_ocr():
            try:
                proc = subprocess.run(
                    ["tesseract", self.current_file, "stdout"], capture_output=True, text=True
                )
                if proc.stdout.strip():
                    subprocess.run(["wl-copy"], input=proc.stdout.encode())
            except Exception:
                pass

        import threading

        threading.Thread(target=run_ocr).start()

    def on_edit_clicked(self, _):
        if self.current_file:
            swappy = shutil.which("swappy")
            if swappy:
                subprocess.Popen([swappy, self.current_file])
            else:
                subprocess.Popen(["xdg-open", self.current_file])

    def on_pin_clicked(self, _):
        selected_paths = self.get_selected_paths()
        if not selected_paths and self.current_file:
            selected_paths = [self.current_file]
        for path in selected_paths:
            if path in self.pinned:
                self.pinned.remove(path)
            else:
                self.pinned.add(path)
        self.save_pins()
        self.reload()

    def load_pins(self):
        try:
            with open(PINS_FILE, "r") as f:
                return {line.strip() for line in f}
        except Exception:
            return set()

    def save_pins(self):
        os.makedirs(CONFIG_DIR, exist_ok=True)
        with open(PINS_FILE, "w") as f:
            for p in self.pinned:
                f.write(f"{p}\n")

    def on_delete_clicked(self, _):
        selected_paths = self.get_selected_paths()
        if not selected_paths and self.current_file:
            selected_paths = [self.current_file]
        if not selected_paths:
            return
        try:
            for path in selected_paths:
                if os.path.exists(path):
                    file = Gio.File.new_for_path(path)
                    file.trash(None)
                    self.pinned.discard(path)
                    if path in self.row_cache:
                        del self.row_cache[path]
                    if path in self.thumb_cache:
                        del self.thumb_cache[path]
            self.save_pins()
            self.reload()
        except Exception:
            pass

    def on_move_clicked(self, _):
        selected_paths = self.get_selected_paths()
        if not selected_paths and self.current_file:
            selected_paths = [self.current_file]
        if not selected_paths:
            return

        dialog = Gtk.FileDialog()
        dialog.select_folder(self, None, lambda d, r: self.on_folder_selected(d, r, selected_paths))

    def on_folder_selected(self, dialog, result, selected_paths):
        try:
            folder = dialog.select_folder_finish(result)
            if folder:
                dest_dir = folder.get_path()
                for path in selected_paths:
                    if os.path.exists(path):
                        filename = os.path.basename(path)
                        dest = unique_destination(dest_dir, filename)
                        shutil.move(path, dest)
                        if path in self.pinned:
                            self.pinned.remove(path)
                            self.pinned.add(dest)
                        if path in self.row_cache:
                            del self.row_cache[path]
                        if path in self.thumb_cache:
                            del self.thumb_cache[path]
                self.save_pins()
                self.reload()
        except Exception:
            pass

    def on_search_changed(self, _):
        self.apply_filter()

    def on_key_pressed(self, _, keyval, keycode, state):
        name = Gtk.accelerator_name(keyval, state)

        if name == "Escape":
            selected = self.listbox.get_selected_rows()
            if selected:
                self.listbox.unselect_all()
                return True
            else:
                self.close()
                return True

        if name == "<Control>a":
            self.listbox.set_selection_mode(Gtk.SelectionMode.MULTIPLE)
            self.listbox.select_all()
            return True

        if name == "<Control>Delete":
            self.on_delete_clicked(None)
            return True

        if name == "<Control>f":
            self.search.grab_focus()
            return True

        if name == "<Control>o":
            if self.has_tesseract and self.current_file:
                self.on_ocr_clicked(None)
            return True

        if name == "<Control>p":
            self.on_pin_clicked(None)
            return True

        if name == "i":
            self.info_btn.set_active(not self.info_btn.get_active())
            return True

        if name == "Return" or name == "KP_Enter":
            if self.current_file:
                self.on_edit_clicked(None)
            return True

        if name == "Up":
            self._navigate_list(-1)
            return True

        if name == "Down":
            self._navigate_list(1)
            return True

        return False

    def _navigate_list(self, direction):
        rows = []
        row = self.listbox.get_first_child()
        while row:
            if hasattr(row, "_file_path"):
                rows.append(row)
            row = row.get_next_sibling()
        if not rows:
            return

        selected = self.listbox.get_selected_rows()
        if not selected:
            row_index = 0 if direction > 0 else len(rows) - 1
        else:
            current_idx = rows.index(selected[0])
            row_index = max(0, min(len(rows) - 1, current_idx + direction))

        self.listbox.unselect_all()
        self.listbox.select_row(rows[row_index])
        rows[row_index].grab_focus()


class BlinkerManagerApp(Adw.Application):
    def __init__(self):
        super().__init__(application_id="org.fcosta.BlinkerManager")
        self.hold()
        self.connect("activate", self.on_activate)

    def on_activate(self, app):
        win = self.props.active_window or BlinkerManagerWindow(self)
        win.present()

    def do_startup(self):
        Adw.Application.do_startup(self)
        Adw.StyleManager.get_default().set_color_scheme(Adw.ColorScheme.PREFER_DARK)

        settings_action = Gio.SimpleAction.new("settings", None)
        settings_action.connect("activate", self.on_settings)
        self.add_action(settings_action)

        about_action = Gio.SimpleAction.new("about", None)
        about_action.connect("activate", self.on_about)
        self.add_action(about_action)

    def on_settings(self, *args):
        win = self.props.active_window
        if win:
            dialog = SettingsDialog(win)
            dialog.present()

    def on_about(self, *args):
        win = self.props.active_window
        if win:
            dialog = Adw.AboutWindow(transient_for=win)
            dialog.set_application_name("Blinker Manager")
            dialog.set_version("1.0.0")
            dialog.set_comments("Manage and edit screenshots")
            dialog.present()


def run():
    if dispatch_to_main("--blinker-manager"):
        return

    BlinkerManagerApp().run([])


if __name__ == "__main__":
    run()
