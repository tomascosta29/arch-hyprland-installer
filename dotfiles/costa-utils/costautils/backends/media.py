#!/usr/bin/env python3
"""One MPRIS monitor and artwork cache shared by all Costa Utils windows."""

from __future__ import annotations

import logging
import subprocess
import threading
import urllib.request
from collections import OrderedDict
from dataclasses import dataclass
from typing import Callable
from urllib.parse import unquote, urlparse

from gi.repository import Gdk, GdkPixbuf, GLib

from .command import run
from .jobs import JobManager

LOGGER = logging.getLogger(__name__)
FIELD_SEPARATOR = "\x1f"
MAX_ARTWORK_BYTES = 2 * 1024 * 1024


@dataclass(frozen=True)
class MediaState:
    status: str
    title: str
    artist: str
    artwork_url: str


def parse_media_record(line: str) -> MediaState | None:
    fields = line.rstrip("\n").split(FIELD_SEPARATOR, 3)
    return MediaState(*fields) if len(fields) == 4 else None


class MediaBackend:
    def __init__(self, jobs: JobManager):
        self.jobs = jobs
        self._subscribers: dict[object, Callable[[MediaState], object]] = {}
        self._process: subprocess.Popen[str] | None = None
        self._process_lock = threading.Lock()
        self._artwork: OrderedDict[str, bytes] = OrderedDict()

    def subscribe(self, owner: object, callback: Callable[[MediaState], object]) -> None:
        self._subscribers[owner] = callback
        self._start()

    def unsubscribe(self, owner: object) -> None:
        self._subscribers.pop(owner, None)
        if not self._subscribers:
            self.stop()

    def _start(self) -> None:
        with self._process_lock:
            if self._process is not None:
                return
            try:
                self._process = subprocess.Popen(
                    [
                        "playerctl",
                        "--follow",
                        "metadata",
                        "--format",
                        FIELD_SEPARATOR.join(
                            (
                                "{{status}}",
                                "{{title}}",
                                "{{artist}}",
                                "{{mpris:artUrl}}",
                            )
                        ),
                    ],
                    stdout=subprocess.PIPE,
                    stderr=subprocess.DEVNULL,
                    text=True,
                    bufsize=1,
                )
            except OSError:
                LOGGER.exception("unable to start playerctl monitor")
                self._process = None
                return
            process = self._process
        threading.Thread(
            target=self._read_monitor,
            args=(process,),
            daemon=True,
            name="costa-media-monitor",
        ).start()

    def _read_monitor(self, process: subprocess.Popen[str]) -> None:
        assert process.stdout is not None
        for line in process.stdout:
            state = parse_media_record(line)
            if state is None:
                LOGGER.warning("discarding malformed playerctl record: %r", line.rstrip())
                continue
            GLib.idle_add(self._publish, state)
        returncode = process.wait()
        with self._process_lock:
            if self._process is process:
                self._process = None
        if returncode not in (0, -15):
            LOGGER.warning("playerctl monitor exited %d", returncode)
        if self._subscribers:
            GLib.timeout_add_seconds(2, self._restart)

    def _restart(self) -> bool:
        if self._subscribers:
            self._start()
        return GLib.SOURCE_REMOVE

    def _publish(self, state: MediaState) -> bool:
        for callback in tuple(self._subscribers.values()):
            callback(state)
        return GLib.SOURCE_REMOVE

    def stop(self) -> None:
        with self._process_lock:
            process = self._process
            self._process = None
        if process is not None and process.poll() is None:
            process.terminate()

    def command(self, action: str, on_error: Callable[[BaseException], object]) -> None:
        allowed = {"previous", "play-pause", "next"}
        if action not in allowed:
            raise ValueError(f"unsupported media action: {action}")
        self.jobs.submit(
            f"media-command-{action}",
            lambda: run(["playerctl", action], check=True),
            on_error=on_error,
            replace=False,
        )

    def load_artwork(
        self,
        owner: object,
        url: str,
        size: int,
        callback: Callable[[Gdk.Texture | None], object],
    ) -> None:
        cached = self._artwork.get(url)
        if cached is not None:
            self._artwork.move_to_end(url)
            callback(self._decode(cached, size))
            return

        def loaded(data: bytes) -> None:
            self._artwork[url] = data
            self._artwork.move_to_end(url)
            while len(self._artwork) > 16:
                self._artwork.popitem(last=False)
            callback(self._decode(data, size))

        self.jobs.submit(
            f"media-artwork-{id(owner)}",
            self._read_artwork,
            url,
            on_success=loaded,
            on_error=lambda _error: callback(None),
        )

    @staticmethod
    def _read_artwork(url: str) -> bytes:
        parsed = urlparse(url)
        if parsed.scheme == "file":
            path = unquote(parsed.path)
            with open(path, "rb") as stream:
                data = stream.read(MAX_ARTWORK_BYTES + 1)
        elif parsed.scheme in ("http", "https"):
            request = urllib.request.Request(url, headers={"User-Agent": "CostaUtils/1.0"})
            with urllib.request.urlopen(request, timeout=4) as response:
                data = response.read(MAX_ARTWORK_BYTES + 1)
        else:
            raise ValueError(f"unsupported artwork URL scheme: {parsed.scheme}")
        if len(data) > MAX_ARTWORK_BYTES:
            raise ValueError("media artwork exceeds 2 MiB")
        return data

    @staticmethod
    def _decode(data: bytes, size: int) -> Gdk.Texture | None:
        try:
            loader = GdkPixbuf.PixbufLoader()
            loader.write(data)
            loader.close()
            pixbuf = loader.get_pixbuf()
            if pixbuf is None:
                return None
            scaled = pixbuf.scale_simple(size, size, GdkPixbuf.InterpType.BILINEAR)
            return Gdk.Texture.new_for_pixbuf(scaled)
        except GLib.Error:
            LOGGER.exception("unable to decode media artwork")
            return None

    def close(self) -> None:
        self._subscribers.clear()
        self.stop()
        self._artwork.clear()
