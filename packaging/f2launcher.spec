# -*- mode: python ; coding: utf-8 -*-
"""Builds the single-file launcher executable (one per OS - not cross-compilable; run
this natively on each target). Bundles schemas/ as read-only data inside the exe - see
backend/paths.py's RESOURCE_DIR, which resolves to sys._MEIPASS in a frozen build.

Run from the launcher/ directory:
    pyinstaller --noconfirm packaging/f2launcher.spec
"""

from pathlib import Path

root = Path(SPECPATH).parent  # launcher/

a = Analysis(
    [str(root / "f2launcher.py")],
    pathex=[str(root)],
    datas=[(str(root / "schemas"), "schemas")],
    hiddenimports=[],
    hookspath=[],
    noarchive=False,
)
pyz = PYZ(a.pure)

exe = EXE(
    pyz,
    a.scripts,
    a.binaries,
    a.datas,
    [],
    name="f2launcher",
    console=False,
    upx=True,
)
