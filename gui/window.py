"""The main window: sidebar navigation, search, and the bottom save/discard/play bar."""

from __future__ import annotations

from PySide6.QtCore import Qt
from PySide6.QtWidgets import (QFrame, QHBoxLayout, QLabel, QLineEdit, QListWidget, QListWidgetItem,
                               QMainWindow, QMessageBox, QPushButton, QStackedWidget, QVBoxLayout, QWidget)

from backend.mod_catalog import CatalogEntry
from backend.paths import GAME_BIN
from backend.schema import Catalog, Page

from .launcher_page import LauncherPage
from .mods_page import ModsPage
from .play_page import PlayPage
from .settings_page import SettingsPage
from .store import Store
from .style import AMBER


class Window(QMainWindow):
    def __init__(self, store: Store, catalog: Catalog, mod_catalog: list[CatalogEntry]):
        super().__init__()
        self.store = store
        self.setWindowTitle("Fallout 2 Launcher")
        self.resize(1180, 820)
        self.result: tuple[str, int | None] | None = None
        root = QWidget()
        root.setObjectName("root")
        self.setCentralWidget(root)
        outer = QVBoxLayout(root)
        outer.setContentsMargins(0, 0, 0, 0)
        outer.setSpacing(0)
        main = QHBoxLayout()
        main.setSpacing(0)
        outer.addLayout(main, 1)

        side = QFrame()
        side.setObjectName("sidebar")
        side.setFixedWidth(250)
        sl = QVBoxLayout(side)
        sl.setContentsMargins(0, 22, 0, 16)
        sl.setSpacing(6)
        title = QLabel("FALLOUT 2")
        title.setObjectName("title")
        title.setContentsMargins(22, 0, 12, 0)
        subtitle = QLabel(" · ".join(["Community Edition", *catalog.headlines]))
        subtitle.setObjectName("subtitle")
        subtitle.setContentsMargins(24, 0, 12, 8)
        subtitle.setWordWrap(True)
        sl.addWidget(title)
        sl.addWidget(subtitle)
        self.search = QLineEdit()
        self.search.setPlaceholderText("Search settings…")
        self.search.setClearButtonEnabled(True)
        box = QWidget()
        bl = QVBoxLayout(box)
        bl.setContentsMargins(14, 6, 14, 6)
        bl.addWidget(self.search)
        sl.addWidget(box)
        self.nav = QListWidget()
        self.nav.setObjectName("nav")
        sl.addWidget(self.nav, 1)
        main.addWidget(side)

        self.stack = QStackedWidget()
        main.addWidget(self.stack, 1)

        self.play = PlayPage()
        self.play.double_clicked = self.launch
        self.pages: list[tuple[str, QWidget]] = [("Play", self.play)]
        launcher_page = None
        for page in catalog.pages:
            if page.name == "Launcher":
                launcher_page = page
                continue
            self.pages.append((page.name, SettingsPage(page.name, page.groups, page.intro, store, catalog)))
        self.mods = ModsPage(store, mod_catalog)
        store.extras.append(self.mods)
        self.pages.append(("Mods", self.mods))
        self.pages.append(("Launcher", LauncherPage(launcher_page or Page("Launcher"), store, catalog)))
        for name, page in self.pages:
            self.nav.addItem(QListWidgetItem(name))
            self.stack.addWidget(page)
        self.nav.currentRowChanged.connect(self.stack.setCurrentIndex)
        self.nav.setCurrentRow(0)
        self.search.textChanged.connect(self.apply_search)

        bar = QFrame()
        bar.setObjectName("bottombar")
        bl2 = QHBoxLayout(bar)
        bl2.setContentsMargins(20, 12, 20, 12)
        self.status = QLabel()
        self.status.setObjectName("muted")
        self.discard = QPushButton("Discard changes")
        self.save_btn = QPushButton("Save")
        self.play_btn = QPushButton()
        self.play_btn.setObjectName("primary")
        self.play_btn.setCursor(Qt.CursorShape.PointingHandCursor)
        bl2.addWidget(self.status, 1)
        bl2.addWidget(self.discard)
        bl2.addWidget(self.save_btn)
        bl2.addSpacing(10)
        bl2.addWidget(self.play_btn)
        outer.addWidget(bar)

        self.discard.clicked.connect(store.revert)
        self.save_btn.clicked.connect(self.save)
        self.play_btn.clicked.connect(self.launch)
        store.dirty_changed.connect(self.update_status)
        self.play.selection_changed.connect(self.update_play)
        self.update_status()
        self.update_play()
        self.nav.setFocus()

    def apply_search(self, text: str):
        text = text.strip().lower()
        first_hit = None
        for i, (name, page) in enumerate(self.pages):
            item = self.nav.item(i)
            if isinstance(page, SettingsPage):
                hits = page.apply_filter(text)
                item.setText(f"{name}  ({hits})" if text else name)
                disabled = bool(text) and hits == 0
                item.setFlags(item.flags() & ~Qt.ItemFlag.ItemIsEnabled if disabled
                              else item.flags() | Qt.ItemFlag.ItemIsEnabled)
                if hits and first_hit is None:
                    first_hit = i
            else:
                match = bool(text) and text in name.lower()
                item.setFlags(item.flags() & ~Qt.ItemFlag.ItemIsEnabled if text and not match
                              else item.flags() | Qt.ItemFlag.ItemIsEnabled)
                if match and first_hit is None:
                    first_hit = i
        current = self.nav.currentRow()
        if text and first_hit is not None and not (self.nav.item(current).flags() & Qt.ItemFlag.ItemIsEnabled):
            self.nav.setCurrentRow(first_hit)

    def update_status(self):
        n = self.store.dirty_count()
        self.status.setText(f"● {n} unsaved change{'s' if n != 1 else ''}" if n else "All changes saved")
        self.status.setStyleSheet(f"color: {AMBER};" if n else "")
        self.discard.setEnabled(bool(n))
        self.save_btn.setEnabled(bool(n))

    def update_play(self):
        self.play_btn.setText(self.play.play_label())

    def save(self) -> bool:
        err = self.store.save()
        if err:
            QMessageBox.critical(self, "Couldn't save settings", err)
            return False
        self.update_status()
        return True

    def launch(self):
        if self.store.dirty_count() and not self.save():
            return
        if not GAME_BIN.is_file():
            QMessageBox.critical(self, "Game not found", f"{GAME_BIN} is missing.")
            return
        self.result = (self.play.mode, self.play.slot)
        self.close()

    def closeEvent(self, ev):
        if self.result is None and self.store.dirty_count():
            box = QMessageBox(self)
            box.setWindowTitle("Unsaved changes")
            box.setText("Save your changes before closing?")
            box.setStandardButtons(QMessageBox.StandardButton.Save | QMessageBox.StandardButton.Discard
                                   | QMessageBox.StandardButton.Cancel)
            choice = box.exec()
            if choice == QMessageBox.StandardButton.Cancel:
                ev.ignore()
                return
            if choice == QMessageBox.StandardButton.Save and not self.save():
                ev.ignore()
                return
        ev.accept()
