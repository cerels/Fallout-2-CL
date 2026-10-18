"""The exact Steam Launch Options line for this OS - see launch.sh / launch.bat, which
implement the behavior this line relies on."""

from __future__ import annotations

import sys
from pathlib import Path

from .paths import FROZEN, LAUNCHER_DIR


def launcher_target() -> Path:
    """The file to point Steam - or a non-Steam shortcut, see steam_shortcuts.py - at:
    this launcher itself. A frozen build targets its own exe directly; a source checkout
    targets launch.sh/launch.bat, which find and run f2launcher.py in turn."""
    if FROZEN:
        return Path(sys.executable)
    return LAUNCHER_DIR / "launcher" / ("launch.bat" if sys.platform == "win32" else "launch.sh")


def steam_launch_option(force_campaign: str | None = None) -> tuple[str, bool]:
    """Return (the Launch Options string to paste into Steam, whether it's verified).

    The target is always where this launcher itself lives (LAUNCHER_DIR) - not the active
    campaign's root, which can be a subfolder underneath it. That's what makes it possible
    to paste a *different* Steam entry's launch options (e.g. Fallout 1's, to force Et Tu)
    at the same launcher: `force_campaign` adds the flag that picks the campaign on
    startup, overriding "remember last campaign" for that one launch.

    Linux and macOS both run Launch Options through a shell that treats "#" as a comment,
    so "<target> # %command%" drops Steam's own command entirely. cmd.exe (Windows) has
    no equivalent for that mid-line, so the Windows line is unverified best-effort: Steam
    may run the target and then still launch the game afterward, rather than replacing it.

    A frozen (PyInstaller) build targets its own exe directly - no launcher/ subfolder,
    no launch.sh/launch.bat, since the exe already contains all of their logic itself.
    """
    target = launcher_target()
    args = f" --campaign={force_campaign}" if force_campaign else ""
    if sys.platform == "win32":
        return f'"{target}"{args} %command%', False
    return f'"{target}"{args} # %command%', True
