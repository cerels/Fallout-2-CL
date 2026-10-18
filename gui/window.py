"""The main window: campaign switcher, sidebar navigation, search, and the bottom
save/discard/play bar."""

from __future__ import annotations

import sys

from PySide6.QtCore import QThread, Qt, Signal
from PySide6.QtWidgets import (QFrame, QHBoxLayout, QLabel, QLineEdit, QListWidget, QListWidgetItem,
                               QMainWindow, QMenu, QMessageBox, QPushButton, QStackedWidget, QVBoxLayout,
                               QWidget)

from backend.campaigns import CAMPAIGNS, Campaign, activate, active_campaign, save_last_campaign_id
from backend.ce_install import install_ce, needs_ce_install
from backend.launch import launcher_prefs
from backend.mod_catalog import CatalogEntry, load_mod_catalog
from backend.paths import game_bin
from backend.schema import Catalog, Page, load_catalog

from .campaigns_page import CampaignsPage
from .launcher_page import LauncherPage
from .mods_page import ModsPage
from .play_page import PlayPage
from .rpu_page import RpuPage
from .settings_page import SettingsPage
from .store import Store
from .style import AMBER
from .widgets import ClickableWidget


class CEInstallThread(QThread):
    failed = Signal(str)
    done = Signal()

    def run(self):
        try:
            install_ce()
        except Exception as exc:  # surfaced to the UI thread via signal, never silent
            self.failed.emit(str(exc))
        else:
            self.done.emit()


class Window(QMainWindow):
    def __init__(self, store: Store, catalog: Catalog, mod_catalog: list[CatalogEntry]):
        super().__init__()
        self.setWindowTitle("Fallout 2 Launcher")
        self.resize(1180, 820)
        self.result: tuple[str, int | None] | None = None
        self._ce_thread: QThread | None = None
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

        self.campaign_header = ClickableWidget()
        self.campaign_header.setObjectName("campaignswitcher")
        self.campaign_header.setCursor(Qt.CursorShape.PointingHandCursor)
        self.campaign_header.clicked.connect(self._show_campaign_menu)
        hl = QVBoxLayout(self.campaign_header)
        hl.setContentsMargins(22, 2, 12, 4)
        hl.setSpacing(2)
        row = QHBoxLayout()
        row.setSpacing(8)
        self.campaign_badge = QLabel()
        self.campaign_badge.setFixedSize(30, 30)
        self.campaign_badge.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self.campaign_title = QLabel()
        self.campaign_title.setObjectName("title")
        caret = QLabel("▾")
        caret.setObjectName("title")
        row.addWidget(self.campaign_badge)
        row.addWidget(self.campaign_title)
        row.addWidget(caret)
        row.addStretch()
        hl.addLayout(row)
        self.subtitle = QLabel()
        self.subtitle.setObjectName("subtitle")
        self.subtitle.setContentsMargins(2, 0, 0, 0)
        self.subtitle.setWordWrap(True)
        hl.addWidget(self.subtitle)
        sl.addWidget(self.campaign_header)

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

        self.nav.currentRowChanged.connect(self.stack.setCurrentIndex)
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

        self.discard.clicked.connect(lambda: self.store.revert())
        self.save_btn.clicked.connect(self.save)
        self.play_btn.clicked.connect(self.launch)

        self.campaigns_page = CampaignsPage()
        self.campaigns_page.campaign_switch_requested.connect(self.switch_campaign)

        self._build_pages(store, catalog, mod_catalog)
        self._update_campaign_header()
        self.nav.setFocus()

    # ---- (re)building the page set for whichever campaign is active ----

    def _build_pages(self, store: Store, catalog: Catalog, mod_catalog: list[CatalogEntry]):
        self.store = store
        self.catalog = catalog

        # Preserve whichever page the user was on (e.g. still on the RPU tab right after
        # installing it) rather than always bouncing back to Play.
        current_name = None
        if getattr(self, "pages", None) and 0 <= self.nav.currentRow() < len(self.pages):
            current_name = self.pages[self.nav.currentRow()][0]

        self.nav.clear()
        while self.stack.count():
            w = self.stack.widget(0)
            self.stack.removeWidget(w)
            # campaigns_page is persistent across switches (it's campaign-agnostic) -
            # only reparented out and back in, never destroyed.
            if w is not self.campaigns_page:
                w.deleteLater()

        self.play = PlayPage()
        self.play.double_clicked = self.launch
        self.play.selection_changed.connect(self.update_play)
        self.pages: list[tuple[str, QWidget]] = [("Play", self.play), ("Campaigns", self.campaigns_page)]
        launcher_page = None
        rpu_page = None
        for page in catalog.pages:
            if page.name == "Launcher":
                launcher_page = page
                continue
            if page.name == "Restoration Project":
                rpu_page = page
                continue
            self.pages.append((page.name, SettingsPage(page.name, page.groups, page.intro, store, catalog)))
        rpu_widget = RpuPage(rpu_page or Page("Restoration Project"), store, catalog)
        rpu_widget.needs_reload.connect(self.reload_pages)
        self.pages.append(("Restoration Project", rpu_widget))
        self.mods = ModsPage(store, mod_catalog)
        self.mods.needs_reload.connect(self.reload_pages)
        store.extras.append(self.mods)
        self.pages.append(("Mods", self.mods))
        self.pages.append(("Launcher", LauncherPage(launcher_page or Page("Launcher"), store, catalog)))
        for name, page in self.pages:
            self.nav.addItem(QListWidgetItem(name))
            self.stack.addWidget(page)
        self.nav.setCurrentRow(self._page_row(current_name) if current_name else 0)

        store.dirty_changed.connect(self.update_status)
        self.campaigns_page.refresh()
        self.update_status()
        self.update_play()

    def _page_row(self, name: str) -> int:
        return next((i for i, (n, _) in enumerate(self.pages) if n == name), 0)

    def reload_pages(self):
        """Re-read the settings catalog for the currently active campaign (no campaign
        change) - installing or deleting a mod/RPU can change which schema-backed
        settings pages exist, so the page set needs rebuilding to show that live."""
        if not self._confirm_unsaved():
            return
        catalog = load_catalog()
        for err in catalog.errors:
            print(f"schema error: {err}", file=sys.stderr)
        self._build_pages(Store(catalog), catalog, load_mod_catalog())

    def _update_campaign_header(self):
        c = active_campaign()
        self.campaign_badge.setText(c.badge)
        self.campaign_badge.setStyleSheet(
            f"background:{c.accent}; color:#06210f; border-radius:7px; font-weight:800; font-size:10.5pt;")
        self.campaign_title.setText(c.name.upper())

    def _show_campaign_menu(self):
        menu = QMenu(self)
        active = active_campaign()
        for c in CAMPAIGNS:
            if c.installed():
                action = menu.addAction(f"{c.badge}  {c.name}")
                action.setCheckable(True)
                action.setChecked(c.id == active.id)
                if c.id != active.id:
                    action.triggered.connect(lambda _=False, camp=c: self.switch_campaign(camp))
            else:
                action = menu.addAction(f"{c.badge}  {c.name} (not installed)")
                action.setEnabled(False)
        menu.addSeparator()
        manage = menu.addAction("Manage campaigns…")
        manage.triggered.connect(lambda: self.nav.setCurrentRow(self._page_row("Campaigns")))
        menu.exec(self.campaign_header.mapToGlobal(self.campaign_header.rect().bottomLeft()))

    def switch_campaign(self, campaign: Campaign):
        if not self._confirm_unsaved():
            return
        activate(campaign)
        save_last_campaign_id(launcher_prefs(), campaign.id)
        catalog = load_catalog()
        for err in catalog.errors:
            print(f"schema error: {err}", file=sys.stderr)
        store = Store(catalog)
        self._build_pages(store, catalog, load_mod_catalog())
        self._update_campaign_header()

    # ---- search ----

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

    # ---- status / play button ----

    def update_status(self):
        n = self.store.dirty_count()
        self.status.setText(f"● {n} unsaved change{'s' if n != 1 else ''}" if n else "All changes saved")
        self.status.setStyleSheet(f"color: {AMBER};" if n else "")
        self.discard.setEnabled(bool(n))
        self.save_btn.setEnabled(bool(n))

    def update_play(self):
        active = active_campaign()
        if active.id == "ce":
            install_needed = needs_ce_install()
            subtitle = "Community Edition — not installed" if install_needed else \
                " · ".join(["Community Edition", *self.catalog.headlines])
        else:
            install_needed = not active.installed()
            subtitle = "Not installed — use the Campaigns page" if install_needed else \
                " · ".join([active.name, *self.catalog.headlines])
        self.play_btn.setText("Install CE" if install_needed and active.id == "ce"
                              else ("Go to Campaigns" if install_needed else self.play.play_label()))
        self.play_btn.setProperty("install", install_needed)
        self.play_btn.style().unpolish(self.play_btn)
        self.play_btn.style().polish(self.play_btn)
        self.subtitle.setText(subtitle)
        self.subtitle.setProperty("warn", install_needed)
        self.subtitle.style().unpolish(self.subtitle)
        self.subtitle.style().polish(self.subtitle)

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
        if not game_bin().is_file():
            active = active_campaign()
            if active.id == "ce" and needs_ce_install():
                self._offer_ce_install()
            elif not active.installed():
                QMessageBox.information(self, "Not installed",
                                        f"{active.name} isn't installed yet - use the Campaigns page.")
                self.nav.setCurrentRow(self._page_row("Campaigns"))
            else:
                QMessageBox.critical(self, "Game not found", f"{game_bin()} is missing.")
            return
        self.result = (self.play.mode, self.play.slot)
        self.close()

    def _offer_ce_install(self):
        if QMessageBox.question(
                self, "Install Fallout 2 CE",
                "This looks like a vanilla Fallout 2 install, but this launcher needs the "
                "Community Edition engine to run the game. Download and install it now?",
                QMessageBox.StandardButton.Yes | QMessageBox.StandardButton.No,
                QMessageBox.StandardButton.Yes) != QMessageBox.StandardButton.Yes:
            return
        self.play_btn.setEnabled(False)
        self.play_btn.setText("Installing CE…")
        self._ce_thread = CEInstallThread()
        self._ce_thread.done.connect(self._ce_install_finished)
        self._ce_thread.failed.connect(self._ce_install_finished)
        self._ce_thread.start()

    def _ce_install_finished(self, error: str | None = None):
        self._ce_thread = None
        self.play_btn.setEnabled(True)
        self.update_play()
        if error:
            QMessageBox.critical(self, "Install failed", error)
            return
        QMessageBox.information(self, "Fallout 2 CE installed",
                                "Fallout 2 CE is installed. Press Play when you're ready.")

    # ---- unsaved changes ----

    def _confirm_unsaved(self) -> bool:
        """True if it's OK to proceed: nothing pending, it was saved, or it was discarded."""
        if not self.store.dirty_count():
            return True
        box = QMessageBox(self)
        box.setWindowTitle("Unsaved changes")
        box.setText("Save your changes before continuing?")
        box.setStandardButtons(QMessageBox.StandardButton.Save | QMessageBox.StandardButton.Discard
                               | QMessageBox.StandardButton.Cancel)
        choice = box.exec()
        if choice == QMessageBox.StandardButton.Cancel:
            return False
        if choice == QMessageBox.StandardButton.Save:
            return self.save()
        self.store.revert()
        return True

    def closeEvent(self, ev):
        if self.result is None and not self._confirm_unsaved():
            ev.ignore()
            return
        ev.accept()
