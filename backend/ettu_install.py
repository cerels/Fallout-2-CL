"""Installing Fallout et Tu: a full, self-contained engine+content package that drops
into its own subfolder (Fallout1in2/) underneath the Vanilla/CE campaign's folder - see
GLOSSARY.md and docs/adr/0001-campaign-root-is-a-subfolder.md. Verified against the
actual release archive's central directory before writing this (see the design session):
unlike RPU's zip, everything in rotators/Fo1in2's release belongs together under one
top-level Fallout1in2/ folder, so the whole thing is extracted as-is - no allowlist needed.

Two things make Et Tu different from a plain CE install, though:

- The release only ships Windows binaries (fallout2-ce.exe, Fallout2.exe) - there's no
  native Linux/macOS engine in it. On Linux, install_ettu() fetches one the same way
  ce_install does for Vanilla/CE, just pointed at Et Tu's own root instead.

- Et Tu needs Fallout 1's own data, which this can never download on its own (that would
  mean distributing someone else's purchased game). The release bundles the official
  conversion tool (undat.sh/undat.exe) for pulling the handful of files it actually needs
  out of a Fallout 1 install's MASTER.DAT; extract_fallout1_data() runs it, but only once
  the user points it at a Fallout 1 copy they already own.
"""

from __future__ import annotations

import sys
from pathlib import Path

from .campaigns import by_id
from .paths import campaign_root_override, ci_path

ETTU_REPO = "rotators/Fo1in2"
CAMPAIGN_ID = "ettu"


def ettu_root() -> Path:
    return by_id(CAMPAIGN_ID).root


def _state_path() -> Path:
    return ettu_root() / ".f2launcher-ettu.json"


def load_ettu_state() -> dict:
    import json
    try:
        return json.loads(_state_path().read_text("utf-8"))
    except (OSError, ValueError):
        return {}


def save_ettu_state(state: dict) -> None:
    import json
    _state_path().write_text(json.dumps(state, indent=2, sort_keys=True), "utf-8")


def installed() -> bool:
    return by_id(CAMPAIGN_ID).installed()


def has_fallout1_data() -> bool:
    """True once extract_fallout1_data() has run successfully. There's no fully reliable
    way to detect this by file presence alone (undat's file list could change release to
    release), so this is our own bookkeeping, same as CE's/RPU's install state files."""
    return bool(load_ettu_state().get("fallout1_data_extracted"))


def resolve_ettu_download() -> tuple[str, str, str]:
    """Return (url, filename, version) for Et Tu's latest release zip."""
    import json
    import urllib.request

    api_url = f"https://api.github.com/repos/{ETTU_REPO}/releases/latest"
    with urllib.request.urlopen(api_url, timeout=20) as resp:
        data = json.load(resp)
    for a in data.get("assets", []):
        if a["name"].lower() == "fallout1in2.zip":
            return a["browser_download_url"], a["name"], str(data.get("tag_name", ""))
    raise RuntimeError("No Fallout1in2.zip asset found in the latest Et Tu release.")


def install_ettu() -> None:
    """Download Et Tu's release and extract it whole into its own subfolder, then (Linux
    only for now, same scope as CE itself) fetch a native engine binary into that same
    folder, since the release only ships Windows ones. Raises on any failure."""
    import shutil
    import tempfile
    import urllib.request
    import zipfile

    from .paths import LAUNCHER_DIR

    campaign = by_id(CAMPAIGN_ID)
    url, filename, version = resolve_ettu_download()
    with tempfile.TemporaryDirectory() as tmp:
        downloaded = Path(tmp) / filename
        with urllib.request.urlopen(url, timeout=600) as resp, open(downloaded, "wb") as out:
            shutil.copyfileobj(resp, out)
        with zipfile.ZipFile(downloaded) as zf:
            top_level = {n.split("/", 1)[0] for n in zf.namelist() if n}
            if top_level != {campaign.subdir}:
                raise RuntimeError(
                    f"Et Tu's release layout has changed (expected a single {campaign.subdir!r} "
                    f"folder, found {sorted(top_level)}) - update ettu_install.py's assumptions.")
            zf.extractall(LAUNCHER_DIR)

    if sys.platform == "linux":
        from .ce_install import install_ce
        with campaign_root_override(ettu_root()):
            install_ce()
    # Windows: the release's own fallout2-ce.exe/Fallout2.exe already run natively.
    # macOS: not automated yet, same as CE's own install (no native build fetched here).

    save_ettu_state({**load_ettu_state(), "version": version})


def extract_fallout1_data(master_dat: Path) -> None:
    """Run Et Tu's bundled undat.sh against a Fallout 1 install's MASTER.DAT - Linux/macOS
    only; on Windows the equivalent (UNDAT.EXE) is a GUI tool with no command-line form, so
    there's nothing to automate there. Raises RuntimeError with the tool's own error
    output on failure."""
    import subprocess

    if sys.platform == "win32":
        raise RuntimeError(
            "Automatic extraction isn't available on Windows - run UNDAT.EXE from the "
            "Fallout1in2 folder yourself and point it at Fallout 1's MASTER.DAT.")
    if not installed():
        raise RuntimeError("Et Tu isn't installed yet - install it first.")
    script = ci_path("undat.sh", root=ettu_root())
    if not script.is_file():
        raise RuntimeError("undat.sh not found in the Et Tu install - it may be a newer/older "
                            "release with a different layout than this launcher expects.")
    result = subprocess.run([str(script), str(master_dat)], cwd=str(ettu_root()),
                             capture_output=True, text=True, timeout=300)
    if result.returncode != 0:
        raise RuntimeError(result.stdout.strip() or result.stderr.strip() or "undat.sh failed")
    save_ettu_state({**load_ettu_state(), "fallout1_data_extracted": True})
