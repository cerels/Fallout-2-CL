"""DIK (DirectInput scan code) keyboard tables: party orders / sfall use these, not
Linux evdev codes or Qt key codes, so captured key presses need translating."""

from __future__ import annotations

DIK_NAMES = {1: "Esc", 12: "-", 13: "=", 14: "Backspace", 15: "Tab", 26: "[", 27: "]", 28: "Enter",
             29: "Left Ctrl", 39: ";", 40: "'", 41: "`", 42: "Left Shift", 43: "\\", 51: ",", 52: ".",
             53: "/", 54: "Right Shift", 55: "Num *", 56: "Left Alt", 57: "Space", 58: "Caps Lock",
             69: "Num Lock", 70: "Scroll Lock", 71: "Num 7", 72: "Num 8", 73: "Num 9", 74: "Num -",
             75: "Num 4", 76: "Num 5", 77: "Num 6", 78: "Num +", 79: "Num 1", 80: "Num 2", 81: "Num 3",
             82: "Num 0", 83: "Num .", 87: "F11", 88: "F12", 156: "Num Enter", 157: "Right Ctrl",
             181: "Num /", 184: "Right Alt", 199: "Home", 200: "Up", 201: "Page Up", 203: "Left",
             205: "Right", 207: "End", 208: "Down", 209: "Page Down", 210: "Insert", 211: "Delete"}
for _i, _c in enumerate("1234567890"):
    DIK_NAMES[2 + _i] = _c
for _row, _start in (("QWERTYUIOP", 16), ("ASDFGHJKL", 30), ("ZXCVBNM", 44)):
    for _i, _c in enumerate(_row):
        DIK_NAMES[_start + _i] = _c
for _i in range(10):
    DIK_NAMES[59 + _i] = f"F{_i + 1}"
DIK_FROM_NAME = {v: k for k, v in DIK_NAMES.items()}

# Linux evdev codes that differ from DIK codes (extended keys).
EVDEV_TO_DIK = {96: 156, 97: 157, 98: 181, 100: 184, 102: 199, 103: 200, 104: 201, 105: 203,
                106: 205, 107: 207, 108: 208, 109: 209, 110: 210, 111: 211}


def dik_label(value: str) -> str:
    parts = []
    for p in value.split("+"):
        p = p.strip()
        if not p.isdigit():
            return value
        code = int(p)
        parts.append("None" if code == 0 else DIK_NAMES.get(code, f"Key {code}"))
    # Show modifier first, like "Left Ctrl + B".
    if len(parts) == 2:
        parts.reverse()
    return " + ".join(parts)
