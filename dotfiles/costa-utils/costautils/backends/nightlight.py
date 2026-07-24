#!/usr/bin/env python3
"""Explicit manual night-light state for the singleton application."""

from __future__ import annotations

from .command import run


class NightLightBackend:
    def __init__(self, temperature: int = 4500):
        self.temperature = temperature
        self.enabled: bool | None = None

    def set_enabled(self, enabled: bool) -> bool:
        profile = f"temperature {self.temperature}" if enabled else "identity"
        run(["hyprctl", "hyprsunset", profile], check=True)
        self.enabled = enabled
        return enabled

    def toggle(self) -> bool:
        return self.set_enabled(not bool(self.enabled))

    def query(self) -> bool:
        """Initialize once; afterwards our state is authoritative."""
        if self.enabled is not None:
            return self.enabled
        result = run(["hyprctl", "hyprsunset", "temperature"])
        try:
            temperature = float(result.stdout.strip())
        except ValueError:
            temperature = 6500
        self.enabled = result.ok and temperature < 6000
        return self.enabled
