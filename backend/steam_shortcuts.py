"""Adding a campaign to Steam as a non-Steam game shortcut (Q17): for Sonora/Nevada, which
have no Steam app of their own to paste a Launch Options line into. Et Tu and CE already
have real Steam entries (Fallout 2's, and Fallout 1's forcing Et Tu) - see
backend/launch_option.py - so this is only for the other two.

Steam stores non-Steam shortcuts in a per-user shortcuts.vdf, in an undocumented but
well-understood binary format (not the human-readable text VDF): each value is prefixed
with a type byte - 0x00 nested map, 0x01 null-terminated string, 0x02 int32 - and every
map ends with 0x08. _read_binary_vdf/_write_binary_vdf implement just that, generally
enough to round-trip a real file byte-for-byte (verified with a synthetic fixture, not a
real file - this never reads this launcher's own test data from an actual Steam install).

add_campaign_to_steam() only ever edits the one entry it owns (matched by its `exe`
field) inside the existing shortcuts.vdf - every other shortcut already in the file is
parsed and written back completely unchanged - and backs up the original once before the
first write, the same way Store.save() backs up config files under backup/launcher/.
"""

from __future__ import annotations

import shutil
import sys
from pathlib import Path

from .campaigns import Campaign
from .launch_option import launcher_target


def _read_binary_vdf(data: bytes, start: int, end: int) -> tuple[dict, int]:
    """Parse one binary-VDF map from data[start:end]. Returns (dict, offset after it).
    Works for both a nested map (ends at its own 0x08 byte) and the implicit top-level map
    (ends when `end` is reached, since the file has no wrapping 0x08 of its own)."""
    obj: dict = {}
    i = start
    while i < end:
        t = data[i]
        i += 1
        if t == 0x08:
            return obj, i
        j = data.index(b"\x00", i)
        key = data[i:j].decode("utf-8")
        i = j + 1
        if t == 0x00:
            val, i = _read_binary_vdf(data, i, end)
        elif t == 0x01:
            j = data.index(b"\x00", i)
            val = data[i:j].decode("utf-8")
            i = j + 1
        elif t == 0x02:
            val = int.from_bytes(data[i:i + 4], "little", signed=True)
            i += 4
        else:
            raise ValueError(f"shortcuts.vdf: unsupported type byte {t:#x} at offset {i - 1}")
        obj[key] = val
    return obj, i


def _write_binary_vdf(obj: dict, top_level: bool = False) -> bytes:
    out = bytearray()
    for key, val in obj.items():
        key_bytes = key.encode("utf-8") + b"\x00"
        if isinstance(val, dict):
            out += b"\x00" + key_bytes + _write_binary_vdf(val)
        elif isinstance(val, str):
            out += b"\x01" + key_bytes + val.encode("utf-8") + b"\x00"
        elif isinstance(val, int):
            out += b"\x02" + key_bytes + val.to_bytes(4, "little", signed=True)
        else:
            raise TypeError(f"shortcuts.vdf: unsupported value type for key {key!r}: {type(val)}")
    if not top_level:
        out += b"\x08"
    return bytes(out)


def read_shortcuts(path: Path) -> dict:
    data = path.read_bytes()
    obj, _ = _read_binary_vdf(data, 0, len(data))
    return obj


def write_shortcuts(path: Path, root: dict) -> None:
    import os
    data = _write_binary_vdf(root, top_level=True)
    backup = path.with_name(path.name + ".bak-f2launcher")
    if path.is_file() and not backup.exists():
        shutil.copy2(path, backup)
    tmp = path.with_name(path.name + ".tmp")
    tmp.write_bytes(data)
    os.replace(tmp, path)


def _steam_userdata_configs() -> list[Path]:
    """Every `userdata/<id>/config` folder this install of Steam might have, across the
    handful of locations each OS commonly uses. Best-effort: Steam's own install location
    is user-configurable, so this can't be exhaustive."""
    candidates: list[Path] = []
    if sys.platform in ("linux", "darwin"):
        bases = [Path.home() / ".steam/steam/userdata", Path.home() / ".local/share/Steam/userdata"]
        if sys.platform == "darwin":
            bases.append(Path.home() / "Library/Application Support/Steam/userdata")
    else:
        import os
        bases = [Path(os.environ[var]) / "Steam" / "userdata"
                 for var in ("ProgramFiles(x86)", "ProgramFiles") if var in os.environ]
    for base in bases:
        if base.is_dir():
            candidates += [p / "config" for p in base.iterdir() if (p / "config").is_dir()]
    seen, unique = set(), []
    for c in candidates:
        resolved = c.resolve()
        if resolved not in seen:
            seen.add(resolved)
            unique.append(c)
    return unique


def _pick_userdata_config() -> Path | None:
    """The Steam profile most likely to be the one in use: whichever has the most
    recently modified localconfig.vdf. Falls back to the only candidate, or None."""
    configs = _steam_userdata_configs()
    if not configs:
        return None
    with_mtime = [(c, (c / "localconfig.vdf").stat().st_mtime) for c in configs
                  if (c / "localconfig.vdf").is_file()]
    return max(with_mtime, key=lambda t: t[1])[0] if with_mtime else configs[0]


def add_non_steam_game(name: str, exe: Path, start_dir: Path, launch_options: str = "") -> Path:
    """Add or update (matched by `exe`) one non-Steam game shortcut. Returns the
    shortcuts.vdf path written. Raises RuntimeError if no Steam profile was found."""
    config_dir = _pick_userdata_config()
    if config_dir is None:
        raise RuntimeError("Couldn't find a Steam userdata folder - is Steam installed?")
    path = config_dir / "shortcuts.vdf"
    root = read_shortcuts(path) if path.is_file() else {"shortcuts": {}}
    shortcuts = root.setdefault("shortcuts", {})

    exe_quoted = f'"{exe}"'
    existing_key = next((k for k, v in shortcuts.items()
                        if isinstance(v, dict) and v.get("exe") == exe_quoted), None)
    key = existing_key if existing_key is not None else str(len(shortcuts))
    entry = dict(shortcuts.get(key, {}))
    entry.update({
        "appname": name,
        "exe": exe_quoted,
        "StartDir": f'"{start_dir}"',
        "icon": entry.get("icon", ""),
        "ShortcutPath": "",
        "LaunchOptions": launch_options,
        "IsHidden": 0,
        "AllowDesktopConfig": 1,
        "AllowOverlay": 1,
        "OpenVR": 0,
        "Devkit": 0,
        "DevkitGameID": "",
        "DevkitOverrideAppID": 0,
        "LastPlayTime": entry.get("LastPlayTime", 0),
        "FlatpakAppID": entry.get("FlatpakAppID", ""),
        "tags": entry.get("tags", {}),
    })
    shortcuts[key] = entry
    write_shortcuts(path, root)
    return path


def add_campaign_to_steam(campaign: Campaign) -> Path:
    """Add this launcher as a non-Steam game, forced into `campaign` - for a campaign
    with no Steam app of its own (Sonora, Nevada). Pressing Play on the resulting Steam
    entry opens straight into that campaign."""
    target = launcher_target()
    return add_non_steam_game(
        name=f"Fallout 2 Launcher — {campaign.name}",
        exe=target, start_dir=target.parent,
        launch_options=f"--campaign={campaign.id}")
