"""Mods the launcher can download: schemas/catalog.toml, GitHub release resolution,
installing, update-checking and deleting. See schemas/catalog.toml for the data format."""

from __future__ import annotations

import re
import shutil
from dataclasses import dataclass
from pathlib import Path

from .paths import RESOURCE_DIR, ci_path, mod_installed


@dataclass
class CatalogEntry:
    """A mod the launcher can download: see schemas/catalog.toml."""
    id: str
    name: str
    description: str
    homepage: str
    dat: list[str]
    github: str | None = None
    asset: str = "*.zip"
    raw: str | None = None
    ini: str | None = None

    def installed(self) -> bool:
        return any(mod_installed(d) for d in self.dat)


def load_mod_catalog() -> list[CatalogEntry]:
    import tomllib

    path = RESOURCE_DIR / "schemas" / "catalog.toml"
    try:
        doc = tomllib.loads(path.read_text("utf-8"))
    except (OSError, UnicodeDecodeError, tomllib.TOMLDecodeError):
        return []
    entries = []
    for e in doc.get("entry", []):
        try:
            entries.append(CatalogEntry(
                id=str(e["id"]), name=str(e["name"]), description=str(e.get("description", "")),
                homepage=str(e["homepage"]), dat=[str(d) for d in e["dat"]],
                github=e.get("github"), asset=str(e.get("asset", "*.zip")), raw=e.get("raw"),
                ini=e.get("ini")))
        except KeyError:
            continue
    return entries


def _catalog_state_path() -> Path:
    return ci_path("mods/.f2launcher-mods.json")


def load_catalog_state() -> dict[str, str]:
    """Versions the launcher last installed/saw, by catalog id. Its own bookkeeping file."""
    import json
    try:
        return json.loads(_catalog_state_path().read_text("utf-8"))
    except (OSError, ValueError):
        return {}


def save_catalog_state(state: dict[str, str]) -> None:
    import json
    path = _catalog_state_path()
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(state, indent=2, sort_keys=True), "utf-8")


def resolve_download(entry: CatalogEntry) -> tuple[str, str, str | None]:
    """Return (url, filename, version) for the file this entry's catalog data points at.
    version is the GitHub release tag, or None for a raw (non-versioned) download."""
    if entry.raw:
        return entry.raw, entry.dat[0], None
    import fnmatch
    import json
    import urllib.request

    api_url = f"https://api.github.com/repos/{entry.github}/releases/latest"
    with urllib.request.urlopen(api_url, timeout=20) as resp:
        data = json.load(resp)
    version = data.get("tag_name")
    for a in data.get("assets", []):
        if fnmatch.fnmatch(a["name"].lower(), entry.asset.lower()):
            return a["browser_download_url"], a["name"], version
    raise RuntimeError(f"{entry.name}: no release asset matching {entry.asset!r}")


def check_for_update(entry: CatalogEntry) -> str | None:
    """Latest version if it's newer than what we last installed, else None.
    The first time an entry is checked, its current version becomes the baseline
    (nothing to compare against yet), so no update is reported that round."""
    if not entry.github:
        return None
    try:
        _, _, version = resolve_download(entry)
    except Exception:
        return None  # offline, rate-limited, etc. - just skip silently
    if not version:
        return None
    state = load_catalog_state()
    baseline = state.get(entry.id)
    if baseline is None:
        state[entry.id] = version
        save_catalog_state(state)
        return None
    return version if version != baseline else None


def install_catalog_entry(entry: CatalogEntry) -> None:
    """Download entry's file and place it under mods/. Raises on any failure."""
    import tempfile
    import urllib.request
    import zipfile

    url, filename, version = resolve_download(entry)
    with tempfile.TemporaryDirectory() as tmp:
        downloaded = Path(tmp) / filename
        with urllib.request.urlopen(url, timeout=60) as resp, open(downloaded, "wb") as out:
            shutil.copyfileobj(resp, out)
        mods_dir = ci_path("mods")
        mods_dir.mkdir(parents=True, exist_ok=True)
        if entry.raw:
            shutil.copy2(downloaded, mods_dir / entry.dat[0])
            return
        with zipfile.ZipFile(downloaded) as z:
            copied = 0
            for info in z.infolist():
                parts = [p for p in info.filename.replace("\\", "/").split("/") if p]
                if info.is_dir() or not parts or parts[0].lower() != "mods" or len(parts) < 2:
                    continue
                dest = mods_dir / "/".join(parts[1:])
                dest.parent.mkdir(parents=True, exist_ok=True)
                with z.open(info) as src, open(dest, "wb") as out:
                    shutil.copyfileobj(src, out)
                copied += 1
            if not copied:
                raise RuntimeError(f"{entry.name}: archive had no mods/ contents")
    enable_mod_file(entry.dat[0])
    if version:
        state = load_catalog_state()
        state[entry.id] = version
        save_catalog_state(state)


def delete_mod_files(dat_name: str, extra: list[str] | None = None) -> list[Path]:
    """Remove dat_name (and any extra files, e.g. its .ini) from mods/, and drop
    dat_name's line from mods_order.txt entirely. Returns the paths removed."""
    removed = []
    for rel in [dat_name, *(extra or [])]:
        p = ci_path(f"mods/{rel}")
        if p.is_file():
            p.unlink()
            removed.append(p)
    path = ci_path("mods/mods_order.txt")
    if path.is_file():
        lines = path.read_text("latin-1").splitlines()
        kept = [ln for ln in lines if re.sub(r"[;#]", "", ln).strip().lower() != dat_name.lower()]
        if kept != lines:
            path.write_text("\n".join(kept) + "\n", "latin-1")
    return removed


def enable_mod_file(dat_name: str) -> None:
    """Add dat_name to mods_order.txt, enabled, unless it's already listed."""
    path = ci_path("mods/mods_order.txt")
    lines = path.read_text("latin-1").splitlines() if path.is_file() else []
    for line in lines:
        if re.sub(r"[;#]", "", line).strip().lower() == dat_name.lower():
            return
    lines.append(dat_name)
    path.write_text("\n".join(lines) + "\n", "latin-1")
