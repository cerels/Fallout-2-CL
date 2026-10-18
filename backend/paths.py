"""Game-tree locations and case-insensitive path resolution.

Everything else in backend/ and in f2launcher.py builds on this module.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

FROZEN = getattr(sys, "frozen", False)

if FROZEN:
    # A PyInstaller onefile build: the exe sits directly in the game folder, and its
    # bundled schemas/ live in the temp dir it extracts itself to (sys._MEIPASS).
    GAME_DIR = Path(sys.executable).resolve().parent
    RESOURCE_DIR = Path(getattr(sys, "_MEIPASS", GAME_DIR))
else:
    # Source checkout: backend/paths.py is <GAME_DIR>/launcher/backend/paths.py.
    GAME_DIR = Path(__file__).resolve().parent.parent.parent
    RESOURCE_DIR = GAME_DIR / "launcher"

GAME_BIN = next((GAME_DIR / n for n in ("fallout2-ce", "fallout2-ce.exe") if (GAME_DIR / n).is_file()),
                GAME_DIR / "fallout2-ce")
LAUNCHER_INI = GAME_DIR / "launcher" / "launcher.ini"
BACKUP_DIR = GAME_DIR / "backup" / "launcher"

PREVIEW_W, PREVIEW_H = 224, 133


def ci_path(rel: str) -> Path:
    """Resolve a path under GAME_DIR case-insensitively, the way CE does."""
    cur = GAME_DIR
    for part in re.split(r"[\\/]", rel):
        if not part:
            continue
        cand = cur / part
        if not cand.exists() and cur.is_dir():
            low = part.lower()
            for entry in cur.iterdir():
                if entry.name.lower() == low:
                    cand = entry
                    break
        cur = cand
    return cur


def mod_installed(dat: str) -> bool:
    return ci_path(f"mods/{dat}").is_file()
