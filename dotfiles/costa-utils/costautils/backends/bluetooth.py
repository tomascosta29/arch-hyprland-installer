#!/usr/bin/env python3
"""BlueZ state and operations without assuming a particular adapter path."""

from __future__ import annotations

import logging
from dataclasses import dataclass

from gi.repository import Gio, GLib

LOGGER = logging.getLogger(__name__)
BLUEZ = "org.bluez"


@dataclass(frozen=True)
class BluetoothState:
    adapter_path: str | None
    powered: bool
    discovering: bool
    devices: tuple[dict, ...]


class BluetoothBackend:
    def __init__(self):
        try:
            self.bus = Gio.bus_get_sync(Gio.BusType.SYSTEM, None)
        except GLib.Error:
            LOGGER.exception("system D-Bus is unavailable")
            self.bus = None
        self._subscribers: dict[object, object] = {}
        self._publish_timeout: int | None = None
        self._signal_id = None
        if self.bus is not None:
            self._signal_id = self.bus.signal_subscribe(
                BLUEZ,
                "org.freedesktop.DBus.Properties",
                "PropertiesChanged",
                None,
                None,
                Gio.DBusSignalFlags.NONE,
                self._on_properties_changed,
            )

    def subscribe(self, owner: object, callback) -> None:
        self._subscribers[owner] = callback

    def unsubscribe(self, owner: object) -> None:
        self._subscribers.pop(owner, None)

    def _on_properties_changed(self, *_args) -> None:
        if self._publish_timeout is None:
            self._publish_timeout = GLib.timeout_add(250, self._publish_changes)

    def _publish_changes(self) -> bool:
        self._publish_timeout = None
        for callback in tuple(self._subscribers.values()):
            callback()
        return GLib.SOURCE_REMOVE

    def query(self) -> BluetoothState:
        if self.bus is None:
            raise RuntimeError("system D-Bus is unavailable")
        reply = self.bus.call_sync(
            BLUEZ,
            "/",
            "org.freedesktop.DBus.ObjectManager",
            "GetManagedObjects",
            None,
            None,
            Gio.DBusCallFlags.NONE,
            5000,
            None,
        )
        objects = reply.unpack()[0]
        adapters: list[tuple[str, dict]] = []
        devices: list[dict] = []
        for path, interfaces in objects.items():
            adapter = interfaces.get("org.bluez.Adapter1")
            if adapter is not None:
                adapters.append((path, adapter))
            device = interfaces.get("org.bluez.Device1")
            if device is not None:
                devices.append(
                    {
                        "path": path,
                        "name": device.get("Alias", device.get("Name", "Unknown Device")),
                        "address": device.get("Address", ""),
                        "connected": bool(device.get("Connected", False)),
                        "paired": bool(device.get("Paired", False)),
                        "trusted": bool(device.get("Trusted", False)),
                        "icon": device.get("Icon", "bluetooth-active-symbolic"),
                        "adapter": device.get("Adapter"),
                    }
                )
        adapters.sort(key=lambda item: item[0])
        adapter_path, properties = adapters[0] if adapters else (None, {})
        if adapter_path:
            devices = [device for device in devices if device["adapter"] == adapter_path]
        devices.sort(
            key=lambda device: (
                not device["connected"],
                not device["paired"],
                device["name"].lower(),
                device["address"],
            )
        )
        return BluetoothState(
            adapter_path=adapter_path,
            powered=bool(properties.get("Powered", False)),
            discovering=bool(properties.get("Discovering", False)),
            devices=tuple(devices),
        )

    def set_power(self, adapter_path: str, powered: bool) -> None:
        self._set_property(
            adapter_path, "org.bluez.Adapter1", "Powered", GLib.Variant("b", powered)
        )

    def start_discovery(self, adapter_path: str) -> None:
        try:
            self._call(adapter_path, "org.bluez.Adapter1", "StartDiscovery", timeout=5000)
        except GLib.Error as error:
            if "InProgress" not in error.message:
                raise

    def stop_discovery(self, adapter_path: str | None) -> None:
        if not adapter_path:
            return
        try:
            self._call(adapter_path, "org.bluez.Adapter1", "StopDiscovery", timeout=5000)
        except GLib.Error as error:
            if "NotReady" not in error.message and "Failed" not in error.message:
                raise

    def connect(self, device_path: str, paired: bool) -> None:
        if not paired:
            self._call(device_path, "org.bluez.Device1", "Pair", timeout=60000)
        self._set_property(
            device_path,
            "org.bluez.Device1",
            "Trusted",
            GLib.Variant("b", True),
        )
        self._call(device_path, "org.bluez.Device1", "Connect", timeout=30000)

    def disconnect(self, device_path: str) -> None:
        self._call(device_path, "org.bluez.Device1", "Disconnect", timeout=15000)

    def cancel_pairing(self, device_path: str) -> None:
        self._call(device_path, "org.bluez.Device1", "CancelPairing", timeout=5000)

    def remove(self, adapter_path: str, device_path: str) -> None:
        self._call(
            adapter_path,
            "org.bluez.Adapter1",
            "RemoveDevice",
            GLib.Variant("(o)", (device_path,)),
            timeout=10000,
        )

    def _set_property(
        self,
        path: str,
        interface: str,
        name: str,
        value: GLib.Variant,
    ) -> None:
        if self.bus is None:
            raise RuntimeError("system D-Bus is unavailable")
        self.bus.call_sync(
            BLUEZ,
            path,
            "org.freedesktop.DBus.Properties",
            "Set",
            GLib.Variant("(ssv)", (interface, name, value)),
            None,
            Gio.DBusCallFlags.NONE,
            5000,
            None,
        )

    def _call(
        self,
        path: str,
        interface: str,
        method: str,
        parameters: GLib.Variant | None = None,
        *,
        timeout: int,
    ) -> None:
        if self.bus is None:
            raise RuntimeError("system D-Bus is unavailable")
        self.bus.call_sync(
            BLUEZ,
            path,
            interface,
            method,
            parameters,
            None,
            Gio.DBusCallFlags.NONE,
            timeout,
            None,
        )

    def close(self) -> None:
        if self._publish_timeout is not None:
            GLib.source_remove(self._publish_timeout)
            self._publish_timeout = None
        self._subscribers.clear()
        if self.bus is not None and self._signal_id is not None:
            self.bus.signal_unsubscribe(self._signal_id)
