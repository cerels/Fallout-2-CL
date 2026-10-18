"""The Mods page: enable/disable/reorder installed mods, and download/update/delete
catalog mods, all in one list. See backend/mod_catalog.py for the install/update/delete logic."""

from __future__ import annotations

import datetime
import re
import shutil

from PySide6.QtCore import QThread, QUrl, Qt, Signal
from PySide6.QtGui import QDesktopServices
from PySide6.QtWidgets import (QAbstractItemView, QHBoxLayout, QLabel, QListWidget, QListWidgetItem,
                               QMessageBox, QPushButton, QVBoxLayout, QWidget)

from backend.mod_catalog import (CatalogEntry, check_for_update, delete_mod_files, install_catalog_entry,
                                 load_catalog_state, save_catalog_state)
from backend.paths import BACKUP_DIR, ci_path, mod_installed

from .store import Store


class InstallThread(QThread):
    failed = Signal(str)
    done = Signal()

    def __init__(self, entry: CatalogEntry):
        super().__init__()
        self.entry = entry

    def run(self):
        try:
            install_catalog_entry(self.entry)
        except Exception as exc:  # surfaced to the UI thread via signal, never silent
            self.failed.emit(str(exc))
        else:
            self.done.emit()


class UpdateCheckThread(QThread):
    found = Signal(str, str)  # catalog id, newer version

    def __init__(self, entries: list[CatalogEntry]):
        super().__init__()
        self.entries = entries

    def run(self):
        for e in self.entries:
            version = check_for_update(e)
            if version:
                self.found.emit(e.id, version)


class ModsPage(QWidget):
    def __init__(self, store: Store, mod_catalog: list[CatalogEntry]):
        super().__init__()
        self.store = store
        self.mod_catalog = mod_catalog
        self.setObjectName("root")
        self.path = ci_path("mods/mods_order.txt")
        self._threads: list[QThread] = []
        self._row_widgets: dict[str, QWidget] = {}
        self._pending_updates: dict[str, str] = {}
        lay = QVBoxLayout(self)
        lay.setContentsMargins(28, 24, 28, 28)
        lay.setSpacing(12)
        t = QLabel("Mods")
        t.setObjectName("pagetitle")
        lay.addWidget(t)
        intro = QLabel("Tick to enable, drag to reorder. Mods lower in the list override the ones above. "
                       "Unticked mods stay in mods_order.txt, commented out. Mods fallout2-ce explicitly "
                       "supports that you don't have yet are listed too, with a Download button.")
        intro.setObjectName("muted")
        intro.setWordWrap(True)
        lay.addWidget(intro)
        self.list = QListWidget()
        self.list.setObjectName("mods")
        self.list.setDragDropMode(QAbstractItemView.DragDropMode.InternalMove)
        self.list.itemChanged.connect(lambda _i: self.store.dirty_changed.emit())
        self.list.model().rowsMoved.connect(self._on_rows_moved)
        self.list.model().rowsInserted.connect(lambda *_: self.store.dirty_changed.emit())
        lay.addWidget(self.list, 1)
        buttons = QHBoxLayout()
        up, down = QPushButton("Move up"), QPushButton("Move down")
        up.clicked.connect(lambda: self.move(-1))
        down.clicked.connect(lambda: self.move(1))
        show_missing = QPushButton("Show not installed")
        show_missing.setCheckable(True)
        show_missing.toggled.connect(lambda on: self.set_hide_missing(not on))
        self.hide_missing = True
        folder = QPushButton("Open mods folder")
        folder.clicked.connect(lambda: QDesktopServices.openUrl(QUrl.fromLocalFile(str(ci_path("mods")))))
        for b in (up, down, show_missing):
            buttons.addWidget(b)
        buttons.addStretch()
        buttons.addWidget(folder)
        lay.addLayout(buttons)

        self.revert()
        self._check_for_updates()

    # ---- data ----

    def _entries(self) -> list[tuple[str, bool]]:
        """Installed, order-participating mods: (dat filename, enabled). A mods_order.txt
        line whose file no longer exists is dropped here — nothing to show, and it won't
        be written back next time anything is saved, so dead references clean themselves up."""
        entries, seen = [], set()
        if self.path.is_file():
            for line in self.path.read_text("latin-1").splitlines():
                t = line.strip()
                if not t:
                    continue
                enabled = not re.search(r"[;#]", t)
                name = re.sub(r"[;#]", "", t).strip()
                if not name or name.lower() in seen:
                    continue
                if mod_installed(name):
                    entries.append((name, enabled))
                    seen.add(name.lower())
        mods_dir = ci_path("mods")
        if mods_dir.is_dir():
            for p in sorted(mods_dir.iterdir()):
                if p.suffix.lower() == ".dat" and p.name.lower() not in seen and p.name.lower() != "f2_res.dat":
                    entries.append((p.name, False))
                    seen.add(p.name.lower())
        return entries

    def _catalog_match(self, filename: str) -> CatalogEntry | None:
        for e in self.mod_catalog:
            if any(d.lower() == filename.lower() for d in e.dat):
                return e
        return None

    def _serialize(self) -> list[tuple[str, bool]]:
        out = []
        for i in range(self.list.count()):
            it = self.list.item(i)
            if not it.data(Qt.ItemDataRole.UserRole + 2):  # a download-only row, not order data
                continue
            out.append((it.data(Qt.ItemDataRole.UserRole), it.checkState() == Qt.CheckState.Checked))
        return out

    # ---- row widgets ----

    def _row_widget(self, *, key: str, title: str, subtitle: str, entry: CatalogEntry | None,
                     installed: bool) -> QWidget:
        w = QWidget()
        w.setMinimumHeight(36)  # room for a button even on a single-line (no subtitle) row
        row = QHBoxLayout(w)
        row.setContentsMargins(10, 2, 10, 2)
        text = QLabel(f"<b>{title}</b>" + (f"<br><span style='color:#86a38b'>{subtitle}</span>"
                                            if subtitle else ""))
        text.setWordWrap(True)
        text.setTextFormat(Qt.TextFormat.RichText)
        row.addWidget(text, 1)
        if entry:
            src_btn = QPushButton("Open source")
            src_btn.clicked.connect(lambda _=False, e=entry: QDesktopServices.openUrl(QUrl(e.homepage)))
            row.addWidget(src_btn)
        w.update_btn = None
        if entry and installed and entry.github:
            update_btn = QPushButton("Update")
            update_btn.setVisible(False)  # revealed once a background check finds a newer version
            update_btn.clicked.connect(lambda _=False, e=entry, b=update_btn: self._start_download(e, b))
            row.addWidget(update_btn)
            w.update_btn = update_btn
        if entry and not installed:
            dl_btn = QPushButton("Download")
            dl_btn.clicked.connect(lambda _=False, e=entry, b=dl_btn: self._start_download(e, b))
            row.addWidget(dl_btn)
        if installed:
            del_btn = QPushButton("Delete")
            del_btn.clicked.connect(lambda _=False, n=key, e=entry: self._delete_row(n, e))
            row.addWidget(del_btn)
        return w

    def _reattach_widgets(self):
        for i in range(self.list.count()):
            it = self.list.item(i)
            key = str(it.data(Qt.ItemDataRole.UserRole)).lower()
            widget = self._row_widgets.get(key)
            if widget is not None and self.list.itemWidget(it) is not widget:
                self.list.setItemWidget(it, widget)

    def _on_rows_moved(self, *_args):
        self._reattach_widgets()
        self.store.dirty_changed.emit()

    # ---- rebuilding the list ----

    def revert(self):
        self.list.blockSignals(True)
        self.list.clear()
        self._row_widgets.clear()
        for name, enabled in self._entries():
            entry = self._catalog_match(name)
            it = QListWidgetItem()
            it.setData(Qt.ItemDataRole.UserRole, name)
            it.setData(Qt.ItemDataRole.UserRole + 1, True)   # installed
            it.setData(Qt.ItemDataRole.UserRole + 2, True)   # participates in load order
            flags = it.flags() | Qt.ItemFlag.ItemIsUserCheckable | Qt.ItemFlag.ItemIsDragEnabled
            flags &= ~Qt.ItemFlag.ItemIsDropEnabled
            it.setFlags(flags)
            it.setCheckState(Qt.CheckState.Checked if enabled else Qt.CheckState.Unchecked)
            widget = self._row_widget(key=name, title=entry.name if entry else name,
                                       subtitle=entry.description if entry else "",
                                       entry=entry, installed=True)
            self.list.addItem(it)
            self.list.setItemWidget(it, widget)
            it.setSizeHint(widget.sizeHint())
            self._row_widgets[name.lower()] = widget
        for entry in self.mod_catalog:
            if entry.installed():
                continue
            dat_name = entry.dat[0]
            it = QListWidgetItem()
            it.setData(Qt.ItemDataRole.UserRole, dat_name)
            it.setData(Qt.ItemDataRole.UserRole + 1, False)  # not installed -> hidden by default
            it.setData(Qt.ItemDataRole.UserRole + 2, False)  # download-only, not an order row
            it.setFlags(Qt.ItemFlag.ItemIsEnabled | Qt.ItemFlag.ItemIsSelectable)
            widget = self._row_widget(key=dat_name, title=entry.name, subtitle=entry.description,
                                       entry=entry, installed=False)
            self.list.addItem(it)
            self.list.setItemWidget(it, widget)
            it.setSizeHint(widget.sizeHint())
            self._row_widgets[dat_name.lower()] = widget
        self.list.blockSignals(False)
        self.initial = self._serialize()
        self.set_hide_missing(getattr(self, "hide_missing", True))
        for entry_id, version in self._pending_updates.items():
            self._reveal_update(entry_id, version)
        self.store.dirty_changed.emit()

    def set_hide_missing(self, hide: bool):
        self.hide_missing = hide
        for i in range(self.list.count()):
            it = self.list.item(i)
            it.setHidden(hide and not it.data(Qt.ItemDataRole.UserRole + 1))

    def move(self, delta: int):
        row = self.list.currentRow()
        new = row + delta
        if row < 0 or not 0 <= new < self.list.count() or not self.list.item(row).data(
                Qt.ItemDataRole.UserRole + 2):
            return
        it = self.list.takeItem(row)
        self.list.insertItem(new, it)
        self.list.setCurrentRow(new)
        self._reattach_widgets()
        self.store.dirty_changed.emit()

    # ---- download / update / delete ----

    def _start_download(self, entry: CatalogEntry, button: QPushButton):
        is_update = button.text().startswith("Update")
        button.setEnabled(False)
        button.setText("Updating…" if is_update else "Downloading…")
        thread = InstallThread(entry)
        thread.done.connect(lambda: self._install_finished(entry, thread, None))
        thread.failed.connect(lambda msg: self._install_finished(entry, thread, msg))
        self._threads.append(thread)
        thread.start()

    def _install_finished(self, entry: CatalogEntry, thread: QThread, error: str | None):
        self._threads.remove(thread)
        if error:
            QMessageBox.warning(self, "Download failed", f"{entry.name}: {error}")
            self.revert()  # restores normal button state/text
            return
        self._pending_updates.pop(entry.id, None)
        self.revert()

    def _delete_row(self, dat_name: str, entry: CatalogEntry | None):
        extra = [entry.ini] if entry and entry.ini else []
        listed = ", ".join([dat_name, *extra])
        if QMessageBox.question(
                self, "Delete mod", f"Remove {listed} from mods/? This can't be undone.",
                QMessageBox.StandardButton.Yes | QMessageBox.StandardButton.No,
                QMessageBox.StandardButton.No) != QMessageBox.StandardButton.Yes:
            return
        delete_mod_files(dat_name, extra)
        if entry:
            state = load_catalog_state()
            if state.pop(entry.id, None) is not None:
                save_catalog_state(state)
            self._pending_updates.pop(entry.id, None)
        self.revert()

    def _check_for_updates(self):
        candidates = [e for e in self.mod_catalog if e.installed() and e.github]
        if not candidates:
            return
        thread = UpdateCheckThread(candidates)
        thread.found.connect(self._reveal_update)
        thread.finished.connect(lambda: thread in self._threads and self._threads.remove(thread))
        self._threads.append(thread)
        thread.start()

    def _reveal_update(self, entry_id: str, version: str):
        self._pending_updates[entry_id] = version
        entry = next((e for e in self.mod_catalog if e.id == entry_id), None)
        if not entry:
            return
        for d in entry.dat:
            widget = self._row_widgets.get(d.lower())
            if widget is not None and widget.update_btn is not None:
                widget.update_btn.setText(f"Update ({version})")
                widget.update_btn.setVisible(True)

    # ---- saving ----

    def dirty_count(self) -> int:
        return 1 if self._serialize() != self.initial else 0

    def save(self):
        if not self.dirty_count():
            return
        if self.path.is_file():
            stamp = datetime.datetime.now().strftime("%Y%m%d-%H%M%S")
            dest = BACKUP_DIR / stamp / "mods" / "mods_order.txt"
            dest.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(self.path, dest)
        lines = [name if enabled else f";{name}" for name, enabled in self._serialize()]
        self.path.write_text("\n".join(lines) + "\n", "latin-1")
        self.initial = self._serialize()
