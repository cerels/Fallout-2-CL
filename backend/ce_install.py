"""Installing Fallout 2 CE itself: detecting a vanilla game directory with no CE engine
yet, downloading the fallout2-ce/fallout2-ce fork's current build, and placing the engine
binary + ce.dat. Linux only for now - see launcher/README.md for other platforms.

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

_LINUX_ASSET_BY_ARCH = {
    "x86_64": "fallout2-ce-linux-x64.tar.gz",
    "aarch64": "fallout2-ce-linux-arm64.tar.gz",
    "armv7l": "fallout2-ce-linux-armhf.tar.gz",
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
    if sys.platform != "linux":
        return False
    return looks_like_vanilla_install() and not game_bin().is_file()


def _linux_asset_name() -> str:
    import platform
    machine = platform.machine()
    try:
        return _LINUX_ASSET_BY_ARCH[machine]
    except KeyError:
        raise RuntimeError(f"No Fallout 2 CE build available for this CPU architecture ({machine}).")


def resolve_ce_download() -> tuple[str, str, str]:
    """Return (url, asset_filename, published_at) for the current release build."""
    import json
    import urllib.request

    asset_name = _linux_asset_name()
    api_url = f"https://api.github.com/repos/{CE_REPO}/releases/tags/{CE_RELEASE_TAG}"
    with urllib.request.urlopen(api_url, timeout=20) as resp:
        data = json.load(resp)
    for a in data.get("assets", []):
        if a["name"] == asset_name:
            return a["browser_download_url"], a["name"], data.get("published_at", "")
    raise RuntimeError(f"No {asset_name} asset found in the latest fallout2-ce release.")


def install_ce() -> None:
    """Download and place the fallout2-ce binary + ce.dat in the active campaign's root.
    Raises on any failure. Never touches any existing vanilla files - only adds the CE
    engine files."""
    import tarfile
    import tempfile
    import urllib.request

    if sys.platform != "linux":
        raise RuntimeError("Automatic Fallout 2 CE install is only supported on Linux right now.")

    url, filename, published_at = resolve_ce_download()
    with tempfile.TemporaryDirectory() as tmp:
        downloaded = Path(tmp) / filename
        with urllib.request.urlopen(url, timeout=120) as resp, open(downloaded, "wb") as out:
            shutil.copyfileobj(resp, out)
        with tarfile.open(downloaded) as tf:
            members = {}
            for member in tf.getmembers():
                parts = [p for p in member.name.split("/") if p]
                if len(parts) == 2 and parts[1] in ("fallout2-ce", "ce.dat"):
                    members[parts[1]] = member
            missing = {"fallout2-ce", "ce.dat"} - members.keys()
            if missing:
                raise RuntimeError(f"Release archive is missing: {', '.join(sorted(missing))}")
            for name, member in members.items():
                src = tf.extractfile(member)
                if src is None:
                    raise RuntimeError(f"Could not read {name} from the release archive")
                with open(game_dir() / name, "wb") as out:
                    shutil.copyfileobj(src, out)
    bin_path = game_dir() / "fallout2-ce"
    bin_path.chmod(bin_path.stat().st_mode | 0o111)
    save_ce_state({"published_at": published_at, "asset": filename})
