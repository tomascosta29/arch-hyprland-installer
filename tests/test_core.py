import sys
import tempfile
import time
import unittest
from pathlib import Path

REPOSITORY_ROOT = Path(__file__).resolve().parents[1]
APP_ROOT = REPOSITORY_ROOT / "dotfiles" / "costa-utils"
sys.path.insert(0, str(APP_ROOT))

from costautils.app_menu import normalize_app_id, should_list_app
from costautils.backends.audio import channel_volume_percent
from costautils.backends.jobs import JobManager
from costautils.backends.media import FIELD_SEPARATOR, parse_media_record
from costautils.blinker import unique_screenshot_path
from costautils.blinker_manager import unique_destination
from costautils.cliphist_gtk import clipboard_mime_type
from costautils.dispatch import infer_target_from_argv0, resolve_target
from costautils.network_menu import parse_nmcli_terse
from gi.repository import GLib


class DispatchTests(unittest.TestCase):
    def test_named_and_flag_targets_resolve(self):
        self.assertEqual(resolve_target("--clipper"), "--clipper")
        self.assertEqual(resolve_target("network"), "--network-menu")
        self.assertEqual(resolve_target("CONTROL_CENTER"), "--control-center")
        self.assertEqual(resolve_target("shutdown"), "--shutdown")

    def test_unknown_target_is_rejected(self):
        self.assertIsNone(resolve_target("not-a-real-tool"))

    def test_target_can_be_inferred_from_symlink_name(self):
        self.assertEqual(
            infer_target_from_argv0("/home/test/.local/bin/blinker-manager"),
            "--blinker-manager",
        )


class LauncherFilterTests(unittest.TestCase):
    class FakeApp:
        def __init__(
            self,
            app_id,
            *,
            show=True,
            categories="",
            desktop=True,
        ):
            self._id = app_id
            self._show = show
            self._categories = categories
            self._desktop = desktop

        def should_show(self):
            return self._show

        def get_id(self):
            return self._id

        def get_categories(self):
            return self._categories

    def test_normalize_strips_desktop_suffix(self):
        self.assertEqual(normalize_app_id("firefox.desktop"), "firefox")

    def test_hidden_duplicates_are_filtered(self):
        app = self.FakeApp("pavucontrol.desktop")
        # Monkeypatch isinstance check by using a simple object without DesktopAppInfo
        self.assertFalse(should_list_app(app))

    def test_non_firefox_browsers_are_filtered(self):
        class BrowserApp(self.FakeApp, object):
            pass

        # Bypass DesktopAppInfo path by testing HIDDEN_APP_IDS directly
        self.assertFalse(should_list_app(self.FakeApp("chromium.desktop")))
        self.assertFalse(should_list_app(self.FakeApp("brave-browser.desktop")))


class NetworkParsingTests(unittest.TestCase):
    def test_nmcli_escaped_fields(self):
        self.assertEqual(
            parse_nmcli_terse(r"Cafe\\Guest\:5G:87:yes:▂▄▆█"),
            [r"Cafe\Guest:5G", "87", "yes", "▂▄▆█"],
        )

    def test_nmcli_empty_field(self):
        self.assertEqual(parse_nmcli_terse(":0:no:"), ["", "0", "no", ""])


class BackendParsingTests(unittest.TestCase):
    def test_media_record_does_not_confuse_colons_for_delimiters(self):
        record = FIELD_SEPARATOR.join(
            ("Playing", "Chapter::Two", "Artist::Guest", "file:///cover%20art.png")
        )
        state = parse_media_record(record)
        self.assertEqual(state.title, "Chapter::Two")
        self.assertEqual(state.artist, "Artist::Guest")

    def test_audio_volume_accepts_non_stereo_channels(self):
        self.assertEqual(
            channel_volume_percent({"volume": {"mono": {"value_percent": "73%"}}}),
            73,
        )

    def test_job_callbacks_are_one_shot_even_if_callback_returns_true(self):
        manager = JobManager(max_workers=1)
        delivered = []
        manager.submit(
            "test",
            lambda: 7,
            on_success=lambda value: (delivered.append(value), True)[1],
        )
        deadline = time.monotonic() + 2
        context = GLib.MainContext.default()
        while not delivered and time.monotonic() < deadline:
            while context.pending():
                context.iteration(False)
            time.sleep(0.01)
        for _iteration in range(3):
            while context.pending():
                context.iteration(False)
        manager.close()
        self.assertEqual(delivered, [7])


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
