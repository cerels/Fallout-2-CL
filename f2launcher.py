#!/usr/bin/env python3
"""Fallout 2 CE launcher.

A settings UI for every option that otherwise lives in config files
(fallout2.cfg, data/config/game#patch.cfg, mods/*.ini), a save game picker,
and a mod load-order editor. "Play" saves pending changes and replaces this
process with fallout2-ce, so Steam keeps tracking the game.

Started by launcher/launch.sh (the Steam launch option).
  --ui      always show the launcher window
  --no-ui   skip the window and start the game right away
  --campaign=<id>   force a campaign (see backend/campaigns.py), overriding
                    "remember last campaign" for this one launch
  --check-schemas   validate the settings schemas and list the generated tabs
"""

from __future__ import annotations

import sys
from pathlib import Path

# -I (isolated mode, used by launch.sh) excludes the script's own directory from
# sys.path, unlike a normal invocation - add it back so `backend` is importable
# regardless of how this script is started or what the current directory is.
sys.path.insert(0, str(Path(__file__).resolve().parent))

from backend.campaigns import activate, by_id, forced_campaign_id, load_last_campaign_id, save_last_campaign_id
from backend.ini_file import IniFile
from backend.launch import exec_game, game_args, launcher_prefs
from backend.mod_catalog import load_mod_catalog
from backend.paths import ci_path
from backend.schema import check_schemas, load_catalog

# ---------------------------------------------------------------------------
# GUI
# ---------------------------------------------------------------------------

def run_gui(prefs: IniFile) -> tuple[str, int | None] | None:
    from PySide6.QtGui import QIcon
    from PySide6.QtWidgets import QApplication

    from gui.store import Store
    from gui.style import STYLESHEET
    from gui.window import Window

    app = QApplication.instance() or QApplication(sys.argv)
    app.setApplicationName("Fallout 2 Launcher")
    app.setDesktopFileName("fallout2-ce-launcher")
    icon_path = ci_path("fallout2.ico")
    if icon_path.is_file():
        app.setWindowIcon(QIcon(str(icon_path)))
    app.setStyleSheet(STYLESHEET)

    catalog = load_catalog()
    for err in catalog.errors:
        print(f"schema error: {err}", file=sys.stderr)
    mod_catalog = load_mod_catalog()
    store = Store(catalog)

    win = Window(store, catalog, mod_catalog)
    win.show()
    app.exec()
    return win.result


# ---------------------------------------------------------------------------

def main() -> None:
    if "--check-schemas" in sys.argv:
        sys.exit(check_schemas())
    prefs = launcher_prefs()

    forced_id = forced_campaign_id(sys.argv)
    campaign_id = forced_id or load_last_campaign_id(prefs)
    campaign = by_id(campaign_id) or by_id("ce")
    activate(campaign)
    if forced_id is None:
        save_last_campaign_id(prefs, campaign.id)

    show = (prefs.get("launcher", "show_launcher") or "1") != "0"
    if "--ui" in sys.argv:
        show = True
    if "--no-ui" in sys.argv:
        show = False

    if show:
        try:
            result = run_gui(prefs)
        except ImportError as exc:
            print(f"Launcher UI unavailable ({exc}); starting the game directly.", file=sys.stderr)
            result = ((prefs.get("launcher", "on_start") or "continue"), None)
        if result is None:
            return
        mode, slot = result
    else:
        mode, slot = (prefs.get("launcher", "on_start") or "continue"), None
    exec_game(game_args(mode, slot))


if __name__ == "__main__":
    main()
