#!/usr/bin/env python3
import os
import subprocess
import gi
import re
import json

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
gi.require_version("Gdk", "4.0")
from gi.repository import Adw, GLib, Gdk, Gtk, Pango, GdkPixbuf, Gio, GObject
from datetime import datetime

try:
    from .dispatch import dispatch_to_main
except ImportError:
    from dispatch import dispatch_to_main

VERSION = "1.0.0"
CONFIG_DIR = os.path.expanduser("~/.config/clipper")
PINS_FILE = os.path.join(CONFIG_DIR, "pins")
STATE_FILE = os.path.join(CONFIG_DIR, "state.json")

class ClipWindow(Adw.ApplicationWindow):
    def __init__(self, app):
        super().__init__(application=app, title="Clipper")
        
        # Load state (window size, etc.)
        self.state = self.load_state()
        self.set_default_size(self.state.get("width", 980), self.state.get("height", 620))

        self.lines = []
        self.filtered = []
        self.pinned = self.load_pins()
        self.current_line = None
        self.thumb_cache = {}
        self.data_cache = {}
        self._current_text_data = ""

        self.load_css()
        self.build_ui()
        
        # Setup real-time monitoring
        self.reload()
        self.setup_monitor()
        
        # Connect state saving
        self.connect("close-request", self.on_close_request)
        self.connect("notify::is-active", self.on_is_active_changed)

    def load_state(self):
        try:
            with open(STATE_FILE, "r") as f: return json.load(f)
        except: return {}

    def on_close_request(self, *args):
        width = self.get_width()
        height = self.get_height()
        with open(STATE_FILE, "w") as f:
            json.dump({"width": width, "height": height}, f)
        self.hide()
        return True

    def on_is_active_changed(self, window, pspec):
        if not self.get_active():
            self.hide()

    def build_ui(self):
        root = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        self.set_content(root)

        # --- Header Bar ---
        header = Adw.HeaderBar()
        root.append(header)

        # Sidebar Toggle
        self.sidebar_button = Gtk.ToggleButton(icon_name="view-sidebar-symbolic")
        self.sidebar_button.set_active(True)
        self.sidebar_button.set_tooltip_text("Toggle Sidebar")
        header.pack_start(self.sidebar_button)

        # Search Entry in Center
        self.search = Gtk.SearchEntry()
        self.search.set_placeholder_text("Search clipboard...")
        self.search.set_hexpand(True)
        self.search.set_max_width_chars(40)
        self.search.connect("search-changed", self.on_search_changed)
        header.set_title_widget(self.search)

        # Left: Filter Tabs
        filter_bin = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL)
        filter_bin.add_css_class("linked")
        self.btn_filter_all = Gtk.ToggleButton(label="All", active=True)
        self.btn_filter_text = Gtk.ToggleButton(label="Text")
        self.btn_filter_imgs = Gtk.ToggleButton(label="Images")
        self.btn_filter_text.set_group(self.btn_filter_all)
        self.btn_filter_imgs.set_group(self.btn_filter_all)
        for b in [self.btn_filter_all, self.btn_filter_text, self.btn_filter_imgs]:
            b.connect("toggled", self.on_filter_type_changed)
            filter_bin.append(b)
        header.pack_start(filter_bin)

        # Right: Menu & Actions
        menu_button = Gtk.MenuButton()
        menu_button.set_icon_name("open-menu-symbolic")
        header.pack_end(menu_button)

        # Action Menu
        menu = Gio.Menu.new()
        menu.append("About Clipper", "app.about")
        menu.append("Clear History", "win.wipe")
        menu_button.set_menu_model(menu)

        # --- Main Layout ---
        main_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
        main_box.set_margin_top(12); main_box.set_margin_bottom(12)
        main_box.set_margin_start(12); main_box.set_margin_end(12)
        main_box.set_vexpand(True)
        root.append(main_box)

        # Action Toolbar (Smart Actions)
        actions_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=6)
        actions_box.set_margin_bottom(4)
        main_box.append(actions_box)

        # Editor Group
        edit_group = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL)
        edit_group.add_css_class("linked")
        actions_box.append(edit_group)

        self.edit_btn = Gtk.ToggleButton(icon_name="document-edit-symbolic")
        self.edit_btn.set_tooltip_text("Edit (Ctrl+E)")
        self.edit_btn.connect("toggled", self.on_edit_toggled)
        edit_group.append(self.edit_btn)

        self.json_btn = Gtk.Button(icon_name="text-x-script-symbolic")
        self.json_btn.set_visible(False)
        self.json_btn.connect("clicked", self.on_json_clicked)
        edit_group.append(self.json_btn)

        # Action Group
        act_group = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL)
        act_group.add_css_class("linked")
        actions_box.append(act_group)

        self.open_path_btn = Gtk.Button(icon_name="folder-open-symbolic")
        self.open_path_btn.set_visible(False)
        self.open_path_btn.connect("clicked", self.on_open_path_clicked)
        act_group.append(self.open_path_btn)

        self.pin_btn = Gtk.Button(icon_name="user-bookmarks-symbolic")
        self.pin_btn.set_tooltip_text("Pin (Ctrl+P)")
        self.pin_btn.connect("clicked", self.on_pin_clicked)
        act_group.append(self.pin_btn)

        self.open_link_btn = Gtk.Button(icon_name="external-link-symbolic")
        self.open_link_btn.set_visible(False)
        self.open_link_btn.connect("clicked", self.on_open_link_clicked)
        act_group.append(self.open_link_btn)

        self.info_btn = Gtk.ToggleButton(icon_name="info-symbolic")
        self.info_btn.set_tooltip_text("Show Info (I)")
        self.info_btn.set_active(True)
        self.info_btn.connect("toggled", self.on_info_toggled)
        actions_box.append(self.info_btn)

        spacer = Gtk.Box(); spacer.set_hexpand(True); actions_box.append(spacer)

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
        self.sidebar_button.bind_property("active", self.split_view, "show-sidebar", GObject.BindingFlags.BIDIRECTIONAL)
        main_box.append(self.split_view)

        self.listbox = Gtk.ListBox()
        self.listbox.add_css_class("history-list")
        self.listbox.set_selection_mode(Gtk.SelectionMode.SINGLE)
        self.listbox.set_activate_on_single_click(False)
        self.listbox.connect("row-selected", self.on_row_selected)
        self.listbox.connect("row-activated", self.on_row_activated)
        
        self.last_selected_row = None
        listbox_controller = Gtk.GestureClick()
        listbox_controller.connect("released", self.on_listbox_click)
        self.listbox.add_controller(listbox_controller)

        left_scroll = Gtk.ScrolledWindow()
        left_scroll.set_child(self.listbox)
        left_scroll.set_min_content_width(320)
        self.split_view.set_sidebar(left_scroll)

        self.preview_stack = Gtk.Stack()
        self.preview_stack.set_transition_type(Gtk.StackTransitionType.CROSSFADE)
        
        self.preview_overlay = Gtk.Overlay()
        self.preview_overlay.set_child(self.preview_stack)
        
        # Metadata Pill
        self.info_pill = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=16)
        self.info_pill.add_css_class("preview-info-pill")
        self.info_pill.set_halign(Gtk.Align.CENTER)
        self.info_pill.set_valign(Gtk.Align.END)
        self.info_pill.set_margin_bottom(24)
        
        self.metadata_type = Gtk.Label(label="Text")
        self.metadata_type.add_css_class("info-label")
        self.info_pill.append(self.metadata_type)
        
        self.metadata_size = Gtk.Label(label="0 B")
        self.metadata_size.add_css_class("dim-label")
        self.info_pill.append(self.metadata_size)
        
        self.preview_overlay.add_overlay(self.info_pill)

        preview_frame = Gtk.Frame()
        preview_frame.add_css_class("preview-pane")
        preview_frame.set_child(self.preview_overlay)
        self.split_view.set_content(preview_frame)

        # Selection Bar
        self.selection_bar = Gtk.ActionBar()
        self.selection_bar.set_revealed(False)
        
        del_btn = Gtk.Button(icon_name="user-trash-symbolic")
        del_btn.add_css_class("destructive-action")
        del_btn.connect("clicked", self.on_delete_clicked)
        self.selection_bar.pack_start(del_btn)
        
        self.sel_label = Gtk.Label()
        self.selection_bar.set_center_widget(self.sel_label)
        
        close_sel = Gtk.Button(icon_name="window-close-symbolic")
        close_sel.add_css_class("flat")
        close_sel.connect("clicked", lambda _: self.listbox.unselect_all())
        self.selection_bar.pack_end(close_sel)
        
        main_box.append(self.selection_bar)

        # ... (Image, Text, Color, Empty states are built the same way as before) ...
        self.preview_image = Gtk.Picture(); self.preview_image.set_can_shrink(True)
        self.preview_image.set_content_fit(Gtk.ContentFit.CONTAIN)
        drag_source = Gtk.DragSource.new()
        drag_source.connect("prepare", self.on_drag_prepare)
        self.preview_image.add_controller(drag_source)
        img_box = Gtk.ScrolledWindow(); img_box.set_child(self.preview_image)

        self.preview_text = Gtk.TextView(); self.preview_text.add_css_class("preview-text")
        self.preview_text.set_monospace(True); self.preview_text.set_wrap_mode(Gtk.WrapMode.WORD_CHAR)
        self.preview_text.set_editable(False)
        txt_box = Gtk.ScrolledWindow(); txt_box.set_child(self.preview_text)

        self.color_preview_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=20)
        self.color_preview_box.set_valign(Gtk.Align.CENTER); self.color_preview_box.set_halign(Gtk.Align.CENTER)
        self.color_swatch = Gtk.Frame(); self.color_swatch.set_size_request(200, 200); self.color_swatch.add_css_class("color-swatch")
        self.color_preview_box.append(self.color_swatch)
        self.color_label = Gtk.Label(); self.color_label.add_css_class("color-label"); self.color_preview_box.append(self.color_label)

        self.empty_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=12); self.empty_box.set_valign(Gtk.Align.CENTER)
        self.empty_icon = Gtk.Image.new_from_icon_name("edit-find-symbolic"); self.empty_icon.set_pixel_size(64); self.empty_icon.add_css_class("dim-label"); self.empty_box.append(self.empty_icon)
        self.preview_label = Gtk.Label(label="Select a clipboard item"); self.preview_label.add_css_class("dim-label"); self.empty_box.append(self.preview_label)

        self.preview_stack.add_titled(self.empty_box, "empty", "Empty")
        self.preview_stack.add_titled(txt_box, "text", "Text")
        self.preview_stack.add_titled(img_box, "image", "Image")
        self.preview_stack.add_titled(self.color_preview_box, "color", "Color")

        # Global Key Handler
        ctrl = Gtk.EventControllerKey()
        ctrl.set_propagation_phase(Gtk.PropagationPhase.CAPTURE)
        ctrl.connect("key-pressed", self.on_key_pressed)
        self.add_controller(ctrl)

        # Actions for the menu
        action_wipe = Gio.SimpleAction.new("wipe", None)
        action_wipe.connect("activate", self.on_wipe_clicked)
        self.add_action(action_wipe)

    def load_css(self):
        css = b"""
        .clip-root { background: @window_bg_color; }
        .history-list row { margin: 2px 0; border-radius: 12px; }
        .history-list row:selected { background: alpha(@accent_bg_color, 0.25); }
        .history-row { padding: 8px 10px; }
        .thumb-frame { 
            min-width: 42px; min-height: 42px; border-radius: 10px; 
            background: alpha(@view_fg_color, 0.08); border: 1px solid alpha(@view_fg_color, 0.1);
        }
        .preview-pane { border-radius: 14px; padding: 8px; background: alpha(@view_fg_color, 0.04); }
        .preview-text { padding: 12px; font-size: 11pt; background: transparent; }
        .preview-text.editable { background: @view_bg_color; }
        .shortcut-badge { font-size: 0.7em; opacity: 0.5; font-weight: bold; margin-right: 4px; }
        .pinned-badge { color: @warning_color; font-size: 1.2em; margin-left: 4px; }
        .color-swatch { border-radius: 20px; border: 4px solid white; box-shadow: 0 4px 12px rgba(0,0,0,0.2); }
        .color-label { font-size: 1.4em; font-weight: bold; font-family: monospace; }
        .dim-label { opacity: 0.4; }
        .preview-info-pill {
            background: alpha(@window_bg_color, 0.85);
            padding: 8px 18px;
            border-radius: 24px;
            border: 1px solid alpha(@view_fg_color, 0.1);
            box-shadow: 0 4px 12px alpha(black, 0.15);
        }
        .info-label {
            font-weight: bold;
            color: @view_fg_color;
        }
        """
        provider = Gtk.CssProvider()
        provider.load_from_data(css)
        Gtk.StyleContext.add_provider_for_display(Gdk.Display.get_default(), provider, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION)

    def setup_monitor(self):
        cache_dir = os.path.expanduser("~/.cache/cliphist")
        if os.path.exists(cache_dir):
            gio_file = Gio.File.new_for_path(cache_dir)
            self.monitor = gio_file.monitor_directory(Gio.FileMonitorFlags.NONE, None)
            self.monitor.connect("changed", self.on_cache_changed)

    def on_cache_changed(self, monitor, file, other_file, event_type):
        if event_type in (Gio.FileMonitorEvent.CHANGED, Gio.FileMonitorEvent.CREATED, Gio.FileMonitorEvent.CHANGES_DONE_HINT):
            if hasattr(self, "_reload_timeout_id") and self._reload_timeout_id: GLib.source_remove(self._reload_timeout_id)
            self._reload_timeout_id = GLib.timeout_add(300, self.reload_from_monitor)

    def reload_from_monitor(self):
        self._reload_timeout_id = None; self.reload(); return False

    def load_pins(self):
        if not os.path.exists(PINS_FILE): return set()
        try:
            with open(PINS_FILE, "r") as f: return set(line.strip() for line in f if line.strip())
        except Exception: return set()

    def save_pins(self):
        os.makedirs(CONFIG_DIR, exist_ok=True)
        with open(PINS_FILE, "w") as f:
            for pin in self.pinned: f.write(f"{pin}\n")

    def fuzzy_match(self, query, text):
        query = query.lower(); text = text.lower()
        if not query: return True
        it = iter(text)
        return all(c in it for c in query)

    def run_cliphist(self, args, input_text=None):
        try:
            proc = subprocess.run(["cliphist"] + args, input=(input_text.encode("utf-8") if input_text else None),
                                  stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False, timeout=3)
            return proc.stdout if proc.returncode == 0 else b""
        except Exception: return b""

    def reload(self):
        raw = self.run_cliphist(["list"]).decode("utf-8", "replace")
        self.lines = [line for line in raw.splitlines() if line.strip()]
        self.apply_filter()

    def on_filter_type_changed(self, _): self.apply_filter()

    def apply_filter(self):
        query = self.search.get_text().strip()
        f_text = self.btn_filter_text.get_active()
        f_imgs = self.btn_filter_imgs.get_active()

        filtered_list = self.lines
        if f_text: filtered_list = [l for l in self.lines if "[[ binary data" not in l]
        elif f_imgs: filtered_list = [l for l in self.lines if "[[ binary data" in l]

        if query: self.filtered = [l for l in filtered_list if self.fuzzy_match(query, l)]
        else: self.filtered = list(filtered_list)

        self.filtered.sort(key=lambda l: l.split("\t", 1)[0] not in self.pinned)

        while (row := self.listbox.get_first_child()): self.listbox.remove(row)
        if not self.filtered:
            self.preview_label.set_label("No results found"); self.preview_stack.set_visible_child_name("empty")
        else:
            if self.preview_label.get_label() == "No results found": self.preview_label.set_label("Select a clipboard item")

        rows_to_load = []
        for i, line in enumerate(self.filtered[:100]):
            row = self.make_row(line, i + 1)
            self.listbox.append(row)
            if line in self.thumb_cache:
                if self.thumb_cache[line]: row._thumb.set_from_paintable(self.thumb_cache[line])
            else: rows_to_load.append(line)
        if rows_to_load: GLib.idle_add(self.load_thumbs_idle, rows_to_load)

    def make_row(self, line, index):
        row = Gtk.ListBoxRow(); row._clip_line = line
        line_id = line.split("\t", 1)[0]
        box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=10)
        box.add_css_class("history-row")
        row.set_child(box)

        icon_name = "text-x-generic-symbolic"
        if "[[ binary data" in line:
            icon_name = "image-x-generic-symbolic" if any(x in line.lower() for x in ["png", "jpg", "jpeg", "webp"]) else "package-x-generic-symbolic"

        thumb = Gtk.Image.new_from_icon_name(icon_name); thumb.set_pixel_size(26)
        row._thumb = thumb
        thumb_frame = Gtk.Frame(); thumb_frame.add_css_class("thumb-frame"); thumb_frame.set_child(thumb)
        box.append(thumb_frame)

        content_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=2); box.append(content_box)
        top_line = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=6); content_box.append(top_line)

        if index <= 9:
            badge = Gtk.Label(label=f"ALT+{index}"); badge.add_css_class("shortcut-badge"); top_line.append(badge)
        if line_id in self.pinned:
            star = Gtk.Image.new_from_icon_name("emblem-favorite-symbolic"); star.add_css_class("pinned-badge"); top_line.append(star)

        display_text = line.split("\t", 1)[1].strip() if "\t" in line else line
        lbl = Gtk.Label(label=display_text); lbl.set_xalign(0.0); lbl.set_ellipsize(Pango.EllipsizeMode.END)
        lbl.set_max_width_chars(60); content_box.append(lbl)
        return row

    def on_listbox_click(self, controller, n_press, x, y):
        event = controller.get_current_event()
        state = event.get_modifier_state() if event else 0
        row = self.listbox.get_row_at_y(y)
        if not row: return False
            
        if state & Gdk.ModifierType.SHIFT_MASK and self.last_selected_row:
            self.listbox.set_selection_mode(Gtk.SelectionMode.MULTIPLE)
            self.listbox.unselect_all()
            rows = []
            child = self.listbox.get_first_child()
            while child: rows.append(child); child = child.get_next_sibling()
            try:
                start, end = rows.index(self.last_selected_row), rows.index(row)
                for i in range(min(start, end), max(start, end) + 1): self.listbox.select_row(rows[i])
            except: pass
        elif state & Gdk.ModifierType.CONTROL_MASK:
            self.listbox.set_selection_mode(Gtk.SelectionMode.MULTIPLE)
            if row.is_selected(): self.listbox.unselect_row(row)
            else: self.listbox.select_row(row)
            self.last_selected_row = row
        else:
            self.listbox.set_selection_mode(Gtk.SelectionMode.SINGLE)
            self.last_selected_row = row
        return False

    def on_drag_prepare(self, source, x, y):
        if not self.current_line: return None
        data = self.decode_line(self.current_line)
        texture = self.try_make_texture(data)
        if not texture: return None
        return Gdk.ContentProvider.new_for_value(texture)

    def on_row_selected(self, _, row):
        selected_rows = self.listbox.get_selected_rows()
        count = len(selected_rows)
        
        self.selection_bar.set_revealed(count > 1)
        if count > 1:
            self.sel_label.set_label(f"{count} items selected")
            self.info_pill.set_visible(False)
            return
        
        # Reset to single selection logic
        if not row: return
        self.current_line = row._clip_line
        self.edit_btn.set_active(False)
        if hasattr(self, "_select_timeout_id") and self._select_timeout_id: GLib.source_remove(self._select_timeout_id)
        self._select_timeout_id = GLib.timeout_add(100, self.deferred_row_selected, row._clip_line)

    def deferred_row_selected(self, line):
        self._select_timeout_id = None
        if self.current_line != line: return False
        data = self.decode_line(line)
        self.show_preview_from_data(data)
        return False

    def on_info_toggled(self, btn):
        self.info_pill.set_visible(btn.get_active())

    def show_preview_from_data(self, data):
        if not data:
            self.show_preview_error("No data available")
            self.open_link_btn.set_visible(False); self.json_btn.set_visible(False); self.open_path_btn.set_visible(False)
            self.info_pill.set_visible(False)
            return
        
        self.info_pill.set_visible(self.info_btn.get_active())
        size_str = self._format_size(len(data))
        self.metadata_size.set_label(size_str)
        
        texture = self.try_make_texture(data)
        if texture:
            self.preview_image.set_paintable(texture)
            self.preview_stack.set_visible_child_name("image")
            self.open_link_btn.set_visible(False); self.json_btn.set_visible(False); self.open_path_btn.set_visible(False)
            self.metadata_type.set_label("Image")
            
            try:
                w, h = texture.get_width(), texture.get_height()
                self.metadata_type.set_label(f"Image {w}x{h}")
            except: pass
        else:
            text = data.decode("utf-8", "replace").replace("\x00", "").strip()
            self._current_text_data = text
            self.metadata_type.set_label("Text")
            
            color_match = re.match(r"^#(?:[0-9a-fA-F]{3}){1,2}$", text)
            if color_match:
                # ... (Color handling) ...
                provider = Gtk.CssProvider()
                provider.load_from_data(f".color-swatch {{ background-color: {text}; }}".encode())
                self.color_swatch.get_style_context().add_provider(provider, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION)
                self.color_label.set_label(text.upper())
                self.preview_stack.set_visible_child_name("color")
                self.open_link_btn.set_visible(False); self.json_btn.set_visible(False); self.open_path_btn.set_visible(False)
                self.metadata_type.set_label("Color")
            else:
                self.open_link_btn.set_visible(text.startswith(("http", "www.")))
                try: json.loads(text); self.json_btn.set_visible(True); self.metadata_type.set_label("JSON")
                except: self.json_btn.set_visible(False)
                is_path = os.path.exists(os.path.expanduser(text))
                self.open_path_btn.set_visible(is_path)
                buf = self.preview_text.get_buffer()
                buf.set_text(text[:30000], -1)
                self.preview_stack.set_visible_child_name("text")

    def _format_size(self, size):
        for unit in ['B', 'KB', 'MB']:
            if size < 1024: return f"{size:.1f} {unit}"
            size /= 1024
        return f"{size:.1f} GB"

    def on_edit_toggled(self, btn):
        editable = btn.get_active()
        self.preview_text.set_editable(editable)
        if editable: self.preview_text.add_css_class("editable")
        else: self.preview_text.remove_css_class("editable")

    def on_json_clicked(self, _):
        try:
            obj = json.loads(self._current_text_data); pretty = json.dumps(obj, indent=2)
            self._current_text_data = pretty; self.preview_text.get_buffer().set_text(pretty, -1)
        except: pass

    def on_open_path_clicked(self, _):
        path = os.path.expanduser(self._current_text_data.strip())
        if os.path.isfile(path): path = os.path.dirname(path)
        subprocess.run(["xdg-open", path], check=False)

    def on_wipe_clicked(self, *args):
        dialog = Gtk.AlertDialog(message="Clear all clipboard history?", detail="Pinned items will be preserved.")
        dialog.set_buttons(["Cancel", "Wipe All"])
        dialog.set_default_button(1); dialog.choose(self, None, self.on_wipe_confirmed)

    def on_wipe_confirmed(self, dialog, result):
        if dialog.choose_finish(result) == 1:
            self.run_cliphist(["wipe"]); self.reload()

    def on_open_link_clicked(self, _):
        url = self._current_text_data.strip()
        if url.startswith("www."): url = "http://" + url
        subprocess.run(["xdg-open", url], check=False)

    def on_pin_clicked(self, _):
        selected = self.listbox.get_selected_rows()
        if not selected: return
        
        # Batch pin
        if len(selected) > 1:
            for row in selected:
                line_id = row._clip_line.split("\t", 1)[0]
                if line_id in self.pinned: self.pinned.remove(line_id)
                else: self.pinned.add(line_id)
            self.save_pins(); self.apply_filter()
            return
            
        # Single pin
        if not self.current_line: return
        line_id = self.current_line.split("\t", 1)[0]
        if line_id in self.pinned: self.pinned.remove(line_id)
        else: self.pinned.add(line_id)
        self.save_pins(); self.apply_filter()

    def on_copy_clicked(self, _): self.copy_current_and_close(close=False)

    def on_row_activated(self, _, row):
        self.current_line = row._clip_line; self.copy_current_and_close(close=True)

    def copy_current_and_close(self, close=True):
        if not self.current_line: return
        if self.edit_btn.get_active():
            buf = self.preview_text.get_buffer()
            text = buf.get_text(buf.get_start_iter(), buf.get_end_iter(), True)
            subprocess.run(["wl-copy"], input=text.encode("utf-8"), check=False)
        else:
            data = self.decode_line(self.current_line)
            subprocess.run(["wl-copy"], input=data, check=False)
        if close: self.close()

    def on_delete_clicked(self, _):
        selected = self.listbox.get_selected_rows()
        if not selected: return
        
        # Batch delete
        if len(selected) > 1:
            for row in selected:
                line_id = row._clip_line.split("\t", 1)[0]
                if line_id in self.pinned: self.pinned.remove(line_id)
                self.run_cliphist(["delete"], input_text=row._clip_line)
            self.save_pins(); self.reload()
            return

        # Single delete
        if not self.current_line: return
        line_id = self.current_line.split("\t", 1)[0]
        if line_id in self.pinned: self.pinned.remove(line_id); self.save_pins()
        self.run_cliphist(["delete"], input_text=self.current_line)
        self.current_line = None; self.reload()

    def load_thumbs_idle(self, lines):
        if not lines: return False
        line = lines.pop(0); data = self.decode_line(line)
        if data: self.update_row_thumb_from_data(line, data)
        return len(lines) > 0

    def update_row_thumb_from_data(self, line, data):
        texture = self.try_make_texture(data); self.thumb_cache[line] = texture
        if not texture: return
        row = self.listbox.get_first_child()
        while row:
            if getattr(row, "_clip_line", None) == line: row._thumb.set_from_paintable(texture); break
            row = row.get_next_sibling()

    def decode_line(self, line):
        if line in self.data_cache: return self.data_cache[line]
        data = self.run_cliphist(["decode"], input_text=line)
        if data: self.data_cache[line] = data
        return data

    def try_make_texture(self, data):
        if not (data.startswith(b"\x89PNG") or data.startswith(b"\xff\xd8")): return None
        try:
            loader = GdkPixbuf.PixbufLoader(); loader.write(data); loader.close()
            pix = loader.get_pixbuf()
            return Gdk.Texture.new_for_pixbuf(pix) if pix else None
        except Exception: return None

    def on_search_changed(self, _): self.apply_filter()

    def show_preview_error(self, message):
        self.preview_label.set_label(message); self.preview_stack.set_visible_child_name("empty")

    def on_key_pressed(self, _, keyval, keycode, state):
        name = Gtk.accelerator_name(keyval, state)
        if name in ("Escape", "<Control>w", "<Control>q"):
            if self.listbox.get_selected_rows(): self.listbox.unselect_all()
            else: self.close()
            return True
        if name == "<Control>a":
            self.listbox.set_selection_mode(Gtk.SelectionMode.MULTIPLE)
            self.listbox.select_all()
            return True
        if name == "<Control>f": self.search.grab_focus(); return True
        if name == "<Control>p": self.on_pin_clicked(None); return True
        if name == "<Control>e": self.edit_btn.set_active(not self.edit_btn.get_active()); return True
        if name == "<Control>Delete": self.on_delete_clicked(None); return True
        if name == "i": self.info_btn.set_active(not self.info_btn.get_active()); return True
        if "Alt" in name:
            num = name.replace("<Alt>", "")
            if num.isdigit():
                idx = int(num) - 1
                if 0 <= idx < len(self.filtered):
                    self.current_line = self.filtered[idx]
                    self.copy_current_and_close(close=True); return True
        if name == "<Shift>Return": self.copy_current_and_close(close=False); return True
        if name in ("Return", "KP_Enter"): self.copy_current_and_close(close=True); return True
        return False

class ClipApp(Adw.Application):
    def __init__(self):
        super().__init__(application_id="org.fcosta.Clipper", flags=Gio.ApplicationFlags.FLAGS_NONE)
        self.hold()

    def do_activate(self):
        win = self.props.active_window or ClipWindow(self)
        win.present()

    def do_startup(self):
        Adw.Application.do_startup(self)
        
        # Set dark theme preference correctly
        Adw.StyleManager.get_default().set_color_scheme(Adw.ColorScheme.PREFER_DARK)
        
        # Check for cliphist dependency
        if subprocess.run(["which", "cliphist"], capture_output=True).returncode != 0:
            print("Error: cliphist is not installed.")
            self.quit()

        # Build App Menu Actions
        action_about = Gio.SimpleAction.new("about", None)
        action_about.connect("activate", self.on_about_clicked)
        self.add_action(action_about)

    def on_about_clicked(self, *args):
        about = Adw.AboutWindow(
            application_name="Clipper",
            application_icon="org.fcosta.Clipper",
            developers=["fcosta"],
            version=VERSION,
            website="https://github.com/fcosta/clipper",
            issue_url="https://github.com/fcosta/clipper/issues",
            copyright="© 2026 fcosta"
        )
        about.set_transient_for(self.props.active_window)
        about.present()

def run():
    if dispatch_to_main("--clipper"):
        return

    ClipApp().run([])

if __name__ == "__main__":
    run()
