#!/usr/bin/env python3
"""NetworkManager operations with BSSID-aware Wi-Fi identities."""

from __future__ import annotations

from dataclasses import dataclass

from .command import run


def parse_nmcli_terse(line: str) -> list[str]:
    fields: list[str] = []
    current: list[str] = []
    escaped = False
    for char in line:
        if escaped:
            current.append(char)
            escaped = False
        elif char == "\\":
            escaped = True
        elif char == ":":
            fields.append("".join(current))
            current = []
        else:
            current.append(char)
    if escaped:
        current.append("\\")
    fields.append("".join(current))
    return fields


@dataclass(frozen=True)
class WifiState:
    enabled: bool
    networks: tuple[dict, ...]


class NetworkBackend:
    def radio_enabled(self) -> bool:
        result = run(["nmcli", "radio", "wifi"], check=True)
        return result.stdout.strip() == "enabled"

    def set_radio(self, enabled: bool) -> None:
        run(["nmcli", "radio", "wifi", "on" if enabled else "off"], check=True)

    def active_status(self) -> tuple[bool, str]:
        enabled = self.radio_enabled()
        if not enabled:
            return False, "Disabled"
        result = run(
            ["nmcli", "--terse", "--escape", "yes", "--fields", "IN-USE,SSID", "device", "wifi"],
            timeout=8,
        )
        for line in result.stdout.splitlines():
            fields = parse_nmcli_terse(line)
            if len(fields) >= 2 and fields[0] == "*":
                return True, fields[1] or "Connected"
        return False, "Disconnected"

    def scan(self) -> WifiState:
        enabled = self.radio_enabled()
        if not enabled:
            return WifiState(False, ())
        # A rescan can legitimately be rate-limited; the cached AP list is still
        # useful, so only the list command is authoritative.
        run(["nmcli", "device", "wifi", "rescan"], timeout=10)
        result = run(
            [
                "nmcli",
                "--terse",
                "--escape",
                "yes",
                "--fields",
                "SSID,BSSID,SIGNAL,IN-USE,SECURITY,BARS",
                "device",
                "wifi",
                "list",
            ],
            timeout=12,
            check=True,
        )
        networks: dict[str, dict] = {}
        for line in result.stdout.splitlines():
            fields = parse_nmcli_terse(line)
            if len(fields) < 6 or not fields[0]:
                continue
            ssid, bssid, raw_signal, in_use, security, bars = fields[:6]
            try:
                signal = int(raw_signal)
            except ValueError:
                signal = 0
            key = bssid or f"{ssid}:{signal}"
            networks[key] = {
                "ssid": ssid,
                "bssid": bssid,
                "signal": signal,
                "active": in_use == "*",
                "security": security.strip(),
                "bars": bars,
            }
        ordered = sorted(
            networks.values(),
            key=lambda network: (
                not network["active"],
                network["ssid"].casefold(),
                -network["signal"],
                network["bssid"],
            ),
        )
        return WifiState(True, tuple(ordered))

    def saved_profiles(self) -> list[dict]:
        result = run(
            [
                "nmcli",
                "--terse",
                "--escape",
                "yes",
                "--fields",
                "NAME,UUID,802-11-wireless.ssid,802-11-wireless.bssid",
                "connection",
                "show",
            ],
            check=True,
        )
        profiles = []
        for line in result.stdout.splitlines():
            fields = parse_nmcli_terse(line)
            if len(fields) >= 4 and fields[2]:
                profiles.append(
                    {
                        "name": fields[0],
                        "uuid": fields[1],
                        "ssid": fields[2],
                        "bssid": fields[3],
                    }
                )
        return profiles

    def connect_saved(self, uuid: str) -> str:
        return run(["nmcli", "connection", "up", "uuid", uuid], timeout=35, check=True).stdout

    def connect_open(self, ssid: str, bssid: str) -> str:
        argv = ["nmcli", "device", "wifi", "connect", ssid]
        if bssid:
            argv.extend(("bssid", bssid))
        return run(argv, timeout=35, check=True).stdout

    def connect_personal(self, ssid: str, bssid: str, password: str) -> str:
        argv = ["nmcli", "--ask", "device", "wifi", "connect", ssid]
        if bssid:
            argv.extend(("bssid", bssid))
        return run(argv, input_text=f"{password}\n", timeout=35, check=True).stdout
