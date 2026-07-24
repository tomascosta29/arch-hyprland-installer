#!/usr/bin/env python3
"""XDG-aware locations shared by screenshot capture and management."""

from __future__ import annotations

import os
import re

DEFAULT_SCREENSHOT_SETTING = "~/Pictures/Screenshots"


def pictures_directory() -> str:
    config_home = os.environ.get("XDG_CONFIG_HOME", os.path.expanduser("~/.config"))
    user_dirs = os.path.join(config_home, "user-dirs.dirs")
    try:
        with open(user_dirs, encoding="utf-8") as stream:
            for line in stream:
                match = re.match(r'^XDG_PICTURES_DIR="(.*)"\s*$', line.strip())
                if match:
                    value = match.group(1).replace("$HOME", os.path.expanduser("~"))
                    return os.path.abspath(os.path.expandvars(os.path.expanduser(value)))
    except OSError:
        pass
    return os.path.expanduser("~/Pictures")


def screenshot_directory(setting: str | None = None) -> str:
    if not setting or setting == DEFAULT_SCREENSHOT_SETTING:
        return os.path.join(pictures_directory(), "Screenshots")
    return os.path.abspath(os.path.expandvars(os.path.expanduser(setting)))
