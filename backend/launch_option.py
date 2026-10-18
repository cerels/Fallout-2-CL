"""The exact Steam Launch Options line for this OS - see launch.sh / launch.bat, which
implement the behavior this line relies on."""

from __future__ import annotations

import sys

from .paths import FROZEN, GAME_DIR


def steam_launch_option() -> tuple[str, bool]:
    """Return (the Launch Options string to paste into Steam, whether it's verified).

    Linux and macOS both run Launch Options through a shell that treats "#" as a comment,
    so "<target> # %command%" drops Steam's own command entirely. cmd.exe (Windows) has
    no equivalent for that mid-line, so the Windows line is unverified best-effort: Steam
    may run the target and then still launch the game afterward, rather than replacing it.

    A frozen (PyInstaller) build targets its own exe directly - no launcher/ subfolder,
    no launch.sh/launch.bat, since the exe already contains all of their logic itself.
    """
    target = sys.executable if FROZEN else GAME_DIR / "launcher" / ("launch.bat" if sys.platform == "win32"
                                                                    else "launch.sh")
    if sys.platform == "win32":
        return f'"{target}" %command%', False
    return f'"{target}" # %command%', True
