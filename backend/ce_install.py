"""Installing Fallout 2 CE itself: detecting a vanilla game directory with no CE engine
yet, downloading the fallout2-ce/fallout2-ce fork's current build, and placing the engine
binary + ce.dat. Linux and Windows for now; macOS ships as a .dmg disk image, which needs
mounting rather than a simple archive extract, so it stays a manual install for now - see
launcher/README.md.

The fork ships a single continuously-overwritten prerelease (tag "continious", sic) rather
than versioned releases, so there's nothing to pin a version against - we just record when
the build we fetched was published, for display elsewhere in the launcher.
"""

from __future__ import annotations

import shutil
import sys
from pathlib import Path

from .paths import ci_path, game_bin, game_dir

CE_REPO = "fallout2-ce/fallout2-ce"
CE_RELEASE_TAG = "continious"

# (asset filename, archive kind, engine binary name inside it) by sys.platform + architecture.
_ASSETS_BY_PLATFORM: dict[str, dict[str, tuple[str, str, str]]] = {
    "linux": {
        "x86_64": ("fallout2-ce-linux-x64.tar.gz", "tar", "fallout2-ce"),
        "aarch64": ("fallout2-ce-linux-arm64.tar.gz", "tar", "fallout2-ce"),
        "armv7l": ("fallout2-ce-linux-armhf.tar.gz", "tar", "fallout2-ce"),
    },
    "win32": {
        "AMD64": ("fallout2-ce-windows-x64.zip", "zip", "fallout2-ce.exe"),
        "x86": ("fallout2-ce-windows-x86.zip", "zip", "fallout2-ce.exe"),
    },
}


def _ce_state_path() -> Path:
    return game_dir() / ".f2launcher-ce.json"


def load_ce_state() -> dict:
    """What the launcher last installed: the release's publish time and asset name, for
    display. Not needed for detection, which just checks game_bin() directly."""
    import json
    try:
        return json.loads(_ce_state_path().read_text("utf-8"))
    except (OSError, ValueError):
        return {}


def save_ce_state(state: dict) -> None:
    import json
    _ce_state_path().write_text(json.dumps(state, indent=2, sort_keys=True), "utf-8")


def looks_like_vanilla_install() -> bool:
    """True if this is a real Fallout 2 game directory (has the original data files),
    regardless of whether CE is installed yet."""
    return ci_path("master.dat").is_file() and ci_path("critter.dat").is_file()


def needs_ce_install() -> bool:
    """True when this is a real game directory that's just missing the CE engine - the
    case this module can offer to fix. Other platforms never need it here (no installer
    for them yet), and an unrelated/empty folder is left to the normal "not found" error."""
    if sys.platform not in _ASSETS_BY_PLATFORM:
        return False
    return looks_like_vanilla_install() and not game_bin().is_file()


def _asset_for_platform() -> tuple[str, str, str]:
    """Return (asset filename, archive kind, engine binary name) for this OS/CPU."""
    import platform

    by_arch = _ASSETS_BY_PLATFORM.get(sys.platform, {})
    machine = platform.machine()
    try:
        return by_arch[machine]
    except KeyError:
        raise RuntimeError(f"No Fallout 2 CE build available for this platform/CPU ({sys.platform}/{machine}).")


def resolve_ce_download() -> tuple[str, str, str]:
    """Return (url, asset_filename, published_at) for the current release build."""
    import json
    import urllib.request

    asset_name, _kind, _bin_name = _asset_for_platform()
    api_url = f"https://api.github.com/repos/{CE_REPO}/releases/tags/{CE_RELEASE_TAG}"
    with urllib.request.urlopen(api_url, timeout=20) as resp:
        data = json.load(resp)
    for a in data.get("assets", []):
        if a["name"] == asset_name:
            return a["browser_download_url"], a["name"], data.get("published_at", "")
    raise RuntimeError(f"No {asset_name} asset found in the latest fallout2-ce release.")


def _extract_members(archive: Path, kind: str, wanted: set[str]) -> dict[str, bytes]:
    """Pull `wanted` top-level files (e.g. {"ce.dat", "fallout2-ce.exe"}) out of a release
    archive that wraps everything in one directory, regardless of tar vs. zip."""
    found: dict[str, bytes] = {}
    if kind == "tar":
        import tarfile
        with tarfile.open(archive) as tf:
            for member in tf.getmembers():
                parts = [p for p in member.name.split("/") if p]
                if len(parts) == 2 and parts[1] in wanted:
                    src = tf.extractfile(member)
                    if src is not None:
                        found[parts[1]] = src.read()
    else:
        import zipfile
        with zipfile.ZipFile(archive) as zf:
            for info in zf.infolist():
                parts = [p for p in info.filename.replace("\\", "/").split("/") if p]
                if len(parts) == 2 and parts[1] in wanted:
                    found[parts[1]] = zf.read(info)
    return found


def install_ce() -> None:
    """Download and place the fallout2-ce binary + ce.dat in the active campaign's root.
    Raises on any failure. Never touches any existing vanilla files - only adds the CE
    engine files."""
    import tempfile
    import urllib.request

    _asset_name, kind, bin_name = _asset_for_platform()
    url, filename, published_at = resolve_ce_download()
    wanted = {bin_name, "ce.dat"}
    with tempfile.TemporaryDirectory() as tmp:
        downloaded = Path(tmp) / filename
        with urllib.request.urlopen(url, timeout=120) as resp, open(downloaded, "wb") as out:
            shutil.copyfileobj(resp, out)
        members = _extract_members(downloaded, kind, wanted)
        missing = wanted - members.keys()
        if missing:
            raise RuntimeError(f"Release archive is missing: {', '.join(sorted(missing))}")
        for name, data in members.items():
            (game_dir() / name).write_bytes(data)
    bin_path = game_dir() / bin_name
    if sys.platform != "win32":
        bin_path.chmod(bin_path.stat().st_mode | 0o111)
    save_ce_state({"published_at": published_at, "asset": filename})
