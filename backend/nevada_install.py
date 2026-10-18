"""Installing Fallout 2: Nevada (English translation): a full standalone game (its own
master.dat/critter.dat), into its own Nevada/ campaign root - see GLOSSARY.md /
docs/adr/0001.

HogLord69/fallout-english-localization's release bundles the game with the classic
Windows-only engine (NEVADA_EV.exe, ddraw.dll's sfall injection, an sfall ini-editor GUI) -
none of that is needed once running it through fallout2-ce instead, which the project's
own README lists as "supported with minor tweaks needed". The one tweak verified by
inspecting the actual archive: its main config file is named config.cfg, not
fallout2.cfg, but is otherwise the exact same classic-engine format fallout2.cfg already
uses - install_nevada() renames it. Engine binary comes from the same place as every
other campaign's, fallout2-ce's own release, reused via ce_install.install_ce() pointed
at this campaign's root instead of Vanilla/CE's.
"""

from __future__ import annotations

import sys
from pathlib import Path

from .campaigns import by_id
from .paths import campaign_root_override, ci_path

NEVADA_REPO = "HogLord69/fallout-english-localization"
CAMPAIGN_ID = "nevada"

# The classic Windows-only engine + its sfall injection + its ini-editor GUI - all
# superseded by fetching a native fallout2-ce build instead, on every platform.
_SKIP_NAMES = {"nevada_ev.exe", "ddraw.dll", "ddraw.ini", "play nevada.bat", "sf-configurator.exe"}


def nevada_root() -> Path:
    return by_id(CAMPAIGN_ID).root


def _state_path() -> Path:
    return nevada_root() / ".f2launcher-nevada.json"


def load_nevada_state() -> dict:
    import json
    try:
        return json.loads(_state_path().read_text("utf-8"))
    except (OSError, ValueError):
        return {}


def save_nevada_state(state: dict) -> None:
    import json
    _state_path().write_text(json.dumps(state, indent=2, sort_keys=True), "utf-8")


def installed() -> bool:
    return by_id(CAMPAIGN_ID).installed()


def resolve_nevada_download() -> tuple[str, str, str]:
    """Return (url, filename, version) for the latest Fallout-Nevada-English.zip - this
    repo's releases bundle Nevada, Resurrection and Sonora English repacks together, so
    this picks out just the Nevada asset."""
    import json
    import urllib.request

    api_url = f"https://api.github.com/repos/{NEVADA_REPO}/releases/latest"
    with urllib.request.urlopen(api_url, timeout=20) as resp:
        data = json.load(resp)
    for a in data.get("assets", []):
        if a["name"].lower() == "fallout-nevada-english.zip":
            return a["browser_download_url"], a["name"], str(data.get("tag_name", ""))
    raise RuntimeError("No Fallout-Nevada-English.zip asset found in the latest release.")


def install_nevada() -> None:
    """Download Nevada's release and extract it into its own campaign root (skipping the
    classic Windows-only engine bits), rename its config.cfg to fallout2.cfg, then fetch a
    native fallout2-ce engine binary the same way as every other campaign. Raises on any
    failure."""
    import shutil
    import tempfile
    import urllib.request
    import zipfile

    root = nevada_root()
    url, filename, version = resolve_nevada_download()
    with tempfile.TemporaryDirectory() as tmp:
        downloaded = Path(tmp) / filename
        with urllib.request.urlopen(url, timeout=600) as resp, open(downloaded, "wb") as out:
            shutil.copyfileobj(resp, out)
        with zipfile.ZipFile(downloaded) as zf:
            top_level = {n.split("/", 1)[0] for n in zf.namelist() if n}
            if len(top_level) != 1:
                raise RuntimeError(f"Nevada's release layout has changed (expected one top-level "
                                   f"folder, found {sorted(top_level)}) - update "
                                   "nevada_install.py's assumptions.")
            prefix = next(iter(top_level)) + "/"
            copied = 0
            for info in zf.infolist():
                if info.is_dir() or not info.filename.startswith(prefix):
                    continue
                rel = info.filename[len(prefix):]
                if rel.split("/")[-1].lower() in _SKIP_NAMES:
                    continue
                dest = ci_path(rel, root=root)
                dest.parent.mkdir(parents=True, exist_ok=True)
                with zf.open(info) as src, open(dest, "wb") as out:
                    shutil.copyfileobj(src, out)
                copied += 1
            if not copied:
                raise RuntimeError("Nevada archive layout has changed - no recognized files found")

    config = ci_path("config.cfg", root=root)
    if config.is_file():
        config.rename(root / "fallout2.cfg")

    if sys.platform in ("linux", "win32"):
        from .ce_install import install_ce
        with campaign_root_override(root):
            install_ce()
    # macOS: not automated yet, same as CE's own install.

    save_nevada_state({"version": version})
