"""Campaigns: the games this launcher can run and manage. Vanilla/CE is rooted at the
launcher's own install folder; every other campaign is a self-contained subfolder
underneath it (see GLOSSARY.md and docs/adr/0001-campaign-root-is-a-subfolder.md).
Installing/managing each campaign's own files is a separate module per campaign; this one
only knows where they live, which is installed, and which one is active right now.
"""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path

from .ini_file import IniFile
from .paths import LAUNCHER_DIR, set_campaign_root

DEFAULT_ID = "ce"


@dataclass(frozen=True)
class Campaign:
    id: str
    name: str
    subdir: str  # "" for Vanilla/CE itself; relative to LAUNCHER_DIR otherwise
    bin_names: tuple[str, ...]  # candidate engine binary filenames, checked in order
    badge: str  # short placeholder shown at the campaign switcher until real artwork exists
    accent: str  # badge color (hex)

    @property
    def root(self) -> Path:
        return LAUNCHER_DIR / self.subdir if self.subdir else LAUNCHER_DIR

    def installed(self) -> bool:
        return any((self.root / name).is_file() for name in self.bin_names)


CAMPAIGNS: list[Campaign] = [
    Campaign("ce", "Fallout 2", "", ("fallout2-ce", "fallout2-ce.exe"), "F2", "#5dfc8a"),
    Campaign("ettu", "Fallout et Tu", "Fallout1in2", ("fallout2-ce", "fallout2-ce.exe", "Fallout2.exe"),
             "ET", "#5d9dfc"),
    Campaign("sonora", "Fallout: Sonora", "Sonora", ("fallout-fission", "fallout-fission.exe"),
             "SO", "#ffb84d"),
    Campaign("nevada", "Fallout 2: Nevada", "Nevada", ("fallout2-ce", "fallout2-ce.exe"), "NV", "#ff7a5d"),
]


def by_id(campaign_id: str) -> Campaign | None:
    return next((c for c in CAMPAIGNS if c.id == campaign_id), None)


def installed_campaigns() -> list[Campaign]:
    return [c for c in CAMPAIGNS if c.installed()]


def active_campaign() -> Campaign:
    from .paths import game_dir
    root = game_dir()
    return next((c for c in CAMPAIGNS if c.root == root), by_id(DEFAULT_ID))


def forced_campaign_id(argv: list[str]) -> str | None:
    """--campaign=<id> on the command line - how a Steam Launch Options line (e.g. one
    pasted into Fallout 1's entry to force Et Tu) picks a campaign without going through
    "remember last campaign"."""
    for arg in argv:
        if arg.startswith("--campaign="):
            return arg.split("=", 1)[1]
    return None


def load_last_campaign_id(prefs: IniFile) -> str:
    return prefs.get("launcher", "last_campaign") or DEFAULT_ID


def save_last_campaign_id(prefs: IniFile, campaign_id: str) -> None:
    prefs.set("launcher", "last_campaign", campaign_id)
    prefs.save()


def activate(campaign: Campaign) -> None:
    set_campaign_root(campaign.root)
