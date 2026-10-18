"""One settings page: a title, an intro, and cards of rows built from a schema's groups."""

from __future__ import annotations

from PySide6.QtCore import Qt
from PySide6.QtWidgets import (QComboBox, QDoubleSpinBox, QFrame, QHBoxLayout, QLabel, QLineEdit,
                               QScrollArea, QSlider, QSpinBox, QToolButton, QVBoxLayout, QWidget)

from backend.schema import HINTS, Catalog, Setting

from .store import Store
from .style import AMBER
from .widgets import KeyCapture, ResolutionEdit, Toggle, card


class SettingRow(QFrame):
    def __init__(self, s: Setting, first: bool, store: Store, catalog: Catalog):
        super().__init__()
        self.s = s
        self.store = store
        self.catalog = catalog
        self.setObjectName("row")
        self.setProperty("first", first)
        self.search_text = f"{s.label} {s.help} {s.note} {s.key}".lower()
        lay = QHBoxLayout(self)
        lay.setContentsMargins(16, 12, 12, 12)
        lay.setSpacing(14)
        text = QVBoxLayout()
        text.setSpacing(3)
        self.title = QLabel(s.label)
        self.title.setObjectName("rowtitle")
        self.title.setWordWrap(True)
        self.title.setTextFormat(Qt.TextFormat.RichText)
        text.addWidget(self.title)
        if s.help:
            h = QLabel(s.help)
            h.setObjectName("muted")
            h.setWordWrap(True)
            text.addWidget(h)
        if s.note:
            n = QLabel(s.note)
            n.setObjectName("note")
            n.setWordWrap(True)
            text.addWidget(n)
        self.hint = QLabel()
        self.hint.setObjectName("hint")
        self.hint.setWordWrap(True)
        self.hint.hide()
        text.addWidget(self.hint)
        lay.addLayout(text, 1)

        self.control = self._make_control()
        lay.addWidget(self.control, 0, Qt.AlignmentFlag.AlignVCenter)
        self.reset = QToolButton()
        self.reset.setObjectName("reset")
        self.reset.setText("↺")
        self.reset.setToolTip("Reset to default")
        self.reset.setCursor(Qt.CursorShape.PointingHandCursor)
        self.reset.clicked.connect(self._reset)
        sp = self.reset.sizePolicy()
        sp.setRetainSizeWhenHidden(True)
        self.reset.setSizePolicy(sp)
        lay.addWidget(self.reset, 0, Qt.AlignmentFlag.AlignVCenter)

        if not store.file_ok(s):
            self.setEnabled(False)
            self.setToolTip(f"{catalog.files.get(s.file, s.file)} not found")
        store.changed.connect(self._on_store)
        self.refresh()

    def _make_control(self) -> QWidget:
        s = self.s
        store = self.store
        if s.kind == "bool":
            w = Toggle()
            w.toggled.connect(lambda on: store.set(s.id, "1" if on else "0"))
            return w
        if s.kind == "choice":
            w = QComboBox()
            w.setMinimumWidth(230)
            for value, label in s.options:
                w.addItem(label, value)
            w.currentIndexChanged.connect(lambda _i: store.set(s.id, w.currentData()))
            return w
        if s.kind == "resolution":
            w = ResolutionEdit()
            w.edited.connect(lambda v: store.set(s.id, v))
            return w
        if s.kind == "key":
            w = KeyCapture(s.combo_keys)
            w.captured.connect(lambda v: store.set(s.id, v))
            return w
        if s.kind in ("int", "float", "volume"):
            box = QWidget()
            lay = QHBoxLayout(box)
            lay.setContentsMargins(0, 0, 0, 0)
            if s.kind == "volume":
                slider = QSlider(Qt.Orientation.Horizontal)
                slider.setRange(0, 100)
                slider.setFixedWidth(200)
                label = QLabel()
                label.setFixedWidth(44)
                lay.addWidget(slider)
                lay.addWidget(label)

                def on_slide(v):
                    label.setText(f"{v}%")
                    store.set(s.id, str(round(v * 32767 / 100)))
                slider.valueChanged.connect(on_slide)
                box.slider, box.spin, box.label = slider, None, label
                return box
            if s.kind == "float":
                spin = QDoubleSpinBox()
                spin.setDecimals(s.decimals)
                spin.setSingleStep(s.step)
                spin.setRange(s.minimum, s.maximum)
                spin.setSuffix(s.suffix)
                spin.valueChanged.connect(lambda v: store.set(s.id, f"{v:.6f}"))
            else:
                spin = QSpinBox()
                spin.setRange(int(s.minimum), int(s.maximum))
                spin.setSingleStep(int(s.step))
                spin.setSuffix(s.suffix)
                spin.valueChanged.connect(lambda v: store.set(s.id, str(v)))
            spin.setMinimumWidth(110)
            slider = None
            if s.slider:
                slider = QSlider(Qt.Orientation.Horizontal)
                slider.setFixedWidth(170)
                mult = 1 / s.step if s.kind == "float" else 1
                slider.setRange(int(round(s.minimum * mult)), int(round(s.maximum * mult)))
                slider.valueChanged.connect(lambda v: spin.setValue(v / mult))
                spin.valueChanged.connect(lambda v: (slider.blockSignals(True),
                                                     slider.setValue(int(round(v * mult))),
                                                     slider.blockSignals(False)))
                box.mult = mult
                lay.addWidget(slider)
            lay.addWidget(spin)
            box.slider, box.spin = slider, spin
            return box
        w = QLineEdit()
        w.setMinimumWidth(230)
        w.editingFinished.connect(lambda: store.set(s.id, w.text()))
        return w

    def refresh(self):
        s, v = self.s, self.store.value(self.s.id) or ""
        c = self.control
        c.blockSignals(True)
        if s.kind == "bool":
            c.setCheckedNow(v == "1")
        elif s.kind == "choice":
            idx = c.findData(v)
            if idx < 0:
                c.addItem(f"Custom ({v})", v)
                idx = c.count() - 1
            c.setCurrentIndex(idx)
        elif s.kind == "resolution":
            c.set_value(v)
        elif s.kind == "key":
            c.set_value(v or "0")
        elif s.kind == "volume":
            c.slider.blockSignals(True)
            pct = round(int(v or 0) * 100 / 32767)
            c.slider.setValue(pct)
            c.label.setText(f"{pct}%")
            c.slider.blockSignals(False)
        elif s.kind in ("int", "float"):
            c.spin.blockSignals(True)
            try:
                num = float(v) if s.kind == "float" else int(float(v))
            except ValueError:
                num = s.minimum
            c.spin.setValue(num)
            c.spin.blockSignals(False)
            if c.slider is not None:
                c.slider.blockSignals(True)
                c.slider.setValue(int(round(num * c.mult)))
                c.slider.blockSignals(False)
        elif isinstance(c, QLineEdit):
            c.setText(v)
        c.blockSignals(False)

        default = self.store.default_of(s)
        differs = default is not None and v != default
        label = s.label.replace("&", "&amp;").replace("<", "&lt;")
        self.title.setText(f"{label}&nbsp;&nbsp;<span style='color:{AMBER}'>●</span>" if differs else label)
        self.title.setToolTip("Changed from the default" if differs else "")
        self.reset.setVisible(differs and self.store.file_ok(s))
        if s.enabled_if and self.store.file_ok(s):
            self.setEnabled(s.enabled_by(self.store.value(s.enabled_if["setting"]) or ""))
        if s.hint in HINTS:
            text = HINTS[s.hint][0](self.store)
            self.hint.setText(text)
            self.hint.setVisible(bool(text))

    def _on_store(self, sid: str):
        s = self.s
        deps = {s.id}
        if s.enabled_if:
            deps.add(s.enabled_if["setting"])
        if s.hint in HINTS:
            deps |= HINTS[s.hint][1]
        if sid in deps:
            self.refresh()

    def _reset(self):
        d = self.store.default_of(self.s)
        if d is not None:
            self.store.set(self.s.id, d)


class SettingsPage(QScrollArea):
    def __init__(self, title: str, groups: list[tuple[str, list[Setting]]], intro: str,
                 store: Store, catalog: Catalog):
        super().__init__()
        self.setWidgetResizable(True)
        body = QWidget()
        body.setObjectName("root")
        self.lay = QVBoxLayout(body)
        self.lay.setContentsMargins(28, 24, 28, 28)
        self.lay.setSpacing(16)
        t = QLabel(title)
        t.setObjectName("pagetitle")
        self.lay.addWidget(t)
        if intro:
            i = QLabel(intro)
            i.setObjectName("muted")
            i.setWordWrap(True)
            self.lay.addWidget(i)
        self.groups = []
        for gtitle, settings in groups:
            frame, inner = card(gtitle)
            rows = []
            for n, s in enumerate(settings):
                store.register(s)
                row = SettingRow(s, n == 0, store, catalog)
                inner.addWidget(row)
                rows.append(row)
            self.groups.append((frame, rows))
            self.lay.addWidget(frame)
        self.lay.addStretch()
        self.setWidget(body)

    def apply_filter(self, text: str) -> int:
        total = 0
        for frame, rows in self.groups:
            visible = 0
            for row in rows:
                show = not text or text in row.search_text
                row.setVisible(show)
                visible += show
            frame.setVisible(visible > 0)
            total += visible
        return total
