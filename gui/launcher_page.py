"""The Launcher page: its own settings (extends SettingsPage) plus file shortcuts and install info."""

from __future__ import annotations

from PySide6.QtCore import Qt, QUrl
from PySide6.QtGui import QDesktopServices, QGuiApplication
from PySide6.QtWidgets import QGridLayout, QHBoxLayout, QLabel, QLineEdit, QPushButton

from backend.ce_install import load_ce_state, needs_ce_install
from backend.launch_option import steam_launch_option
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
        if GAME_BIN.is_file():
            engine = f"Fallout 2 Community Edition ({GAME_BIN.name})"
            published = load_ce_state().get("published_at")
            if published:
                engine += f" · installed build from {published[:10]}"
        elif needs_ce_install():
            engine = "Not installed yet — press Play to download and install it"
        else:
            engine = f"Not found ({GAME_BIN.name} is missing)"
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

        frame3, inner3 = card("Steam launch options")
        intro3 = QLabel(
            "Paste this into the game's Properties → General → Launch Options in Steam, so "
            "pressing Play opens this launcher instead of the game directly.")
        intro3.setWordWrap(True)
        intro3.setObjectName("muted")
        intro3.setContentsMargins(16, 0, 16, 8)
        inner3.addWidget(intro3)
        option_text, verified = steam_launch_option()
        row3 = QHBoxLayout()
        row3.setContentsMargins(16, 0, 16, 4 if not verified else 12)
        field = QLineEdit(option_text)
        field.setReadOnly(True)
        field.setCursorPosition(0)
        copy_btn = QPushButton("Copy")
        copy_btn.clicked.connect(lambda: QGuiApplication.clipboard().setText(option_text))
        row3.addWidget(field, 1)
        row3.addWidget(copy_btn)
        inner3.addLayout(row3)
        if not verified:
            warn = QLabel(
                "Unverified on Windows: Steam may run this and still start the game "
                "afterward too, rather than replacing it. Please confirm and adjust if needed.")
            warn.setObjectName("note")
            warn.setWordWrap(True)
            warn.setContentsMargins(16, 4, 16, 12)
            inner3.addWidget(warn)
        self.lay.insertWidget(self.lay.count() - 1, frame3)
