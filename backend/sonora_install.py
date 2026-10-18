"""Installing Fallout: Sonora: a full standalone game (its own master.dat/critter.dat),
into its own Sonora/ campaign root - see GLOSSARY.md / docs/adr/0001.

The official English release (cambragol/Fallout-Sonora-English) ships the complete game
plus a default Windows engine binary and Sonora's own fission.dat, and bundles every
platform's engine build again as nested zips inside itself. Extracting those nested zips
was more trouble than it's worth (a zip inside a zip means parsing a second central
directory out of a byte range of the first), and there's a cleaner source anyway: the
engine's own project, cambragol/fission-ce, publishes the same per-platform builds as
plain top-level release assets. install_sonora() uses that instead for the binary - but
never for fission.dat: fission-ce's own fission.dat is a different size than the one
bundled in the Sonora release, so it's Sonora-specific and must come from the Sonora zip,
not the generic engine release.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

from .campaigns import by_id
from .paths import ci_path

SONORA_REPO = "cambragol/Fallout-Sonora-English"
FISSION_REPO = "cambragol/fission-ce"
CAMPAIGN_ID = "sonora"

# Inside the main release zip: things that aren't part of the playable game, or that
# we're replacing with a native binary fetched from FISSION_REPO instead.
_SKIP_TOP_DIRS = {"__MACOSX", "BIS_mapper"}
# fallout-sonora-x64.exe is the bundled default Windows binary - always superseded by
# _fetch_fission_binary(), on every platform, so there's no point keeping it around.
_SKIP_NAMES = {"fsonoradlc.exe", "log_develop.txt", "fallout-sonora-x64.exe"}

# (archive filename, binary filename inside it) from FISSION_REPO's latest release, by
# sys.platform + architecture - same idea as ce_install's _ASSETS_BY_PLATFORM.
_FISSION_ASSETS: dict[str, dict[str, tuple[str, str]]] = {
    "linux": {
        "x86_64": ("fallout-fission-linux-x64.zip", "fallout-fission-linux-x64"),
        "aarch64": ("fallout-fission-linux-arm64.zip", "fallout-fission-linux-arm64"),
        "armv7l": ("fallout-fission-linux-armhf.zip", "fallout-fission-linux-armhf"),
    },
    "win32": {
        "AMD64": ("fallout-fission-windows-x64.zip", "fallout-fission-x64.exe"),
        "x86": ("fallout-fission-windows-x86.zip", "fallout-fission-x86.exe"),
    },
}


def sonora_root() -> Path:
    return by_id(CAMPAIGN_ID).root


def _state_path() -> Path:
    return sonora_root() / ".f2launcher-sonora.json"


def load_sonora_state() -> dict:
    import json
    try:
        return json.loads(_state_path().read_text("utf-8"))
    except (OSError, ValueError):
        return {}


def save_sonora_state(state: dict) -> None:
    import json
    _state_path().write_text(json.dumps(state, indent=2, sort_keys=True), "utf-8")


def installed() -> bool:
    return by_id(CAMPAIGN_ID).installed()


def resolve_sonora_download() -> tuple[str, str, str]:
    """Return (url, filename, version) for Sonora's latest English release zip (not the
    macOS .dmg also published there)."""
    import json
    import urllib.request

    api_url = f"https://api.github.com/repos/{SONORA_REPO}/releases/latest"
    with urllib.request.urlopen(api_url, timeout=20) as resp:
        data = json.load(resp)
    for a in data.get("assets", []):
        if a["name"].lower().endswith(".zip"):
            return a["browser_download_url"], a["name"], str(data.get("tag_name", ""))
    raise RuntimeError("No Sonora release zip found.")


def _fission_asset_for_platform() -> tuple[str, str]:
    import platform
    by_arch = _FISSION_ASSETS.get(sys.platform, {})
    machine = platform.machine()
    try:
        return by_arch[machine]
    except KeyError:
        raise RuntimeError(f"No native Sonora/FISSION engine build for this platform/CPU "
                            f"({sys.platform}/{machine}) - install fails, since the release's own "
                            f"bundled engine binary is Windows-only.")


def _should_skip(parts: list[str]) -> bool:
    if not parts or parts[0] in _SKIP_TOP_DIRS:
        return True
    name = parts[-1]
    if name.lower() in _SKIP_NAMES or name.lower().endswith(".ds_store"):
        return True
    # The nested per-platform engine zips (fallout-sonora-linux-x64.zip, ...android.zip,
    # ...ipa.zip) - superseded by fetching a native build from FISSION_REPO instead.
    if len(parts) == 1 and re.match(r"fallout-sonora.*\.zip$", name, re.IGNORECASE):
        return True
    return False


def _fetch_fission_binary(root: Path) -> None:
    """Fetch a native engine binary for this platform from FISSION_REPO's latest release
    and place it in `root`, under the name campaigns.py expects to find."""
    import json
    import shutil
    import tempfile
    import urllib.request
    import zipfile

    asset_name, bin_name = _fission_asset_for_platform()
    api_url = f"https://api.github.com/repos/{FISSION_REPO}/releases/latest"
    with urllib.request.urlopen(api_url, timeout=20) as resp:
        data = json.load(resp)
    asset_url = next((a["browser_download_url"] for a in data.get("assets", [])
                       if a["name"] == asset_name), None)
    if not asset_url:
        raise RuntimeError(f"No {asset_name} asset found in the latest FISSION release.")
    with tempfile.TemporaryDirectory() as tmp:
        downloaded = Path(tmp) / asset_name
        with urllib.request.urlopen(asset_url, timeout=120) as resp, open(downloaded, "wb") as out:
            shutil.copyfileobj(resp, out)
        with zipfile.ZipFile(downloaded) as zf:
            binary = zf.read(bin_name)

    out_name = "fallout-fission.exe" if sys.platform == "win32" else "fallout-fission"
    out_path = root / out_name
    out_path.write_bytes(binary)
    if sys.platform != "win32":
        out_path.chmod(out_path.stat().st_mode | 0o111)


def install_sonora() -> None:
    """Download Sonora's release, extract the game (skipping dev/platform-bundle extras),
    then fetch a native engine binary from FISSION_REPO. Raises on any failure."""
    import shutil
    import tempfile
    import urllib.request
    import zipfile

    root = sonora_root()
    url, filename, version = resolve_sonora_download()
    with tempfile.TemporaryDirectory() as tmp:
        downloaded = Path(tmp) / filename
        with urllib.request.urlopen(url, timeout=600) as resp, open(downloaded, "wb") as out:
            shutil.copyfileobj(resp, out)
        with zipfile.ZipFile(downloaded) as zf:
            top_level = {n.split("/", 1)[0] for n in zf.namelist() if n} - _SKIP_TOP_DIRS
            if len(top_level) != 1:
                raise RuntimeError(f"Sonora's release layout has changed (expected one top-level "
                                   f"folder besides {_SKIP_TOP_DIRS}, found {sorted(top_level)}) - "
                                   "update sonora_install.py's assumptions.")
            prefix = next(iter(top_level)) + "/"
            copied = 0
            for info in zf.infolist():
                if info.is_dir() or not info.filename.startswith(prefix):
                    continue
                parts = info.filename[len(prefix):].split("/")
                if _should_skip(parts):
                    continue
                dest = ci_path("/".join(parts), root=root)
                dest.parent.mkdir(parents=True, exist_ok=True)
                with zf.open(info) as src, open(dest, "wb") as out:
                    shutil.copyfileobj(src, out)
                copied += 1
            if not copied:
                raise RuntimeError("Sonora archive layout has changed - no recognized files found")

    _fetch_fission_binary(root)
    save_sonora_state({"version": version})
