#!/usr/bin/env python3
"""Shared PipeWire/WirePlumber operations."""

from __future__ import annotations

import json

from .command import run


def channel_volume_percent(node: dict) -> float:
    channels = node.get("volume") or {}
    channel = channels.get("front-left") or next(iter(channels.values()), None)
    if not channel:
        return 0.0
    return float(channel.get("value_percent", "0%").rstrip("%"))


class AudioBackend:
    def list_devices(self) -> tuple[list[dict], list[dict], str, str]:
        sinks_result = run(["pactl", "-f", "json", "list", "sinks"])
        sources_result = run(["pactl", "-f", "json", "list", "sources"])
        sink_result = run(["pactl", "get-default-sink"])
        source_result = run(["pactl", "get-default-source"])
        sinks = json.loads(sinks_result.stdout) if sinks_result.ok else []
        sources = json.loads(sources_result.stdout) if sources_result.ok else []
        return (
            self._as_list(sinks),
            self._as_list(sources),
            sink_result.stdout.strip(),
            source_result.stdout.strip(),
        )

    @staticmethod
    def _as_list(value: object) -> list[dict]:
        if isinstance(value, dict):
            return [value]
        if isinstance(value, list):
            return value
        return []

    def get_default_volume(self, target: str) -> tuple[int, bool]:
        result = run(["wpctl", "get-volume", target])
        if not result.ok or "Volume:" not in result.stdout:
            return 0, False
        fields = result.stdout.split(":", 1)[1].strip().split()
        return round(float(fields[0]) * 100), "[MUTED]" in result.stdout

    def set_volume(self, target: str, percentage: float) -> None:
        run(["wpctl", "set-volume", target, f"{max(0, percentage) / 100:.3f}"], check=True)

    def toggle_mute(self, target: str) -> None:
        run(["wpctl", "set-mute", target, "toggle"], check=True)

    def set_default(self, node_id: int | str) -> None:
        run(["wpctl", "set-default", str(node_id)], check=True)

    def set_default_sink(self, name: str) -> None:
        run(["pactl", "set-default-sink", name], check=True)

    def set_default_source(self, name: str) -> None:
        run(["pactl", "set-default-source", name], check=True)
