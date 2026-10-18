"""Reading Fallout 2 DAT2 archives, and a minimal read-only INI parser for their contents."""

from __future__ import annotations

import struct
import zlib
from pathlib import Path

_dat_dirs: dict[Path, dict[str, tuple]] = {}


def dat_read(dat: Path, name: str) -> bytes | None:
    """Read one file from a Fallout 2 DAT2 archive."""
    try:
        if dat not in _dat_dirs:
            entries = {}
            with open(dat, "rb") as f:
                f.seek(-8, 2)
                tree_size, dat_size = struct.unpack("<II", f.read(8))
                f.seek(dat_size - 8 - tree_size)
                d = f.read(tree_size)
            count = struct.unpack_from("<I", d, 0)[0]
            o = 4
            for _ in range(count):
                ln = struct.unpack_from("<I", d, o)[0]
                o += 4
                entry_name = d[o:o + ln].decode("latin-1").lower()
                o += ln
                entries[entry_name] = struct.unpack_from("<BIII", d, o)
                o += 13
            _dat_dirs[dat] = entries
        entry = _dat_dirs[dat].get(name.lower())
        if entry is None:
            return None
        compressed, _real, packed, offset = entry
        with open(dat, "rb") as f:
            f.seek(offset)
            raw = f.read(packed)
        return zlib.decompress(raw) if compressed else raw
    except (OSError, struct.error, zlib.error):
        return None


def parse_ini_text(text: str) -> dict[tuple[str, str], str]:
    values, section = {}, ""
    for line in text.splitlines():
        t = line.strip()
        if t.startswith("[") and "]" in t:
            section = t[1:t.index("]")].strip().lower()
        elif "=" in t and not t.startswith((";", "#")):
            k, v = t.split("=", 1)
            values[(section, k.strip().lower())] = v.strip()
    return values
