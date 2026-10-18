"""Game-tree locations and case-insensitive path resolution.

Two directories matter, and they're not always the same one:
  LAUNCHER_DIR  Where this launcher itself lives - fixed for the life of the process.
                Holds launcher.ini, the bundled schemas/, and (source checkout) backend/gui/.
  game_dir()    The active campaign's root - its engine binary, data, mods/, config files.
                Defaults to LAUNCHER_DIR (the Vanilla/CE campaign IS that folder) but a
                total conversion campaign roots at a subfolder underneath it instead; see
                backend/campaigns.py. Switching the active campaign calls set_campaign_root().

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
    LAUNCHER_DIR = Path(sys.executable).resolve().parent
    RESOURCE_DIR = Path(getattr(sys, "_MEIPASS", LAUNCHER_DIR))
    # A frozen Python sometimes can't locate the OS's own CA bundle (varies by distro -
    # notably atomic/immutable ones), breaking HTTPS downloads with CERTIFICATE_VERIFY_FAILED.
    # Point it at certifi's bundled one explicitly instead of relying on OS discovery.
    import os
    try:
        import certifi
        os.environ.setdefault("SSL_CERT_FILE", certifi.where())
    except ImportError:
        pass
else:
    # Source checkout: backend/paths.py is <LAUNCHER_DIR>/launcher/backend/paths.py.
    LAUNCHER_DIR = Path(__file__).resolve().parent.parent.parent
    RESOURCE_DIR = LAUNCHER_DIR / "launcher"

LAUNCHER_INI = LAUNCHER_DIR / "launcher" / "launcher.ini"

PREVIEW_W, PREVIEW_H = 224, 133

_active_root = LAUNCHER_DIR


def game_dir() -> Path:
    """The active campaign's root. Vanilla/CE until set_campaign_root() points elsewhere."""
    return _active_root


def set_campaign_root(root: Path) -> None:
    global _active_root
    _active_root = root


def game_bin() -> Path:
    """The active campaign's engine binary, if any of its known names exist; otherwise the
    Linux name, so callers have something to report as missing."""
    for name in ("fallout2-ce", "fallout2-ce.exe"):
        candidate = _active_root / name
        if candidate.is_file():
            return candidate
    return _active_root / "fallout2-ce"


def backup_dir() -> Path:
    return game_dir() / "backup" / "launcher"


def ci_path(rel: str, root: Path | None = None) -> Path:
    """Resolve a path case-insensitively the way CE does, under `root` (default: the
    active campaign's root)."""
    cur = root if root is not None else game_dir()
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
