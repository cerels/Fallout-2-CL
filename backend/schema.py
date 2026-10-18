"""Settings are described by TOML schema files, not by code:
  launcher/schemas/*.toml        core schemas: Fallout 2 CE engine settings only
  launcher/schemas/mods/*.toml   schemas for popular mods
  mods/*.schema.toml             schemas shipped by mods themselves
Mod schemas only show up when the files they configure are installed, and
settings marked `requires_engine` only when the fallout2-ce binary has them.
Any mods/*.ini that no schema covers gets a tab generated from the file's
own comments (see generate_ini_groups). Run with --check-schemas to validate.
"""

from __future__ import annotations

import re
import sys
from dataclasses import dataclass, field
from pathlib import Path

from .paths import RESOURCE_DIR, ci_path, game_bin, mod_installed

SETTING_TYPES = {"bool", "choice", "int", "float", "volume", "key", "resolution", "text"}


@dataclass
class Setting:
    file: str
    section: str
    key: str
    label: str
    kind: str
    default: str | None = None
    help: str = ""
    note: str = ""
    options: list[tuple[str, str]] = field(default_factory=list)
    minimum: float = 0
    maximum: float = 100
    step: float = 1
    suffix: str = ""
    slider: bool = False
    combo_keys: bool = False
    enabled_if: dict | None = None
    hint: str | None = None
    decimals: int = 2
    # Engine setting name that must exist in the fallout2-ce binary (for settings
    # newer than some CE releases).
    requires_engine: str | None = None
    # Option value -> engine setting name it needs.
    option_requires: dict[str, str] = field(default_factory=dict)
    # From a mod schema: hidden when its file isn't installed.
    optional: bool = False

    @property
    def id(self) -> str:
        return f"{self.file}:{self.section}:{self.key}"

    def normalize(self, value: str | None) -> str | None:
        if value is None:
            return None
        v = value.strip()
        try:
            if self.kind == "bool":
                return "1" if v.lower() in ("1", "true", "yes", "on") or (v.isdigit() and int(v) != 0) else "0"
            if self.kind in ("int", "volume", "choice") and re.fullmatch(r"-?\d+", v):
                return str(int(v))
            if self.kind == "float":
                return f"{float(v):.6f}"
        except ValueError:
            pass
        return v

    def enabled_by(self, value: str) -> bool:
        if not self.enabled_if:
            return True
        if "equals" in self.enabled_if:
            return value in self.enabled_if["equals"]
        if "not" in self.enabled_if:
            return value not in self.enabled_if["not"]
        return True


@dataclass
class Page:
    name: str
    groups: list[tuple[str, list[Setting]]] = field(default_factory=list)
    intro: str = ""
    generated: bool = False

    def group(self, name: str) -> list[Setting]:
        for gname, settings in self.groups:
            if gname == name:
                return settings
        self.groups.append((name, []))
        return self.groups[-1][1]


@dataclass
class Catalog:
    files: dict[str, str] = field(default_factory=dict)
    defaults_from: dict[str, tuple[str, str]] = field(default_factory=dict)
    pages: list[Page] = field(default_factory=list)
    errors: list[str] = field(default_factory=list)
    # Files that are mod data rather than player settings; never given a generated tab.
    not_settings: list[str] = field(default_factory=list)
    # Mods page descriptions, by lowercase .dat name.
    mod_info: dict[str, str] = field(default_factory=dict)
    # Installed mods whose title is shown next to the engine name.
    headlines: list[str] = field(default_factory=list)
    # Human-readable notes about settings left out (missing files, engine features).
    skipped: list[str] = field(default_factory=list)

    def page(self, name: str) -> Page:
        for p in self.pages:
            if p.name == name:
                return p
        self.pages.append(Page(name))
        return self.pages[-1]


def schema_paths() -> list[Path]:
    builtin = RESOURCE_DIR / "schemas"
    paths = sorted(builtin.glob("*.toml")) + sorted((builtin / "mods").glob("*.toml"))
    mods_dir = ci_path("mods")
    if mods_dir.is_dir():
        paths += sorted(p for p in mods_dir.iterdir() if p.name.lower().endswith(".schema.toml"))
    return paths


def _setting_from_toml(data: dict, group: dict, where: str, optional: bool) -> Setting:
    kind = data.get("type", "text")
    if kind not in SETTING_TYPES:
        raise ValueError(f"{where}: unknown type {kind!r}")
    for required in ("key", "label"):
        if required not in data:
            raise ValueError(f"{where}: missing {required!r}")
    file_id = data.get("file", group.get("file"))
    section = data.get("section", group.get("section"))
    if not file_id or section is None:
        raise ValueError(f"{where}: {data['key']} needs a file and section")
    options, option_requires = [], {}
    for opt in data.get("options", []):
        if len(opt) not in (2, 3):
            raise ValueError(f"{where}: options entries are [value, label] or [value, label, requires_engine]")
        options.append((str(opt[0]), str(opt[1])))
        if len(opt) == 3:
            option_requires[str(opt[0])] = str(opt[2])
    return Setting(
        file=file_id, section=section, key=data["key"], label=data["label"], kind=kind,
        default=None if data.get("default") is None else str(data["default"]),
        help=data.get("help", ""), note=data.get("note", ""),
        options=options, option_requires=option_requires,
        requires_engine=data.get("requires_engine"), optional=optional,
        minimum=data.get("min", 0), maximum=data.get("max", 100), step=data.get("step", 1),
        suffix=data.get("suffix", ""), slider=data.get("slider", False),
        combo_keys=data.get("combo_keys", False), enabled_if=data.get("enabled_if"),
        hint=data.get("hint"), decimals=data.get("decimals", 2))


_engine_binary_cache: dict[Path, bytes] = {}


def engine_supports(name: str) -> bool:
    """True if the active campaign's engine binary knows the setting `name`."""
    path = game_bin()
    binary = _engine_binary_cache.get(path)
    if binary is None:
        try:
            binary = path.read_bytes()
        except OSError:
            binary = b""
        _engine_binary_cache[path] = binary
    return b"\0" + name.encode() + b"\0" in binary


def load_catalog() -> Catalog:
    import tomllib

    cat = Catalog()
    core_dir = RESOURCE_DIR / "schemas"
    for path in schema_paths():
        is_core = path.parent == core_dir
        try:
            doc = tomllib.loads(path.read_text("utf-8"))
        except (OSError, UnicodeDecodeError, tomllib.TOMLDecodeError) as exc:
            cat.errors.append(f"{path.name}: {exc}")
            continue
        cat.files.update({k: str(v) for k, v in doc.get("files", {}).items()})
        cat.not_settings += [str(v) for v in doc.get("not_settings", [])]
        # A mod counts as installed when its archive is present; schemas without
        # [[mod]] entries fall back to their config files.
        mods = doc.get("mod", [])
        for mod in mods:
            if mod.get("description"):
                cat.mod_info[str(mod.get("dat", "")).lower()] = str(mod["description"])
        if mods:
            installed = any(mod_installed(str(m.get("dat", ""))) for m in mods)
        else:
            installed = any(ci_path(rel).is_file() for rel in doc.get("files", {}).values())
        if doc.get("headline") and doc.get("title") and installed:
            cat.headlines.append(str(doc["title"]))
        for fid, src in doc.get("defaults_from", {}).items():
            cat.defaults_from[fid] = (src["dat"], src["path"])
        page_name = doc.get("page")
        for gi, group in enumerate(doc.get("group", [])):
            page = cat.page(group.get("page", page_name) or path.stem)
            if doc.get("intro") and not page.intro:
                page.intro = doc["intro"]
            settings = page.group(group.get("name", "Settings"))
            for si, data in enumerate(group.get("setting", [])):
                try:
                    settings.append(_setting_from_toml(
                        data, group, f"{path.name} group {gi + 1} setting {si + 1}", optional=not is_core))
                except (ValueError, TypeError) as exc:
                    cat.errors.append(str(exc))

    for page in cat.pages:
        for _, settings in page.groups:
            for s in settings:
                if s.file not in cat.files and s.file != "launcher":
                    cat.errors.append(f"{page.name}: {s.key} refers to unknown file {s.file!r}")

    # Leave out what this install can't use: mod settings whose file isn't installed,
    # and engine settings the installed fallout2-ce binary doesn't have.
    for page in cat.pages:
        kept_groups = []
        for gname, settings in page.groups:
            kept = []
            for s in settings:
                if s.requires_engine and not engine_supports(s.requires_engine):
                    cat.skipped.append(f"{s.key}: needs engine setting {s.requires_engine!r}")
                    continue
                if s.optional and not ci_path(cat.files.get(s.file, "")).is_file():
                    cat.skipped.append(f"{cat.files.get(s.file)} not installed")
                    continue
                for value, feature in list(s.option_requires.items()):
                    if not engine_supports(feature):
                        s.options = [o for o in s.options if o[0] != value]
                        cat.skipped.append(f"{s.key} option {value}: needs engine setting {feature!r}")
                kept.append(s)
            if kept:
                kept_groups.append((gname, kept))
        page.groups = kept_groups
    cat.pages = [p for p in cat.pages if p.groups or p.name == "Launcher"]

    # Mod config files without a schema get a generated tab.
    covered = {ci_path(rel).resolve() for rel in [*cat.files.values(), *cat.not_settings]}
    mods_dir = ci_path("mods")
    if mods_dir.is_dir():
        for ini in sorted(mods_dir.iterdir()):
            if ini.suffix.lower() != ".ini" or ini.resolve() in covered:
                continue
            file_id = "auto_" + re.sub(r"[^a-z0-9]+", "_", ini.stem.lower())
            cat.files[file_id] = f"mods/{ini.name}"
            page, intro = Page(ini.stem), ""
            try:
                groups, intro = generate_ini_groups(ini, file_id)
            except OSError as exc:
                cat.errors.append(f"{ini.name}: {exc}")
                continue
            if not groups:
                continue
            for _, ss in groups:
                for s in ss:
                    s.optional = True
            page.groups = groups
            page.generated = True
            page.intro = (f"Generated automatically from mods/{ini.name}. Labels and controls are guessed "
                          f"from the file's comments; a schema file describes it properly. {intro}").strip()
            cat.pages.append(page)
    return cat


# Comment lines like "1 - glowing red" or "2: Fill screen".
_OPTION_RE = re.compile(r"^(-?\d+)\s*(?:-|–|:|=)\s*(.+)$")
# A comment that talks about values other than 0/1 ("set to 2", "-1 is vanilla", "is 49").
_NON_BOOL_RE = re.compile(r"(?:^|\s)-1\b|\b(?:set (?:it )?to|to|is|=)\s*(?:[2-9]|\d\d+)\b", re.IGNORECASE)
_BOOL_RE = re.compile(r"enable|disable|set (?:it )?to [01]\b|\b1/0\b|\b0/1\b|\bon\b|\boff\b", re.IGNORECASE)


def humanize(key: str) -> str:
    text = re.sub(r"(?<=[a-z])(?=[A-Z])", " ", key).replace("_", " ").strip()
    return text[:1].upper() + text[1:] if text else key


def infer_setting(file_id: str, section: str, key: str, value: str, comments: list[str],
                  bool_hint: bool) -> Setting:
    text = " ".join(comments)
    options = [m.groups() for m in (_OPTION_RE.match(c) for c in comments) if m]
    help_lines = [c for c in comments if not _OPTION_RE.match(c)]
    s = Setting(file_id, section, key, humanize(key), "text", help=" ".join(help_lines))
    is_int = re.fullmatch(r"-?\d+", value) is not None
    if len(options) >= 2 and is_int:
        s.kind, s.options = "choice", [(v, l.strip()) for v, l in options]
    elif is_int and re.search(r"scan ?code|\bdik\b|directinput|dik\.h", text, re.IGNORECASE):
        s.kind = "key"
    elif value in ("0", "1") and (bool_hint or _BOOL_RE.search(text)) and not _NON_BOOL_RE.search(text):
        s.kind = "bool"
    elif is_int:
        s.kind, s.minimum, s.maximum = "int", -1_000_000, 1_000_000
    elif re.fullmatch(r"-?\d+\.\d+", value):
        s.kind, s.minimum, s.maximum, s.decimals = "float", -1_000_000.0, 1_000_000.0, 3
    return s


def generate_ini_groups(path: Path, file_id: str) -> tuple[list[tuple[str, list[Setting]]], str]:
    """Build settings groups from an INI file's sections, keys and comments."""
    groups: list[tuple[str, list[Setting]]] = []
    preamble: list[str] = []
    comments: list[str] = []
    section = None
    settings: list[Setting] = []
    for raw in path.read_text("latin-1").splitlines():
        t = raw.strip()
        if not t:
            comments = []
        elif t.startswith("[") and "]" in t:
            section = t[1:t.index("]")].strip()
            settings = []
            groups.append((section, settings))
            comments = []
        elif t.startswith((";", "#")):
            c = t.lstrip(";#").strip()
            if c:
                (comments if section is not None else preamble).append(c)
        elif "=" in t and section is not None:
            key, value = (x.strip() for x in t.split("=", 1))
            value = re.split(r"\s+;", value, maxsplit=1)[0].strip()
            bool_hint = bool(_BOOL_RE.search(" ".join(preamble)))
            settings.append(infer_setting(file_id, section, key, value, comments, bool_hint))
            comments = []
    return [(name, ss) for name, ss in groups if ss], " ".join(preamble)


def scale_hint(store) -> str:
    res = store.value("cfg:screen:resolution_x|resolution_y") or "640x480"
    try:
        w, h = (int(x) for x in res.lower().split("x"))
        scale = int(store.value("cfg:screen:scale") or 1)
    except ValueError:
        return ""
    gw, gh = w // scale, h // scale
    if gw < 640 or gh < 480:
        return f"⚠ {w}×{h} at {scale}× would be {gw}×{gh}, below the 640×480 minimum. The game will ignore the scale."
    return f"The game will render at {gw}×{gh}, with every pixel drawn {scale}× larger."


# Dynamic hints a schema can attach with `hint = "<name>"`, and the settings they depend on.
HINTS = {
    "scale": (scale_hint, {"cfg:screen:resolution_x|resolution_y", "cfg:screen:scale"}),
}


def check_schemas() -> int:
    cat = load_catalog()
    for page in cat.pages:
        count = sum(len(ss) for _, ss in page.groups)
        tag = "  (generated)" if page.generated else ""
        print(f"{page.name}: {len(page.groups)} groups, {count} settings{tag}")
    from collections import Counter
    for note, count in Counter(cat.skipped).items():
        print(f"hidden: {note}" + (f" ({count} settings)" if count > 1 else ""))
    if cat.headlines:
        print("headline mods:", ", ".join(cat.headlines))
    for err in cat.errors:
        print(f"error: {err}", file=sys.stderr)
    return 1 if cat.errors else 0
