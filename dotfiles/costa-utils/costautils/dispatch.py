#!/usr/bin/env python3
import os
import subprocess
import sys

COMMAND_TO_FLAG = {
    "--app-menu": "--app-menu",
    "--runner": "--runner",
    "--blinker": "--blinker",
    "--blinker-manager": "--blinker-manager",
    "--clipper": "--clipper",
    "--power-menu": "--power-menu",
    "--network-menu": "--network-menu",
    "--bluetooth-menu": "--bluetooth-menu",
    "--volume-menu": "--volume-menu",
    "--control-center": "--control-center",
    "--shutdown": "--shutdown",
}

NAME_TO_FLAG = {
    "app-menu": "--app-menu",
    "appmenu": "--app-menu",
    "runner": "--runner",
    "blinker": "--blinker",
    "blinker-manager": "--blinker-manager",
    "blinker_manager": "--blinker-manager",
    "clipper": "--clipper",
    "power-menu": "--power-menu",
    "power_menu": "--power-menu",
    "network-menu": "--network-menu",
    "network_menu": "--network-menu",
    "network": "--network-menu",
    "bluetooth-menu": "--bluetooth-menu",
    "bluetooth_menu": "--bluetooth-menu",
    "bluetooth": "--bluetooth-menu",
    "volume-menu": "--volume-menu",
    "volume_menu": "--volume-menu",
    "volume": "--volume-menu",
    "control-center": "--control-center",
    "control_center": "--control-center",
    "control": "--control-center",
    "shutdown": "--shutdown",
}


def resolve_target(value):
    if not value:
        return None

    if value in COMMAND_TO_FLAG:
        return COMMAND_TO_FLAG[value]

    return NAME_TO_FLAG.get(value.lower())


def infer_target_from_argv0(argv0):
    if not argv0:
        return None

    return resolve_target(os.path.basename(argv0))


def get_main_script_path():
    package_dir = os.path.dirname(os.path.realpath(__file__))
    return os.path.join(os.path.dirname(package_dir), "costa_utils.py")


def dispatch_to_main(target_flag):
    script_path = get_main_script_path()
    if not os.path.isfile(script_path):
        return False

    try:
        subprocess.Popen([sys.executable, script_path, target_flag])
    except OSError:
        return False

    return True
