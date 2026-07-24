#!/usr/bin/env python3
import gi

from gi.repository import GLib, Gio


BUS_NAME = "org.fcosta.CostaUtils"
OBJECT_PATH = "/org/fcosta/CostaUtils"
APP_INTERFACE = "org.freedesktop.Application"
ACTION_NAME = "activate-target"
CALL_TIMEOUT_MS = 500


def _get_session_bus():
    try:
        return Gio.bus_get_sync(Gio.BusType.SESSION, None)
    except GLib.Error:
        return None


def forward_command(target):
    connection = _get_session_bus()
    if connection is None:
        return False

    try:
        connection.call_sync(
            BUS_NAME,
            OBJECT_PATH,
            APP_INTERFACE,
            "ActivateAction",
            GLib.Variant(
                "(sava{sv})",
                (ACTION_NAME, [GLib.Variant("s", target)], {}),
            ),
            None,
            Gio.DBusCallFlags.NONE,
            CALL_TIMEOUT_MS,
        )
        return True
    except GLib.Error:
        return False
