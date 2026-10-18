"""Installing RPU (Restoration Project Updated): a content overlay for the Vanilla/CE
campaign, not a separate campaign - see GLOSSARY.md and schemas/mods/restoration-project.toml,
which already describes its settings once installed.

The official release zip (github.com/BGforgeNet/Fallout2_Restoration_Project) is a full
classic-sfall install package meant to be unzipped by hand onto a vanilla Fallout 2 copy:
alongside RPU's own content it bundles fallout2.exe, ddraw.dll/.ini, sfall.dat, f2_res's
own files, its own mods_order.txt, and - deliberately, this is the officially-tested
combination - NPC Armor, Party Orders and the Unofficial Patch's settings. None of the
classic-engine files belong on a CE install, and blindly extracting mods_order.txt would
overwrite whatever load order the user already has, but everything under mods/ itself
(RPU's own dat files plus the bundled NPC Armor/Party Orders/Unofficial Patch/ammo-mod
configs) is exactly what's needed for those mods' own bundled schemas
(schemas/mods/npc-armor.toml, party-orders.toml, unofficial-patch.toml, sfall-mods.toml)
to find their files and show up - so it's all extracted, merged into the existing mods/
setup via the same enable_mod_file() helper the regular mod catalog uses.

RPU also needs ddraw.ini's BoostScriptDialogLimit raised from its shipped default (0) to 1 -
without it, fallout2-ce throws "BoostScriptDialogLimit not enabled" and kicks you back to
the main menu the moment a save touches RPU's restored content, which adds far more
dialog/script names than vanilla's 1450-name limit. install_rpu() sets it directly.
"""

from __future__ import annotations

import shutil
import zipfile
from pathlib import Path

from .ini_file import IniFile
from .mod_catalog import delete_mod_files, enable_mod_file
from .paths import ci_path, mod_installed

RPU_REPO = "BGforgeNet/Fallout2_Restoration_Project"

# RPU's own optional content dat files, beyond the core rpu.dat - each is an individually
# togglable extra on the regular Mods page once installed (see restoration-project.toml).
OPTIONAL_DAT_FILES = (
    "rpu_enhanced_worldmap.dat", "rpu_extended_flamer.dat", "rpu_rifle_animations.dat",
    "rpu_wakizashi_animations.dat", "cassidy_head.dat", "cassidy_voice_joey_bracken_hq.dat",
    "rpu_improved_mysterious_stranger.dat", "rpu_alternative_explosions.dat",
)


def _rpu_state_path() -> Path:
    return ci_path("mods/.f2launcher-rpu.json")


def load_rpu_state() -> dict:
    """The version last installed, for update checks - not needed for detection, which
    just checks mod_installed("rpu.dat") directly."""
    import json
    try:
        return json.loads(_rpu_state_path().read_text("utf-8"))
    except (OSError, ValueError):
        return {}


def save_rpu_state(state: dict) -> None:
    import json
    path = _rpu_state_path()
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(state, indent=2, sort_keys=True), "utf-8")


def installed() -> bool:
    return mod_installed("rpu.dat")


def resolve_rpu_download() -> tuple[str, str, str]:
    """Return (url, filename, version) for RPU's current release zip - the main one, not
    the separate *_translations.zip (this launcher is English-only for now)."""
    import json
    import urllib.request

    api_url = f"https://api.github.com/repos/{RPU_REPO}/releases/latest"
    with urllib.request.urlopen(api_url, timeout=20) as resp:
        data = json.load(resp)
    for a in data.get("assets", []):
        name = a["name"]
        if name.lower().endswith(".zip") and "translations" not in name.lower():
            return a["browser_download_url"], name, str(data.get("tag_name", ""))
    raise RuntimeError("No RPU release zip found.")


def check_for_rpu_update() -> str | None:
    """Latest version if newer than what's installed, else None. Mirrors
    mod_catalog.check_for_update: the first check just records a baseline."""
    try:
        _, _, version = resolve_rpu_download()
    except Exception:
        return None  # offline, rate-limited, etc. - just skip silently
    if not version:
        return None
    state = load_rpu_state()
    baseline = state.get("version")
    if baseline is None:
        save_rpu_state({**state, "version": version})
        return None
    return version if version != baseline else None


def _wanted_dest(parts: list[str]) -> Path | None:
    """Where to extract a zip entry's parts, or None to skip it - see module docstring
    for why most of the archive is skipped. Everything directly under mods/ is wanted
    except mods_order.txt itself, which must never overwrite the user's own load order."""
    if parts[0] == "appearance" or parts[:2] == ["data", "sound"]:
        return ci_path("/".join(parts))
    if parts[0] == "mods" and len(parts) == 2 and parts[1].lower() != "mods_order.txt":
        return ci_path(f"mods/{parts[1]}")
    return None


def _enable_boost_script_dialog_limit() -> None:
    """See module docstring - required for RPU's restored content to work at all."""
    path = ci_path("ddraw.ini")
    if not path.is_file():
        return
    ini = IniFile(path)
    if ini.get("Misc", "BoostScriptDialogLimit") != "1":
        ini.set("Misc", "BoostScriptDialogLimit", "1")
        ini.save()


def install_rpu() -> None:
    """Download RPU's current release and extract its content (including the mods it
    officially bundles - NPC Armor, Party Orders, Unofficial Patch settings). Raises on
    any failure; never touches mods_order.txt beyond enabling rpu.dat itself."""
    import tempfile
    import urllib.request

    url, filename, version = resolve_rpu_download()
    with tempfile.TemporaryDirectory() as tmp:
        downloaded = Path(tmp) / filename
        with urllib.request.urlopen(url, timeout=600) as resp, open(downloaded, "wb") as out:
            shutil.copyfileobj(resp, out)
        with zipfile.ZipFile(downloaded) as zf:
            copied = 0
            for info in zf.infolist():
                parts = [p for p in info.filename.replace("\\", "/").split("/") if p]
                if info.is_dir() or not parts:
                    continue
                dest = _wanted_dest(parts)
                if dest is None:
                    continue
                dest.parent.mkdir(parents=True, exist_ok=True)
                with zf.open(info) as src, open(dest, "wb") as out:
                    shutil.copyfileobj(src, out)
                copied += 1
            if not copied:
                raise RuntimeError("RPU archive layout has changed - no recognized files found")
    enable_mod_file("rpu.dat")
    _enable_boost_script_dialog_limit()
    save_rpu_state({"version": version})


def delete_rpu() -> list[Path]:
    """Remove RPU's own mods/ files (not appearance/ or the restored music - just extra
    data, harmless to leave, and re-installing overwrites them anyway) and drop rpu.dat
    from mods_order.txt."""
    removed = delete_mod_files("rpu.dat", [*OPTIONAL_DAT_FILES, "rpu.ini"])
    state_path = _rpu_state_path()
    if state_path.is_file():
        state_path.unlink()
    return removed
