"""Reading SAVE.DAT headers and the game's palette for save-slot thumbnails."""

from __future__ import annotations

import re
import struct
from dataclasses import dataclass
from pathlib import Path

from .dat import dat_read
from .paths import PREVIEW_H, PREVIEW_W, ci_path

LOCATIONS = [
    ("ARTEMPLE", "Temple of Trials"), ("AR", "Arroyo"), ("KLA", "Klamath"),
    ("DEN", "The Den"), ("MOD", "Modoc"), ("GHOST", "Ghost Farm"),
    ("VCT", "Vault City"), ("VAULTC", "Vault City"), ("GECK", "Gecko"),
    ("BH", "Broken Hills"), ("BROKEN", "Broken Hills"), ("NEWR", "New Reno"),
    ("RED", "Redding"), ("NCR", "NCR"), ("V13", "Vault 13"), ("V15", "Vault 15"),
    ("SF", "San Francisco"), ("NAV", "Navarro"), ("RAIDERS", "Raiders' camp"),
    ("ABBEY", "Abbey"), ("EPA", "EPA"),
]
MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"]


@dataclass
class SaveGame:
    slot: int
    path: Path
    name: str
    description: str
    location: str
    map_name: str
    game_date: str
    mtime: float
    thumbnail: bytes | None


def friendly_location(map_name: str) -> str:
    up = map_name.upper()
    for prefix, label in LOCATIONS:
        if up.startswith(prefix):
            return label
    return map_name.title() if map_name else "Unknown"


def read_saves() -> list[SaveGame]:
    saves = []
    root = ci_path("data/SAVEGAME")
    if not root.is_dir():
        return saves
    for slot_dir in root.iterdir():
        m = re.fullmatch(r"slot(\d+)", slot_dir.name, re.IGNORECASE)
        if not m or not slot_dir.is_dir():
            continue
        dat = next((p for p in slot_dir.iterdir() if p.name.lower() == "save.dat"), None)
        if dat is None:
            continue
        try:
            with open(dat, "rb") as f:
                head = f.read(131 + PREVIEW_W * PREVIEW_H)
        except OSError:
            continue
        if len(head) < 131 or not head.startswith(b"FALLOUT SAVE FILE"):
            continue

        def cstr(b: bytes) -> str:
            return b.split(b"\0", 1)[0].decode("latin-1").strip()

        month, day, year = struct.unpack_from(">hhh", head, 101)
        map_name = cstr(head[115:131]).rsplit(".", 1)[0]
        game_date = f"{day} {MONTHS[month - 1]} {year}" if 1 <= month <= 12 else ""
        thumb = head[131:] if len(head) == 131 + PREVIEW_W * PREVIEW_H else None
        saves.append(SaveGame(
            slot=int(m.group(1)), path=dat, name=cstr(head[29:61]) or "Unnamed",
            description=cstr(head[61:91]), location=friendly_location(map_name),
            map_name=map_name, game_date=game_date, mtime=dat.stat().st_mtime,
            thumbnail=thumb))
    saves.sort(key=lambda s: s.mtime, reverse=True)
    return saves


def load_palette() -> list[tuple[int, int, int]] | None:
    for dat in [ci_path("master.dat")]:
        pal = dat_read(dat, "color.pal")
        if pal and len(pal) >= 768:
            return [(min(pal[i] * 4, 255), min(pal[i + 1] * 4, 255), min(pal[i + 2] * 4, 255))
                    for i in range(0, 768, 3)]
    return None
