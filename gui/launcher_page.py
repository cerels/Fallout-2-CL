"""The Launcher page: its own settings (extends SettingsPage) plus file shortcuts and install info."""

from __future__ import annotations

from PySide6.QtCore import Qt, QUrl
from PySide6.QtGui import QDesktopServices
from PySide6.QtWidgets import QGridLayout, QLabel, QPushButton

from backend.paths import GAME_BIN, GAME_DIR, ci_path
from backend.schema import Catalog, Page

from .settings_page import SettingsPage
from .store import Store
from .widgets import card


class LauncherPage(SettingsPage):
    def __init__(self, page: Page, store: Store, catalog: Catalog):
        super().__init__(page.name, page.groups, page.intro, store, catalog)
        frame, inner = card("Files & folders")
        grid = QGridLayout()
        grid.setContentsMargins(16, 4, 16, 12)
        grid.setSpacing(8)
        links = [("Game folder", ""), ("Save games", "data/SAVEGAME"), ("Mods folder", "mods"),
                 ("Settings backups", "backup/launcher"), ("fallout2.cfg", "fallout2.cfg"),
                 ("game#patch.cfg", "data/config/game#patch.cfg")]
        for i, (label, rel) in enumerate(links):
            b = QPushButton(label)
            path = ci_path(rel) if rel else GAME_DIR
            b.clicked.connect(lambda _=False, p=path: QDesktopServices.openUrl(QUrl.fromLocalFile(str(p))))
            if not path.exists():
                b.setEnabled(False)
            grid.addWidget(b, i // 3, i % 3)
        inner.addLayout(grid)
        self.lay.insertWidget(self.lay.count() - 1, frame)

        frame2, inner2 = card("About this install")
        content = store.files.get("content")
        version = (content.get("main_menu", "version_string") if content else None) or "unknown"
        engine = f"Fallout 2 Community Edition ({GAME_BIN.name})"
        mods = ", ".join(catalog.headlines) or "none with a schema"
        info = QLabel(
            f"<b>Engine:</b> {engine}<br>"
            f"<b>Headline mods:</b> {mods}<br>"
            f"<b>Version string:</b> {version}<br>"
            f"<b>Folder:</b> {GAME_DIR}<br>"
            f"Every time the launcher saves, the previous config files are copied to backup/launcher/ first.")
        info.setWordWrap(True)
        info.setObjectName("muted")
        info.setTextInteractionFlags(Qt.TextInteractionFlag.TextSelectableByMouse)
        info.setContentsMargins(16, 4, 16, 12)
        inner2.addWidget(info)
        self.lay.insertWidget(self.lay.count() - 1, frame2)
