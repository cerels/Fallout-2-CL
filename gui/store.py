"""Store: the live settings values every page reads from and writes through."""

from __future__ import annotations

import datetime
import shutil

from PySide6.QtCore import QObject, Signal

from backend.dat import dat_read, parse_ini_text
from backend.ini_file import IniFile
from backend.paths import BACKUP_DIR, ci_path
from backend.schema import Catalog, Setting


class Store(QObject):
    changed = Signal(str)
    dirty_changed = Signal()

    def __init__(self, catalog: Catalog):
        super().__init__()
        self.catalog = catalog
        self.files: dict[str, IniFile] = {}
        self.settings: dict[str, Setting] = {}
        self.initial: dict[str, str] = {}
        self.current: dict[str, str] = {}
        self.extras: list = []
        self.files = {fid: IniFile(ci_path(rel)) for fid, rel in catalog.files.items()}
        # Defaults for keys missing from a file, read from a config inside a DAT archive.
        self.dat_defaults: dict[str, dict[tuple[str, str], str]] = {}
        for fid, (dat, inner) in catalog.defaults_from.items():
            text = dat_read(ci_path(dat), inner)
            self.dat_defaults[fid] = parse_ini_text(text.decode("latin-1")) if text else {}

    def file_ok(self, setting: Setting) -> bool:
        f = self.files.get(setting.file)
        return f is not None and (f.exists or setting.file == "launcher")

    def default_of(self, s: Setting) -> str | None:
        d = self.dat_defaults.get(s.file, {}).get((s.section.lower(), s.key.lower()))
        if d is not None:
            return s.normalize(d)
        return s.normalize(s.default)

    def _read(self, s: Setting) -> str | None:
        f = self.files[s.file]
        if s.kind == "resolution":
            kx, ky = s.key.split("|")
            x, y = f.get(s.section, kx), f.get(s.section, ky)
            return f"{x}x{y}" if x and y else None
        return s.normalize(f.get(s.section, s.key))

    def register(self, s: Setting):
        self.settings[s.id] = s
        value = self._read(s)
        if value is None:
            value = self.default_of(s) or ""
        self.initial[s.id] = value
        self.current[s.id] = value

    def value(self, sid: str) -> str | None:
        return self.current.get(sid)

    def set(self, sid: str, value: str):
        s = self.settings[sid]
        value = s.normalize(value) or ""
        if self.current.get(sid) == value:
            return
        self.current[sid] = value
        self.changed.emit(sid)
        self.dirty_changed.emit()

    def dirty_ids(self) -> list[str]:
        return [sid for sid, v in self.current.items() if v != self.initial.get(sid)]

    def dirty_count(self) -> int:
        return len(self.dirty_ids()) + sum(e.dirty_count() for e in self.extras)

    def revert(self):
        for sid in self.dirty_ids():
            self.current[sid] = self.initial[sid]
            self.changed.emit(sid)
        for e in self.extras:
            e.revert()
        self.dirty_changed.emit()

    def save(self) -> str | None:
        """Write all pending changes. Returns an error message or None."""
        try:
            touched = set()
            for sid in self.dirty_ids():
                s = self.settings[sid]
                f = self.files[s.file]
                if s.kind == "resolution":
                    kx, ky = s.key.split("|")
                    x, y = self.current[sid].lower().split("x")
                    f.set(s.section, kx, x)
                    f.set(s.section, ky, y)
                else:
                    f.set(s.section, s.key, self.current[sid])
                touched.add(s.file)
            backup_stamp = getattr(self, "_backup_stamp", None)
            if backup_stamp is None:
                backup_stamp = datetime.datetime.now().strftime("%Y%m%d-%H%M%S")
            for fid in touched:
                f = self.files[fid]
                if f.exists and fid != "launcher":
                    dest = BACKUP_DIR / backup_stamp / self.catalog.files[fid]
                    if not dest.exists():
                        dest.parent.mkdir(parents=True, exist_ok=True)
                        shutil.copy2(f.path, dest)
                        self._backup_stamp = backup_stamp
                f.save()
            for e in self.extras:
                e.save()
            for sid in self.dirty_ids():
                self.initial[sid] = self.current[sid]
            self.dirty_changed.emit()
            for sid in list(self.current):
                self.changed.emit(sid)
            return None
        except (OSError, ValueError) as exc:
            return str(exc)
