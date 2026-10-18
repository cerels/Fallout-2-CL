"""Color constants and the application-wide Qt stylesheet. No PySide6 import needed here."""

from __future__ import annotations

ACCENT = "#5dfc8a"
AMBER = "#ffb84d"
DANGER = "#ff5c5c"

STYLESHEET = f"""
    * {{ font-size: 10.5pt; }}
    QMainWindow, QWidget#root {{ background: #0c110d; color: #d4ecd6; }}
    QWidget {{ color: #d4ecd6; }}
    QLabel#muted {{ color: #86a38b; }}
    QLabel#note {{ color: {AMBER}; }}
    QLabel#hint {{ color: #9fd8ab; }}
    QLabel#title {{ font-family: monospace; font-size: 26pt; font-weight: 800; color: {ACCENT};
                   letter-spacing: 4px; }}
    QLabel#subtitle {{ color: #86a38b; font-size: 10pt; }}
    QLabel#subtitle[warn="true"] {{ color: {DANGER}; font-weight: 700; }}
    QLabel#pagetitle {{ font-size: 17pt; font-weight: 700; color: #e8f7ea; }}
    QLabel#cardtitle {{ font-size: 11pt; font-weight: 700; color: {ACCENT};
                       text-transform: uppercase; letter-spacing: 1px; }}
    QLabel#rowtitle {{ font-weight: 600; color: #e6f5e8; }}
    QLabel#dot {{ color: {AMBER}; font-size: 13pt; }}
    QFrame#sidebar {{ background: #101812; border-right: 1px solid #1f3324; }}
    QWidget#campaignswitcher {{ border-radius: 8px; }}
    QWidget#campaignswitcher:hover {{ background: #16241a; }}
    QFrame#card {{ background: #131c15; border: 1px solid #1f3324; border-radius: 10px; }}
    QFrame#row {{ border-top: 1px solid #1a2a1e; }}
    QFrame#row[first="true"] {{ border-top: none; }}
    QFrame#bottombar {{ background: #101812; border-top: 1px solid #1f3324; }}
    QListWidget#nav {{ background: transparent; border: none; outline: none; }}
    QListWidget#nav::item {{ padding: 10px 14px; border-radius: 8px; margin: 2px 8px; color: #b9d6bd; }}
    QListWidget#nav::item:selected {{ background: #1c3322; color: {ACCENT}; font-weight: 700; }}
    QListWidget#nav::item:hover:!selected {{ background: #16241a; }}
    QListWidget#nav::item:disabled {{ color: #4a5f4e; }}
    QLineEdit, QComboBox, QSpinBox, QDoubleSpinBox {{
        background: #0f1711; border: 1px solid #2a4530; border-radius: 6px; padding: 5px 8px;
        selection-background-color: #2f6b3d; min-height: 20px; }}
    QLineEdit:focus, QComboBox:focus, QSpinBox:focus, QDoubleSpinBox:focus {{ border-color: {ACCENT}; }}
    QComboBox QAbstractItemView {{ background: #111a13; border: 1px solid #2a4530;
                                  selection-background-color: #1f3d27; }}
    QComboBox::drop-down {{ border: none; width: 22px; }}
    QPushButton {{ background: #18261b; border: 1px solid #2a4530; border-radius: 7px;
                  padding: 7px 14px; }}
    QPushButton:hover {{ background: #1f3324; border-color: #3d6a47; }}
    QPushButton:disabled {{ color: #4a5f4e; border-color: #1c2b1f; }}
    QPushButton#primary {{ background: {ACCENT}; color: #06210f; border: none; font-weight: 800;
                          font-size: 12.5pt; padding: 10px 28px; letter-spacing: 1px; }}
    QPushButton#primary:hover {{ background: #86ffa8; }}
    QPushButton#primary[install="true"] {{ background: {AMBER}; color: #2a1800; }}
    QPushButton#primary[install="true"]:hover {{ background: #ffce85; }}
    QPushButton#keycap {{ font-family: monospace; min-width: 150px; }}
    QPushButton#keycap[capturing="true"] {{ border-color: {AMBER}; color: {AMBER}; }}
    QToolButton#reset {{ background: transparent; border: none; color: #86a38b; font-size: 13pt; }}
    QToolButton#reset:hover {{ color: {AMBER}; }}
    QSlider::groove:horizontal {{ height: 4px; background: #24372a; border-radius: 2px; }}
    QSlider::sub-page:horizontal {{ background: {ACCENT}; border-radius: 2px; }}
    QSlider::handle:horizontal {{ background: #e6f5e8; width: 14px; height: 14px; margin: -6px 0;
                                 border-radius: 7px; }}
    QScrollArea {{ border: none; background: transparent; }}
    QScrollBar:vertical {{ background: transparent; width: 10px; }}
    QScrollBar::handle:vertical {{ background: #24372a; border-radius: 5px; min-height: 30px; }}
    QScrollBar::add-line, QScrollBar::sub-line {{ height: 0; }}
    QFrame#savecard {{ background: #0f1711; border: 2px solid #1f3324; border-radius: 10px; }}
    QFrame#savecard:hover {{ border-color: #3d6a47; }}
    QFrame#savecard[selected="true"] {{ border-color: {ACCENT}; background: #17261a; }}
    QListWidget#mods {{ background: #0f1711; border: 1px solid #1f3324; border-radius: 10px; outline: none; }}
    QListWidget#mods::item {{ padding: 8px 10px; border-bottom: 1px solid #1a2a1e; }}
    QListWidget#mods::item:selected {{ background: #1c3322; }}
    QMessageBox {{ background: #131c15; }}
    QPushButton:checked {{ border: 2px solid {ACCENT}; color: {ACCENT}; background: #17261a; }}
    QListWidget#mods::item:disabled {{ color: #4a5f4e; }}
    QListWidget#mods::indicator {{ width: 18px; height: 18px; border: 1px solid #3d6a47; border-radius: 4px;
                                  background: #0c110d; margin-right: 8px; }}
    QListWidget#mods::indicator:checked {{ background: {ACCENT}; border-color: {ACCENT}; }}
"""
