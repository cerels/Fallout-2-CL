"""Reading launcher.ini, building fallout2-ce's argv, and replacing this process with it."""

from __future__ import annotations

import os

from .ini_file import IniFile
from .paths import GAME_BIN, GAME_DIR, LAUNCHER_INI
from .saves import read_saves


def launcher_prefs() -> IniFile:
    return IniFile(LAUNCHER_INI)


def game_args(mode: str, slot: int | None = None) -> list[str]:
    """Command-line arguments for fallout2-ce for a launch mode."""
    if mode == "slot" and slot is not None:
        return [f"--dev-load-game={slot}"]
    if mode == "continue":
        saves = read_saves()
        if saves:
            return [f"--dev-load-game={saves[0].slot}"]
    return []


def exec_game(args: list[str]) -> None:
    os.chdir(GAME_DIR)
    os.execv(str(GAME_BIN), [str(GAME_BIN), *args])
