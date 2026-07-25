"""Installer-focused unit tests for the costa-utils CLI contract."""

from __future__ import annotations

import unittest
from pathlib import Path

REPOSITORY_ROOT = Path(__file__).resolve().parents[1]
COSTA_UTILS_ROOT = REPOSITORY_ROOT / "costa-utils"

KNOWN_TARGETS = {
    "--app-menu",
    "--runner",
    "--blinker",
    "--blinker-area",
    "--blinker-manager",
    "--clipper",
    "--power-menu",
    "--network-menu",
    "--bluetooth-menu",
    "--volume-menu",
    "--control-center",
    "--shutdown",
}

ALIASES = {
    "app-menu": "--app-menu",
    "appmenu": "--app-menu",
    "runner": "--runner",
    "blinker": "--blinker",
    "blinker-area": "--blinker-area",
    "blinker-manager": "--blinker-manager",
    "clipper": "--clipper",
    "power-menu": "--power-menu",
    "power": "--power-menu",
    "network-menu": "--network-menu",
    "network": "--network-menu",
    "bluetooth-menu": "--bluetooth-menu",
    "bluetooth": "--bluetooth-menu",
    "volume-menu": "--volume-menu",
    "volume": "--volume-menu",
    "control-center": "--control-center",
    "shutdown": "--shutdown",
}


def resolve_target(raw: str | None) -> str | None:
    if raw is None:
        return None
    value = raw.strip().lower().replace("_", "-")
    if not value:
        return None
    if value in KNOWN_TARGETS:
        return value
    if value.startswith("--"):
        return value if value in KNOWN_TARGETS else None
    return ALIASES.get(value)


def infer_target_from_argv0(argv0: str) -> str | None:
    return ALIASES.get(Path(argv0).name.lower().replace("_", "-"))


class DispatchTests(unittest.TestCase):
    def test_named_and_flag_targets_resolve(self):
        self.assertEqual(resolve_target("--clipper"), "--clipper")
        self.assertEqual(resolve_target("network"), "--network-menu")
        self.assertEqual(resolve_target("CONTROL_CENTER"), "--control-center")
        self.assertEqual(resolve_target("blinker-area"), "--blinker-area")
        self.assertEqual(resolve_target("shutdown"), "--shutdown")

    def test_unknown_target_is_rejected(self):
        self.assertIsNone(resolve_target("not-a-real-tool"))

    def test_target_can_be_inferred_from_symlink_name(self):
        self.assertEqual(
            infer_target_from_argv0("/home/test/.local/bin/blinker-manager"),
            "--blinker-manager",
        )


class CostaUtilsTreeTests(unittest.TestCase):
    def test_rust_workspace_is_present(self):
        self.assertTrue((COSTA_UTILS_ROOT / "Cargo.toml").is_file())
        self.assertTrue((COSTA_UTILS_ROOT / "crates" / "costa-bin").is_dir())
        self.assertTrue((COSTA_UTILS_ROOT / "crates" / "costa-core").is_dir())
        self.assertTrue((COSTA_UTILS_ROOT / "crates" / "costa-ui").is_dir())

    def test_desktop_assets_exist(self):
        self.assertTrue(
            (
                COSTA_UTILS_ROOT
                / "assets"
                / "applications"
                / "org.fcosta.CostaUtils.desktop"
            ).is_file()
        )
        self.assertTrue((COSTA_UTILS_ROOT / "assets" / "icons" / "costa_utils.svg").is_file())

    def test_cli_surface_documents_known_flags(self):
        target_rs = (COSTA_UTILS_ROOT / "crates" / "costa-core" / "src" / "target.rs").read_text()
        for flag in sorted(KNOWN_TARGETS):
            self.assertIn(flag, target_rs, f"missing CLI flag {flag} in target.rs")


if __name__ == "__main__":
    unittest.main()
