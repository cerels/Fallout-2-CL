"""The Play page: continue/menu choice and the save-game picker with thumbnails."""

from __future__ import annotations

import datetime

from PySide6.QtCore import QSize, Qt, QTimer, Signal
from PySide6.QtGui import QColor, QImage, QPixmap
from PySide6.QtWidgets import QFrame, QGridLayout, QHBoxLayout, QLabel, QPushButton, QScrollArea, QVBoxLayout, QWidget

from backend.paths import PREVIEW_H, PREVIEW_W
from backend.saves import SaveGame, load_palette, read_saves

from .style import ACCENT
from .widgets import card

palette = load_palette()


def thumbnail_pixmap(save: SaveGame, scale: float) -> QPixmap:
    size = QSize(int(PREVIEW_W * scale), int(PREVIEW_H * scale))
    if save.thumbnail and palette:
        img = QImage(save.thumbnail, PREVIEW_W, PREVIEW_H, PREVIEW_W, QImage.Format.Format_Indexed8)
        img.setColorTable([0xFF000000 | (r << 16) | (g << 8) | b for r, g, b in palette])
        return QPixmap.fromImage(img.copy()).scaled(
            size, Qt.AspectRatioMode.KeepAspectRatio, Qt.TransformationMode.FastTransformation)
    pm = QPixmap(size)
    pm.fill(QColor("#0a0f0b"))
    return pm


def time_ago(ts: float) -> str:
    delta = datetime.datetime.now() - datetime.datetime.fromtimestamp(ts)
    mins = int(delta.total_seconds() // 60)
    if mins < 1:
        return "just now"
    if mins < 60:
        return f"{mins} min ago"
    if mins < 60 * 24:
        return f"{mins // 60} h ago"
    return datetime.datetime.fromtimestamp(ts).strftime("%d %b %Y")


class SaveCard(QFrame):
    clicked = Signal()
    double_clicked = Signal()

    def __init__(self, save: SaveGame):
        super().__init__()
        self.save = save
        self.setObjectName("savecard")
        self.setCursor(Qt.CursorShape.PointingHandCursor)
        self.setFixedWidth(PREVIEW_W + 24)
        lay = QVBoxLayout(self)
        lay.setContentsMargins(12, 12, 12, 12)
        lay.setSpacing(4)
        img = QLabel()
        img.setPixmap(thumbnail_pixmap(save, 1.0))
        lay.addWidget(img)
        name = QLabel(f"<b>{save.name}</b> <span style='color:#86a38b'>· slot {save.slot}</span>")
        loc = QLabel(save.location + (f" — {save.description}" if save.description else ""))
        loc.setStyleSheet(f"color: {ACCENT};")
        loc.setWordWrap(True)
        when = QLabel(f"{save.game_date} · saved {time_ago(save.mtime)}")
        when.setObjectName("muted")
        for w in (name, loc, when):
            lay.addWidget(w)
        self.setToolTip(f"{save.path.parent.name} — map {save.map_name}\nDouble-click to play")

    def set_selected(self, on: bool):
        self.setProperty("selected", on)
        self.style().unpolish(self)
        self.style().polish(self)

    def mouseReleaseEvent(self, ev):
        if ev.button() == Qt.MouseButton.LeftButton:
            self.clicked.emit()

    def mouseDoubleClickEvent(self, ev):
        if ev.button() == Qt.MouseButton.LeftButton:
            self.double_clicked.emit()


class PlayPage(QScrollArea):
    selection_changed = Signal()

    def __init__(self):
        super().__init__()
        self.setWidgetResizable(True)
        body = QWidget()
        body.setObjectName("root")
        lay = QVBoxLayout(body)
        lay.setContentsMargins(28, 24, 28, 28)
        lay.setSpacing(16)
        self.saves = read_saves()
        self.mode = "continue" if self.saves else "menu"
        self.slot = self.saves[0].slot if self.saves else None

        hero, hl = card("Continue")
        row = QHBoxLayout()
        row.setContentsMargins(16, 6, 16, 12)
        row.setSpacing(18)
        self.hero_img = QLabel()
        self.hero_text = QLabel()
        self.hero_text.setWordWrap(True)
        self.hero_text.setTextFormat(Qt.TextFormat.RichText)
        row.addWidget(self.hero_img)
        right = QVBoxLayout()
        right.addWidget(self.hero_text)
        right.addStretch()
        choices = QHBoxLayout()
        self.btn_continue = QPushButton("Continue latest save")
        self.btn_menu = QPushButton("Start at main menu")
        for b in (self.btn_continue, self.btn_menu):
            b.setCheckable(True)
            choices.addWidget(b)
        choices.addStretch()
        self.btn_continue.clicked.connect(lambda: self.choose("continue"))
        self.btn_menu.clicked.connect(lambda: self.choose("menu"))
        right.addLayout(choices)
        row.addLayout(right, 1)
        hl.addLayout(row)
        lay.addWidget(hero)

        saves_card, sl = card(f"Saved games ({len(self.saves)})")
        if self.saves:
            tip = QLabel("Click a save to load it instead, or double-click to start right away.")
            tip.setObjectName("muted")
            tip.setContentsMargins(16, 0, 16, 4)
            sl.addWidget(tip)
            self.grid_host = QWidget()
            self.grid = QGridLayout(self.grid_host)
            self.grid.setContentsMargins(12, 4, 12, 12)
            self.grid.setSpacing(12)
            self.cards = []
            for sv in self.saves:
                c = SaveCard(sv)
                c.clicked.connect(lambda slot=sv.slot: self.choose("slot", slot))
                c.double_clicked.connect(lambda slot=sv.slot: self._double(slot))
                self.cards.append(c)
            self._cols = 0
            sl.addWidget(self.grid_host)
            self._fit_list(cols=3)
        else:
            empty = QLabel("No saves yet. Start at the main menu and create a new character.")
            empty.setObjectName("muted")
            empty.setContentsMargins(16, 4, 16, 12)
            sl.addWidget(empty)
            self.cards = []
            self.btn_continue.setEnabled(False)
        lay.addWidget(saves_card)
        lay.addStretch()
        self.setWidget(body)
        self.double_clicked = None
        self.choose(self.mode, self.slot)

    def _fit_list(self, cols: int | None = None):
        if not self.cards:
            return
        if cols is None:
            cols = max(1, (self.viewport().width() - 90) // (PREVIEW_W + 40))
        if cols == self._cols:
            return
        self._cols = cols
        for i, c in enumerate(self.cards):
            self.grid.addWidget(c, i // cols, i % cols)
            c.show()
        self.grid.setColumnStretch(cols, 1)

    def resizeEvent(self, ev):
        super().resizeEvent(ev)
        QTimer.singleShot(0, self._fit_list)

    def _double(self, slot: int):
        self.choose("slot", slot)
        if self.double_clicked:
            self.double_clicked()

    def choose(self, mode: str, slot: int | None = None):
        self.mode = mode
        if mode == "continue" and self.saves:
            slot = self.saves[0].slot
        self.slot = slot if mode != "menu" else None
        self.btn_continue.setChecked(mode == "continue")
        self.btn_menu.setChecked(mode == "menu")
        for c in self.cards:
            c.set_selected(c.save.slot == self.slot)
        save = next((s for s in self.saves if s.slot == self.slot), None)
        if save:
            self.hero_img.setPixmap(thumbnail_pixmap(save, 1.5))
            self.hero_img.show()
            desc = f"<br><span style='color:#86a38b'>“{save.description}”</span>" if save.description else ""
            self.hero_text.setText(
                f"<div style='font-size:16pt;font-weight:700;color:#e8f7ea'>{save.name}</div>"
                f"<div style='font-size:12pt;color:{ACCENT}'>{save.location}</div>{desc}"
                f"<div style='color:#86a38b;margin-top:6px'>{save.game_date} · Slot {save.slot} · "
                f"saved {time_ago(save.mtime)}</div>")
        else:
            self.hero_img.hide()
            self.hero_text.setText(
                "<div style='font-size:16pt;font-weight:700;color:#e8f7ea'>Main menu</div>"
                "<div style='color:#86a38b'>Start a new game, load a save, or change options in-game.</div>")
        self.selection_changed.emit()

    def play_label(self) -> str:
        if self.mode == "menu" or self.slot is None:
            return "PLAY  ▶"
        if self.mode == "continue":
            return "CONTINUE  ▶"
        return f"LOAD SLOT {self.slot}  ▶"
