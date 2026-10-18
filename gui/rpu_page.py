"""The Restoration Project (RPU) page: a dedicated tab inside the CE campaign, always
present - RPU is a content overlay, not a separate campaign and not a regular mod (see
GLOSSARY.md), so it gets its own install/update/uninstall card here instead of living on
the Mods page, and shows up even before it's installed."""

from __future__ import annotations

from PySide6.QtCore import QThread, QUrl, Signal
from PySide6.QtGui import QDesktopServices
from PySide6.QtWidgets import QHBoxLayout, QLabel, QMessageBox, QPushButton

from backend.rpu_install import check_for_rpu_update, delete_rpu, install_rpu, installed, load_rpu_state
from backend.schema import Catalog, Page

from .settings_page import SettingsPage
from .store import Store
from .widgets import card

RPU_HOMEPAGE = "https://github.com/BGforgeNet/Fallout2_Restoration_Project"


class _InstallThread(QThread):
    failed = Signal(str)
    done = Signal()

    def run(self):
        try:
            install_rpu()
        except Exception as exc:  # surfaced to the UI thread via signal, never silent
            self.failed.emit(str(exc))
        else:
            self.done.emit()


class RpuPage(SettingsPage):
    def __init__(self, page: Page, store: Store, catalog: Catalog):
        super().__init__(page.name, page.groups, page.intro, store, catalog)
        self._thread: QThread | None = None
        self._pending_update: str | None = None

        frame, inner = card("Restoration Project Updated")
        self.status = QLabel()
        self.status.setObjectName("muted")
        self.status.setWordWrap(True)
        self.status.setContentsMargins(16, 4, 16, 8)
        inner.addWidget(self.status)
        row = QHBoxLayout()
        row.setContentsMargins(16, 0, 16, 12)
        src_btn = QPushButton("Open source")
        src_btn.clicked.connect(lambda: QDesktopServices.openUrl(QUrl(RPU_HOMEPAGE)))
        self.action_btn = QPushButton()
        self.action_btn.clicked.connect(self._on_action)
        self.uninstall_btn = QPushButton("Uninstall")
        self.uninstall_btn.clicked.connect(self._on_uninstall)
        row.addWidget(src_btn)
        row.addStretch()
        row.addWidget(self.action_btn)
        row.addWidget(self.uninstall_btn)
        inner.addLayout(row)
        # Right after the page title, ahead of any settings groups below (there are none
        # until RPU is actually installed).
        self.lay.insertWidget(1, frame)
        self._refresh()

    def _refresh(self):
        self._pending_update = None
        if installed():
            version = load_rpu_state().get("version", "an unknown version")
            self.status.setText(
                f"Installed ({version}). Restores cut content and fixes to Fallout 2 - the "
                "optional extras (enhanced world map, new animations, Cassidy's voice/head, ...) "
                "can be switched on individually from the Mods page.")
            self.action_btn.setText("Check for updates")
            self.uninstall_btn.setVisible(True)
        else:
            self.status.setText(
                "Restores cut content and fixes to Fallout 2 - not a separate campaign, just "
                "more of the same game. Not installed yet.")
            self.action_btn.setText("Install")
            self.uninstall_btn.setVisible(False)

    def _on_action(self):
        if not installed() or self._pending_update:
            self._on_install()
            return
        version = check_for_rpu_update()
        if version:
            self._pending_update = version
            self.action_btn.setText(f"Update to {version}")
        else:
            QMessageBox.information(self, "Restoration Project", "Already up to date.")

    def _on_install(self):
        self.action_btn.setEnabled(False)
        self.action_btn.setText("Installing…" if not installed() else "Updating…")
        self._thread = _InstallThread()
        self._thread.done.connect(lambda: self._install_finished(None))
        self._thread.failed.connect(self._install_finished)
        self._thread.start()

    def _install_finished(self, error: str | None):
        self._thread = None
        self.action_btn.setEnabled(True)
        if error:
            QMessageBox.warning(self, "Install failed", f"Restoration Project: {error}")
        self._refresh()

    def _on_uninstall(self):
        if QMessageBox.question(
                self, "Uninstall Restoration Project",
                "Remove RPU's files from mods/? This can't be undone.",
                QMessageBox.StandardButton.Yes | QMessageBox.StandardButton.No,
                QMessageBox.StandardButton.No) != QMessageBox.StandardButton.Yes:
            return
        delete_rpu()
        self._refresh()
