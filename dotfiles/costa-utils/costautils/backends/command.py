#!/usr/bin/env python3
"""Small, observable wrapper around external desktop commands."""

from __future__ import annotations

import logging
import subprocess
from dataclasses import dataclass
from typing import Sequence

LOGGER = logging.getLogger(__name__)


@dataclass(frozen=True)
class CommandResult:
    argv: tuple[str, ...]
    returncode: int
    stdout: str
    stderr: str

    @property
    def ok(self) -> bool:
        return self.returncode == 0


def run(
    argv: Sequence[str],
    *,
    input_text: str | None = None,
    timeout: float = 15,
    check: bool = False,
) -> CommandResult:
    """Run a bounded command and retain enough context for useful logs."""
    command = tuple(str(value) for value in argv)
    LOGGER.debug("running command: %s", command)
    try:
        completed = subprocess.run(
            command,
            input=input_text,
            capture_output=True,
            text=True,
            timeout=timeout,
            check=False,
        )
    except (OSError, subprocess.TimeoutExpired):
        LOGGER.exception("command failed to execute: %s", command)
        raise

    result = CommandResult(
        argv=command,
        returncode=completed.returncode,
        stdout=completed.stdout,
        stderr=completed.stderr,
    )
    if not result.ok:
        LOGGER.warning(
            "command exited %d: %s; stderr=%r",
            result.returncode,
            command,
            result.stderr.strip(),
        )
        if check:
            raise subprocess.CalledProcessError(
                result.returncode,
                command,
                output=result.stdout,
                stderr=result.stderr,
            )
    return result
