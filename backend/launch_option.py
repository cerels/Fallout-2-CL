"""The exact Steam Launch Options line for this OS - see launch.sh / launch.bat, which
implement the behavior this line relies on."""

from __future__ import annotations

import sys

from .paths import GAME_DIR


def steam_launch_option() -> tuple[str, bool]:
    """Return (the Launch Options string to paste into Steam, whether it's verified).

    Linux and macOS both run Launch Options through a shell that treats "#" as a comment,
    so "launch.sh # %command%" drops Steam's own command entirely. cmd.exe (Windows) has
    no equivalent for that mid-line, so the Windows line is unverified best-effort: Steam
    may run launch.bat and then still launch the game afterward, rather than replacing it.
    """
    launcher_dir = GAME_DIR / "launcher"
    if sys.platform == "win32":
        return f'"{launcher_dir / "launch.bat"}" %command%', False
    return f'"{launcher_dir / "launch.sh"}" # %command%', True
