#!/usr/bin/env python3
import os
import signal
import sys

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Adw, Gio, GLib

# Add the current directory to sys.path to ensure modules can be found
sys.path.append(os.path.dirname(os.path.realpath(__file__)))

from costautils.app_menu import AppMenuWindow
from costautils.blinker import BlinkerLauncher
from costautils.blinker_manager import BlinkerManagerWindow, SettingsDialog
from costautils.cliphist_gtk import ClipWindow
from costautils.dispatch import infer_target_from_argv0, resolve_target
from costautils.power_menu import PowerWindow
from costautils.singleton import forward_command

USAGE = "Usage: costa-utils [--app-menu | --runner | --blinker | --blinker-manager | --clipper | --power-menu | --network-menu | --bluetooth-menu | --volume-menu | --control-center]"


def parse_target(argv):
    if len(argv) > 1:
        raw_target = argv[1]
        if raw_target in ("-h", "--help"):
            return "help", None, raw_target
        return "target", resolve_target(raw_target), raw_target

    inferred = infer_target_from_argv0(argv[0])
    if inferred:
        return "target", inferred, None

    return "none", None, None


class CostaUtilsApp(Adw.Application):
    def __init__(self, initial_target=None):
        super().__init__(
            application_id="org.fcosta.CostaUtils",
            flags=Gio.ApplicationFlags.FLAGS_NONE,
        )
        self.hold()

        self.initial_target = initial_target
        self.win_app_menu = None
        self.win_runner = None
        self.win_blinker = None
        self.win_blinker_manager = None
        self.win_clipper = None
        self.win_power = None
        self.win_network = None
        self.win_bluetooth = None
        self.win_volume = None
        self.win_control_center = None

        action = Gio.SimpleAction.new("activate-target", GLib.VariantType.new("s"))
        action.connect("activate", self._on_activate_target_action)
        self.add_action(action)

        settings_action = Gio.SimpleAction.new("settings", None)
        settings_action.connect("activate", self._on_settings_action)
        self.add_action(settings_action)

        about_action = Gio.SimpleAction.new("about", None)
        about_action.connect("activate", self._on_about_action)
        self.add_action(about_action)

    def _on_activate_target_action(self, _action, parameter):
        if parameter is None or not parameter.is_of_type(GLib.VariantType.new("s")):
            return
        target = parameter.get_string()
        if target:
            self.activate_target(target)

    def _on_settings_action(self, *_args):
        self.activate_blinker_manager()
        SettingsDialog(self.win_blinker_manager).present()

    def _on_about_action(self, *_args):
        about = Adw.AboutWindow(
            application_name="Costa Utils",
            application_icon="org.fcosta.CostaUtils",
            developers=["fcosta"],
            version="1.0.0",
            comments="Desktop utilities for the Arch Hyprland workstation.",
        )
        if self.props.active_window:
            about.set_transient_for(self.props.active_window)
        about.present()

    def do_activate(self):
        target = self.initial_target
        self.initial_target = None
        if target:
            self.activate_target(target)

    def activate_target(self, target):
        if target == "--app-menu":
            self.activate_app_menu()
        elif target == "--runner":
            self.activate_runner()
        elif target == "--blinker":
            self.activate_blinker()
        elif target == "--blinker-manager":
            self.activate_blinker_manager()
        elif target == "--clipper":
            self.activate_clipper()
        elif target == "--power-menu":
            self.activate_power()
        elif target == "--network-menu":
            self.activate_network()
        elif target == "--bluetooth-menu":
            self.activate_bluetooth()
        elif target == "--volume-menu":
            self.activate_volume()
        elif target == "--control-center":
            self.activate_control_center()

    def activate_app_menu(self):
        if not self.win_app_menu:
            self.win_app_menu = AppMenuWindow(self)
        self.win_app_menu.present()
        self.win_app_menu.search_entry.grab_focus()

    def activate_runner(self):
        if not self.win_runner:
            self.win_runner = AppMenuWindow(self, runner_mode=True)
        self.win_runner.present()
        self.win_runner.search_entry.grab_focus()

    def activate_blinker(self):
        if not self.win_blinker:
            self.win_blinker = BlinkerLauncher(self)
        self.win_blinker.present()

    def activate_blinker_manager(self):
        if not self.win_blinker_manager:
            self.win_blinker_manager = BlinkerManagerWindow(self)
        else:
            self.win_blinker_manager.refresh_screenshot_directory()
        self.win_blinker_manager.present()

    def activate_clipper(self):
        if not self.win_clipper:
            self.win_clipper = ClipWindow(self)
        else:
            self.win_clipper.reload()
        self.win_clipper.present()

    def activate_power(self):
        if not self.win_power:
            self.win_power = PowerWindow(self)
        self.win_power.present()

    def activate_network(self):
        if not self.win_network:
            from costautils.network_menu import NetworkWindow

            self.win_network = NetworkWindow(self)
        else:
            self.win_network.refresh_networks()
        self.win_network.present()

    def activate_bluetooth(self):
        if not self.win_bluetooth:
            from costautils.bluetooth_menu import BluetoothWindow

            self.win_bluetooth = BluetoothWindow(self)
        else:
            self.win_bluetooth.refresh_devices()
        self.win_bluetooth.present()

    def activate_volume(self):
        if not self.win_volume:
            from costautils.volume_menu import VolumeWindow

            self.win_volume = VolumeWindow(self)
        self.win_volume.refresh_audio_devices()
        self.win_volume.start_media_monitor()
        self.win_volume.present()

    def activate_control_center(self):
        if not self.win_control_center:
            from costautils.control_center import ControlCenterWindow

            self.win_control_center = ControlCenterWindow(self)
        self.win_control_center.refresh_states()
        self.win_control_center.start_media_monitor()
        self.win_control_center.present()


def main(argv):
    mode, target, raw_target = parse_target(argv)

    if mode == "help":
        print(USAGE)
        return 0

    if mode == "target" and target is None:
        print(f"Unknown argument: {raw_target}")
        print(USAGE)
        return 1

    if mode == "none":
        print(USAGE)
        return 0

    if forward_command(target):
        return 0

    app = CostaUtilsApp(initial_target=target)
    app.register()
    if app.get_is_remote():
        # Another instance won the bus name after our forward check.
        # It is now fully registered, so forward the target to it.
        return 0 if forward_command(target) else 1

    return app.run([argv[0]])


if __name__ == "__main__":
    signal.signal(signal.SIGINT, signal.SIG_DFL)
    sys.exit(main(sys.argv))
