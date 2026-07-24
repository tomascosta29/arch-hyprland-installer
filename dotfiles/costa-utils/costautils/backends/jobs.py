#!/usr/bin/env python3
"""Bounded background work and GTK-main-loop delivery."""

from __future__ import annotations

import itertools
import logging
import threading
from concurrent.futures import Future, ThreadPoolExecutor
from typing import Any, Callable

from gi.repository import GLib

LOGGER = logging.getLogger(__name__)


class JobManager:
    """A bounded executor with keyed generations to discard stale results."""

    def __init__(self, max_workers: int = 4):
        self._executor = ThreadPoolExecutor(
            max_workers=max_workers,
            thread_name_prefix="costa-utils",
        )
        self._lock = threading.Lock()
        self._generations: dict[str, int] = {}
        self._closed = False

    def next_generation(self, key: str) -> int:
        with self._lock:
            generation = self._generations.get(key, 0) + 1
            self._generations[key] = generation
            return generation

    def is_current(self, key: str, generation: int) -> bool:
        with self._lock:
            return not self._closed and self._generations.get(key) == generation

    def is_open(self) -> bool:
        with self._lock:
            return not self._closed

    def submit(
        self,
        key: str,
        worker: Callable[..., Any],
        *args: Any,
        on_success: Callable[[Any], Any] | None = None,
        on_error: Callable[[BaseException], Any] | None = None,
        replace: bool = True,
    ) -> Future[Any] | None:
        with self._lock:
            if self._closed:
                return None
        generation = self.next_generation(key) if replace else 0
        future = self._executor.submit(worker, *args)

        def deliver(callback, value) -> bool:
            callback(value)
            return GLib.SOURCE_REMOVE

        def completed(done: Future[Any]) -> None:
            if (replace and not self.is_current(key, generation)) or (
                not replace and not self.is_open()
            ):
                return
            try:
                value = done.result()
            except BaseException as error:  # Future transports worker failures.
                LOGGER.error(
                    "background job %s failed: %s",
                    key,
                    error,
                    exc_info=(type(error), error, error.__traceback__),
                )
                if on_error is not None:
                    GLib.idle_add(deliver, on_error, error)
                return
            if on_success is not None:
                GLib.idle_add(deliver, on_success, value)

        future.add_done_callback(completed)
        return future

    def invalidate(self, key: str) -> None:
        self.next_generation(key)

    def close(self) -> None:
        with self._lock:
            if self._closed:
                return
            self._closed = True
            for key in tuple(self._generations):
                self._generations[key] += 1
        self._executor.shutdown(wait=False, cancel_futures=True)


class Debouncer:
    """Coalesce rapid GTK signals into the latest call."""

    _ids = itertools.count()

    def __init__(self, delay_ms: int, callback: Callable[..., Any]):
        self.delay_ms = delay_ms
        self.callback = callback
        self.source_id: int | None = None
        self.identity = next(self._ids)

    def schedule(self, *args: Any) -> None:
        self.cancel()

        def invoke() -> bool:
            self.source_id = None
            self.callback(*args)
            return GLib.SOURCE_REMOVE

        self.source_id = GLib.timeout_add(self.delay_ms, invoke)

    def cancel(self) -> None:
        if self.source_id is not None:
            GLib.source_remove(self.source_id)
            self.source_id = None
