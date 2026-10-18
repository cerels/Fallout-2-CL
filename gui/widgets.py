"""Reusable Qt widgets with no dependency on the settings store or any page."""

from __future__ import annotations

from PySide6.QtCore import QEasingCurve, QPropertyAnimation, QRectF, QSize, Property, Qt, Signal
from PySide6.QtGui import QColor, QGuiApplication, QKeyEvent, QPainter
from PySide6.QtWidgets import (QAbstractButton, QComboBox, QFrame, QHBoxLayout, QLabel, QPushButton,
                               QSpinBox, QVBoxLayout, QWidget)

from backend.keys import DIK_FROM_NAME, EVDEV_TO_DIK, dik_label

from .style import ACCENT


class Toggle(QAbstractButton):
    def __init__(self):
        super().__init__()
        self.setCheckable(True)
        self.setCursor(Qt.CursorShape.PointingHandCursor)
        self.setFixedSize(48, 26)
        self._pos = 0.0
        self._anim = QPropertyAnimation(self, b"knob", self)
        self._anim.setDuration(120)
        self._anim.setEasingCurve(QEasingCurve.Type.OutCubic)
        self.toggled.connect(self._animate)

    def sizeHint(self):
        return QSize(48, 26)

    def _animate(self, on: bool):
        self._anim.stop()
        self._anim.setEndValue(1.0 if on else 0.0)
        self._anim.start()

    def setCheckedNow(self, on: bool):
        self.blockSignals(True)
        self.setChecked(on)
        self.blockSignals(False)
        self._pos = 1.0 if on else 0.0
        self.update()

    def getKnob(self):
        return self._pos

    def setKnob(self, v):
        self._pos = v
        self.update()

    knob = Property(float, getKnob, setKnob)

    def paintEvent(self, _):
        p = QPainter(self)
        p.setRenderHint(QPainter.RenderHint.Antialiasing)
        r = QRectF(1, 3, 46, 20)
        off, on = QColor("#2a3a2e"), QColor(ACCENT)
        track = QColor(
            int(off.red() + (on.red() - off.red()) * self._pos),
            int(off.green() + (on.green() - off.green()) * self._pos),
            int(off.blue() + (on.blue() - off.blue()) * self._pos))
        if not self.isEnabled():
            track.setAlpha(80)
        p.setPen(Qt.PenStyle.NoPen)
        p.setBrush(track)
        p.drawRoundedRect(r, 10, 10)
        x = r.left() + 2 + (r.width() - 20) * self._pos
        p.setBrush(QColor("#f2fbf3") if self.isEnabled() else QColor("#6f7f72"))
        p.drawEllipse(QRectF(x, r.top() + 2, 16, 16))


class KeyCapture(QPushButton):
    captured = Signal(str)

    def __init__(self, combo: bool):
        super().__init__()
        self.setObjectName("keycap")
        self.combo = combo
        self.value = "0"
        self.capturing = False
        self._pending_mod = None
        self.clicked.connect(self.start)

    def set_value(self, v: str):
        self.value = v
        self.setText(dik_label(v))

    def start(self):
        self.capturing = True
        self._pending_mod = None
        self.setProperty("capturing", True)
        self.style().unpolish(self)
        self.style().polish(self)
        self.setText("Press a key…  (Esc cancels, Del clears)")
        self.grabKeyboard()

    def stop(self):
        self.capturing = False
        self.setProperty("capturing", False)
        self.style().unpolish(self)
        self.style().polish(self)
        self.releaseKeyboard()
        self.setText(dik_label(self.value))

    @staticmethod
    def to_dik(ev: QKeyEvent) -> int | None:
        sc = ev.nativeScanCode()
        if sc > 8:
            evdev = sc - 8
            if evdev in EVDEV_TO_DIK:
                return EVDEV_TO_DIK[evdev]
            if evdev <= 88:
                return evdev
        text = ev.text().upper()
        if text in DIK_FROM_NAME:
            return DIK_FROM_NAME[text]
        return None

    MODS = {Qt.Key.Key_Control: 29, Qt.Key.Key_Shift: 42, Qt.Key.Key_Alt: 56}

    def keyPressEvent(self, ev: QKeyEvent):
        if not self.capturing:
            return super().keyPressEvent(ev)
        key = ev.key()
        if key == Qt.Key.Key_Escape:
            self.stop()
            return
        if key == Qt.Key.Key_Delete:
            self.value = "0"
            self.stop()
            self.captured.emit("0")
            return
        if key in self.MODS:
            self._pending_mod = self.to_dik(ev) or self.MODS[key]
            return
        dik = self.to_dik(ev)
        if dik is None:
            return
        mod = None
        if self.combo:
            mods = ev.modifiers()
            if mods & Qt.KeyboardModifier.ControlModifier:
                mod = 29
            elif mods & Qt.KeyboardModifier.AltModifier:
                mod = 56
            elif mods & Qt.KeyboardModifier.ShiftModifier:
                mod = 42
        self.value = f"{dik}+{mod}" if mod else str(dik)
        self.stop()
        self.captured.emit(self.value)

    def keyReleaseEvent(self, ev: QKeyEvent):
        if self.capturing and self._pending_mod and ev.key() in self.MODS:
            self.value = str(self._pending_mod)
            self.stop()
            self.captured.emit(self.value)
            return
        super().keyReleaseEvent(ev)

    def focusOutEvent(self, ev):
        if self.capturing:
            self.stop()
        super().focusOutEvent(ev)


class ResolutionEdit(QWidget):
    edited = Signal(str)
    PRESETS = ["1280x720", "1366x768", "1600x900", "1920x1080", "1920x1200", "2560x1080",
               "2560x1440", "3440x1440", "3840x2160"]

    def __init__(self):
        super().__init__()
        lay = QHBoxLayout(self)
        lay.setContentsMargins(0, 0, 0, 0)
        self.combo = QComboBox()
        screens = []
        for sc in QGuiApplication.screens():
            g = sc.geometry()
            ratio = sc.devicePixelRatio()
            screens.append(f"{int(g.width() * ratio)}x{int(g.height() * ratio)}")
        self.native = screens
        items = []
        for r in screens:
            items.append((r, f"{r.replace('x', '×')}  (monitor)"))
        for r in self.PRESETS:
            if r not in screens:
                items.append((r, r.replace("x", "×")))
        for value, label in items:
            self.combo.addItem(label, value)
        self.combo.addItem("Custom…", "custom")
        self.w = QSpinBox()
        self.h = QSpinBox()
        self.w.setRange(640, 7680)
        self.h.setRange(480, 4320)
        self.x = QLabel("×")
        lay.addWidget(self.combo)
        lay.addWidget(self.w)
        lay.addWidget(self.x)
        lay.addWidget(self.h)
        self.combo.currentIndexChanged.connect(self._combo)
        self.w.valueChanged.connect(self._spin)
        self.h.valueChanged.connect(self._spin)

    def set_value(self, v: str):
        self.blockSignals(True)
        for w in (self.combo, self.w, self.h):
            w.blockSignals(True)
        try:
            x, y = (int(a) for a in v.lower().split("x"))
        except ValueError:
            x, y = 640, 480
        idx = self.combo.findData(f"{x}x{y}")
        self.combo.setCurrentIndex(idx if idx >= 0 else self.combo.count() - 1)
        self.w.setValue(x)
        self.h.setValue(y)
        custom = idx < 0
        for w in (self.w, self.x, self.h):
            w.setVisible(custom)
        for w in (self.combo, self.w, self.h):
            w.blockSignals(False)
        self.blockSignals(False)

    def _combo(self):
        data = self.combo.currentData()
        custom = data == "custom"
        for w in (self.w, self.x, self.h):
            w.setVisible(custom)
        if custom:
            self.edited.emit(f"{self.w.value()}x{self.h.value()}")
        else:
            self.edited.emit(data)

    def _spin(self):
        self.edited.emit(f"{self.w.value()}x{self.h.value()}")


class ClickableWidget(QWidget):
    """A plain QWidget that emits clicked() on a left click - for a header area that
    should act like a button but needs custom layout (badge + title + subtitle)."""
    clicked = Signal()

    def __init__(self):
        super().__init__()
        self.setAttribute(Qt.WidgetAttribute.WA_Hover, True)

    def mousePressEvent(self, ev):
        if ev.button() == Qt.MouseButton.LeftButton:
            self.clicked.emit()
        super().mousePressEvent(ev)


def card(title: str) -> tuple[QFrame, QVBoxLayout]:
    frame = QFrame()
    frame.setObjectName("card")
    outer = QVBoxLayout(frame)
    outer.setContentsMargins(0, 12, 0, 6)
    outer.setSpacing(0)
    t = QLabel(title)
    t.setObjectName("cardtitle")
    t.setContentsMargins(16, 0, 16, 6)
    outer.addWidget(t)
    return frame, outer
