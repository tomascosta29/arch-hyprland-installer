import sys
import tempfile
import unittest
from pathlib import Path

REPOSITORY_ROOT = Path(__file__).resolve().parents[1]
APP_ROOT = REPOSITORY_ROOT / "dotfiles" / "costa-utils"
sys.path.insert(0, str(APP_ROOT))

from costautils.blinker import unique_screenshot_path
from costautils.blinker_manager import unique_destination
from costautils.cliphist_gtk import clipboard_mime_type
from costautils.dispatch import infer_target_from_argv0, resolve_target
from costautils.network_menu import parse_nmcli_terse


class DispatchTests(unittest.TestCase):
    def test_named_and_flag_targets_resolve(self):
        self.assertEqual(resolve_target("--clipper"), "--clipper")
        self.assertEqual(resolve_target("network"), "--network-menu")
        self.assertEqual(resolve_target("CONTROL_CENTER"), "--control-center")

    def test_unknown_target_is_rejected(self):
        self.assertIsNone(resolve_target("not-a-real-tool"))

    def test_target_can_be_inferred_from_symlink_name(self):
        self.assertEqual(
            infer_target_from_argv0("/home/test/.local/bin/blinker-manager"),
            "--blinker-manager",
        )


class NetworkParsingTests(unittest.TestCase):
    def test_nmcli_escaped_fields(self):
        self.assertEqual(
            parse_nmcli_terse(r"Cafe\\Guest\:5G:87:yes:▂▄▆█"),
            [r"Cafe\Guest:5G", "87", "yes", "▂▄▆█"],
        )

    def test_nmcli_empty_field(self):
        self.assertEqual(parse_nmcli_terse(":0:no:"), ["", "0", "no", ""])


class ClipboardTests(unittest.TestCase):
    def test_common_mime_types_are_preserved(self):
        self.assertEqual(clipboard_mime_type(b"\x89PNG\r\n\x1a\n..."), "image/png")
        self.assertEqual(clipboard_mime_type(b"\xff\xd8\xff..."), "image/jpeg")
        self.assertEqual(clipboard_mime_type(b"GIF89a..."), "image/gif")
        self.assertEqual(
            clipboard_mime_type(b"RIFF\x00\x00\x00\x00WEBP..."),
            "image/webp",
        )
        self.assertEqual(
            clipboard_mime_type("hello".encode()),
            "text/plain;charset=utf-8",
        )
        self.assertEqual(
            clipboard_mime_type(b"\x00\xff\x00"),
            "application/octet-stream",
        )


class ScreenshotPathTests(unittest.TestCase):
    def test_capture_names_never_overwrite(self):
        with tempfile.TemporaryDirectory() as directory:
            first = unique_screenshot_path(directory, "Screenshot")
            Path(first).touch()
            second = unique_screenshot_path(directory, "Screenshot")
            self.assertEqual(Path(second).name, "Screenshot_1.png")

    def test_capture_pattern_cannot_escape_directory(self):
        with tempfile.TemporaryDirectory() as directory:
            result = unique_screenshot_path(directory, "../../outside")
            self.assertEqual(Path(result).parent, Path(directory))
            self.assertEqual(Path(result).name, "outside.png")

    def test_moves_never_overwrite(self):
        with tempfile.TemporaryDirectory() as directory:
            existing = Path(directory) / "capture.png"
            existing.touch()
            destination = unique_destination(directory, existing.name)
            self.assertEqual(Path(destination).name, "capture_1.png")


if __name__ == "__main__":
    unittest.main()
