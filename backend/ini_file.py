"""Line-preserving INI editing: only the lines that change are rewritten."""

from __future__ import annotations

import os
import re
from pathlib import Path


class IniFile:
    """Line-preserving INI editor: only the lines that change are rewritten."""

    def __init__(self, path: Path):
        self.path = path
        self.exists = path.is_file()
        raw = path.read_bytes().decode("latin-1") if self.exists else ""
        self.newline = "\r\n" if "\r\n" in raw else "\n"
        self.lines = raw.splitlines()
        self.dirty = False

    @staticmethod
    def _header(line: str) -> str | None:
        t = line.strip()
        if t.startswith("[") and "]" in t:
            return t[1:t.index("]")].strip().lower()
        return None

    def _find(self, section: str, key: str):
        sec_start = key_idx = None
        sec_end = len(self.lines)
        s, k = section.lower(), key.lower()
        for i, line in enumerate(self.lines):
            name = self._header(line)
            if name is not None:
                if sec_start is not None:
                    sec_end = i
                    break
                if name == s:
                    sec_start = i
                continue
            t = line.strip()
            if sec_start is not None and key_idx is None and "=" in t and not t.startswith((";", "#")):
                if t.split("=", 1)[0].strip().lower() == k:
                    key_idx = i
        return sec_start, key_idx, sec_end

    def get(self, section: str, key: str) -> str | None:
        _, k, _ = self._find(section, key)
        if k is None:
            return None
        return self.lines[k].split("=", 1)[1].strip()

    def set(self, section: str, key: str, value: str) -> None:
        s, k, end = self._find(section, key)
        if k is not None:
            line = self.lines[k]
            prefix = re.match(r"\s*[^=]*=\s*", line).group(0)
            new = prefix + value
            if new != line:
                self.lines[k] = new
                self.dirty = True
            return
        if s is None:
            if self.lines and self.lines[-1].strip():
                self.lines.append("")
            self.lines += [f"[{section}]", f"{key}={value}"]
        else:
            ins = end
            while ins - 1 > s and not self.lines[ins - 1].strip():
                ins -= 1
            self.lines.insert(ins, f"{key}={value}")
        self.dirty = True

    def save(self) -> None:
        if not self.dirty:
            return
        self.path.parent.mkdir(parents=True, exist_ok=True)
        tmp = self.path.with_name(self.path.name + ".tmp")
        tmp.write_bytes((self.newline.join(self.lines) + self.newline).encode("latin-1"))
        os.replace(tmp, self.path)
        self.exists = True
        self.dirty = False
