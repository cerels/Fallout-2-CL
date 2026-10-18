"""The Campaigns page: install, switch between, and manage every campaign this launcher
knows about (see GLOSSARY.md). The sidebar's quick-switch dropdown covers picking an
already-installed one; this page is where you install a new one or fix up something like
Et Tu's Fallout 1 data."""

from __future__ import annotations

from pathlib import Path

from PySide6.QtCore import QThread, Signal
from PySide6.QtWidgets import (QFileDialog, QFrame, QHBoxLayout, QLabel, QMessageBox, QPushButton,
                               QScrollArea, QVBoxLayout, QWidget)

from backend import ce_install, ettu_install, nevada_install, sonora_install
from backend.campaigns import CAMPAIGNS, Campaign, active_campaign

from .widgets import card

INSTALLERS = {
    "ce": ce_install.install_ce,
    "ettu": ettu_install.install_ettu,
    "sonora": sonora_install.install_sonora,
    "nevada": nevada_install.install_nevada,
}

DESCRIPTIONS = {
    "ce": "Vanilla Fallout 2, on the Community Edition engine. RPU (Restoration Project) "
          "lives on its own tab inside this campaign's menu, not here.",
    "ettu": "Fallout 1, ported into the Fallout 2 engine. Installing fetches Fallout 2's own "
            "engine files automatically; it still needs a one-time extraction from a "
            "Fallout 1 copy you already own before it can start a game.",
    "sonora": "A standalone total conversion set in a post-war Mexico/Sonora - no other game "
              "required.",
    "nevada": "A standalone total conversion (English translation) - no other game required.",
}


class _RunThread(QThread):
    """Runs a zero-arg callable off the UI thread - install_ce/install_ettu/install_sonora/
    install_nevada all take no arguments, so one thread class covers every campaign."""
    failed = Signal(str)
    done = Signal()

    def __init__(self, func):
        super().__init__()
        self._func = func

    def run(self):
        try:
            self._func()
        except Exception as exc:  # surfaced to the UI thread via signal, never silent
            self.failed.emit(str(exc))
        else:
            self.done.emit()


class CampaignsPage(QScrollArea):
    campaign_switch_requested = Signal(object)  # Campaign

    def __init__(self):
        super().__init__()
        self.setWidgetResizable(True)
        self._threads: list[QThread] = []
        self._cards: dict[str, dict] = {}

        body = QWidget()
        body.setObjectName("root")
        self.lay = QVBoxLayout(body)
        self.lay.setContentsMargins(28, 24, 28, 28)
        self.lay.setSpacing(16)
        t = QLabel("Campaigns")
        t.setObjectName("pagetitle")
        self.lay.addWidget(t)
        intro = QLabel("Pick what you're playing. Switching reloads this launcher's settings, "
                       "mods and saves for that campaign - each keeps its own.")
        intro.setObjectName("muted")
        intro.setWordWrap(True)
        self.lay.addWidget(intro)

        for campaign in CAMPAIGNS:
            self.lay.addWidget(self._build_card(campaign))
        self.lay.addStretch()
        self.setWidget(body)
        self.refresh()

    def _build_card(self, campaign: Campaign) -> QFrame:
        frame, inner = card(f"{campaign.badge}  {campaign.name}")
        status = QLabel()
        status.setObjectName("muted")
        status.setWordWrap(True)
        status.setContentsMargins(16, 0, 16, 8)
        inner.addWidget(status)

        row = QHBoxLayout()
        row.setContentsMargins(16, 0, 16, 8)
        action_btn = QPushButton("Install")
        action_btn.clicked.connect(lambda: self._on_install(campaign))
        switch_btn = QPushButton("Switch to this campaign")
        switch_btn.clicked.connect(lambda: self.campaign_switch_requested.emit(campaign))
        row.addWidget(action_btn)
        row.addWidget(switch_btn)
        row.addStretch()
        inner.addLayout(row)

        extra_btn = None
        if campaign.id == "ettu":
            extra_row = QHBoxLayout()
            extra_row.setContentsMargins(16, 0, 16, 12)
            extra_btn = QPushButton("Extract Fallout 1 data…")
            extra_btn.clicked.connect(self._on_extract_fallout1)
            extra_row.addWidget(extra_btn)
            extra_row.addStretch()
            inner.addLayout(extra_row)

        self._cards[campaign.id] = {
            "status": status, "action_btn": action_btn, "switch_btn": switch_btn, "extra_btn": extra_btn,
        }
        return frame

    def refresh(self):
        active = active_campaign()
        for campaign in CAMPAIGNS:
            w = self._cards[campaign.id]
            is_active, is_installed = campaign.id == active.id, campaign.installed()
            lines = [DESCRIPTIONS[campaign.id]]
            if is_active:
                lines.append("Currently active.")
            elif is_installed:
                lines.append("Installed.")
            else:
                lines.append("Not installed yet.")
            if campaign.id == "ettu" and is_installed and not ettu_install.has_fallout1_data():
                lines.append("Needs Fallout 1's data extracted before it can start a game.")
            w["status"].setText(" ".join(lines))
            w["action_btn"].setVisible(not is_installed)
            w["switch_btn"].setVisible(is_installed and not is_active)
            if w["extra_btn"] is not None:
                w["extra_btn"].setVisible(is_installed)

    def _on_install(self, campaign: Campaign):
        w = self._cards[campaign.id]
        w["action_btn"].setEnabled(False)
        w["action_btn"].setText("Installing…")
        thread = _RunThread(INSTALLERS[campaign.id])
        thread.done.connect(lambda: self._install_finished(campaign, thread, None))
        thread.failed.connect(lambda msg: self._install_finished(campaign, thread, msg))
        self._threads.append(thread)
        thread.start()

    def _install_finished(self, campaign: Campaign, thread: QThread, error: str | None):
        self._threads.remove(thread)
        w = self._cards[campaign.id]
        w["action_btn"].setEnabled(True)
        w["action_btn"].setText("Install")
        if error:
            QMessageBox.warning(self, "Install failed", f"{campaign.name}: {error}")
        self.refresh()

    def _on_extract_fallout1(self):
        path, _ = QFileDialog.getOpenFileName(
            self, "Select Fallout 1's MASTER.DAT", "", "MASTER.DAT (*.dat *.DAT);;All files (*)")
        if not path:
            return
        try:
            ettu_install.extract_fallout1_data(Path(path))
        except Exception as exc:
            QMessageBox.warning(self, "Extraction failed", str(exc))
            return
        QMessageBox.information(self, "Fallout 1 data extracted", "Done - Et Tu is ready to play.")
        self.refresh()
