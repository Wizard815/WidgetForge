#!/usr/bin/env python3
"""Checks every widget in widgets/ and builds the catalog the WidgetForge Hub reads.

    python3 tools/build_index.py                  # check only (pull requests)
    python3 tools/build_index.py --out site       # check, then write site/index.json
                                                  # and site/packages/<id>-<version>.forgewidget

Standard library only (Python 3.9+). The rules match what the app enforces on import,
so a widget that passes here installs in the app.
"""
import argparse, hashlib, io, json, os, re, sys, zipfile
from datetime import datetime, timezone
from pathlib import Path

MAX_FILE = 20 * 1024 * 1024
MAX_TOTAL = 100 * 1024 * 1024
MAX_FILES = 2000
ID_PART = re.compile(r"^[a-z0-9-]+$")
SEMVER = re.compile(r"^\d+\.\d+\.\d+([-+][0-9A-Za-z.-]+)?$")
KNOWN_FLAGS = ["storage", "clipboard", "media", "audio", "docker", "latency", "discord", "wake", "game",
               "mail", "activity", "notify", "launch"]
KNOWN_LISTS = ["system", "network", "run", "folders", "files"]

try:
    import yaml  # PyYAML, if installed: full YAML.
except ImportError:  # pragma: no cover
    yaml = None


def load_yaml(text):
    if yaml is None:
        sys.exit("PyYAML is needed: pip install pyyaml")
    return yaml.safe_load(text)


def check_widget(folder: Path):
    """Returns (manifest, files, problems)."""
    problems = []
    yml = folder / "widget.yaml"
    if not yml.is_file():
        return None, [], ["no widget.yaml"]
    try:
        m = load_yaml(yml.read_text(encoding="utf-8"))
    except Exception as e:  # noqa: BLE001
        return None, [], [f"widget.yaml is not valid YAML: {e}"]
    if not isinstance(m, dict):
        return None, [], ["widget.yaml must be a mapping"]

    wid = str(m.get("id", ""))
    parts = wid.split(".")
    if m.get("schema") != 1:
        problems.append("`schema` must be 1")
    if len(parts) < 2 or not all(ID_PART.match(p) for p in parts):
        problems.append(f"bad id `{wid}`: lowercase words joined by dots, like sam.cpu-card")
    if wid.startswith("forge."):
        problems.append("ids starting with forge. belong to the built-in widgets")
    if folder.name != wid:
        problems.append(f"the folder must be named after the id ({wid})")
    for key in ("name", "version", "author", "description"):
        if not str(m.get(key, "")).strip():
            problems.append(f"missing `{key}`")
    if m.get("version") and not SEMVER.match(str(m["version"])):
        problems.append("`version` must look like 1.2.3")
    ui = m.get("ui") or {}
    if not (ui.get("blocks") is not None or ui.get("entry") or ui.get("stack")):
        problems.append("`ui` needs `blocks` or `entry`")
    perms = m.get("permissions") or {}
    for k, v in perms.items():
        if k in KNOWN_LISTS:
            if not isinstance(v, list):
                problems.append(f"permissions.{k} must be a list")
        elif k in KNOWN_FLAGS:
            if not isinstance(v, bool):
                problems.append(f"permissions.{k} must be true or false")
        else:
            problems.append(f"unknown permission `{k}`")
    for s in m.get("settings") or []:
        if isinstance(s, dict) and s.get("type") == "command" and s.get("default") not in (None, ""):
            problems.append(f"setting `{s.get('key')}`: a command may not have a default; only the user types one")
    for h in perms.get("network") or []:
        if isinstance(h, str) and ("/" in h or "*" in h):
            problems.append(f"network permission `{h}`: list a host name (or host:port), no paths or wildcards")

    files, total = [], 0
    for root, dirs, names in os.walk(folder):
        dirs[:] = [d for d in dirs if not d.startswith(".")]
        for n in names:
            if n.startswith("."):
                continue
            p = Path(root) / n
            if p.is_symlink():
                problems.append(f"{p.relative_to(folder)} is a link; only plain files are allowed")
                continue
            size = p.stat().st_size
            total += size
            if size > MAX_FILE:
                problems.append(f"{p.relative_to(folder)} is larger than 20 MB")
            files.append(p)
    if total > MAX_TOTAL:
        problems.append("the widget is larger than 100 MB")
    if len(files) > MAX_FILES:
        problems.append("the widget has more than 2000 files")
    return m, sorted(files), problems


def package(folder: Path, files):
    buf = io.BytesIO()
    with zipfile.ZipFile(buf, "w", zipfile.ZIP_DEFLATED) as z:
        for p in files:
            info = zipfile.ZipInfo(p.relative_to(folder).as_posix(), date_time=(2026, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = 0o100644 << 16
            z.writestr(info, p.read_bytes())
    return buf.getvalue()


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--widgets", default="widgets")
    ap.add_argument("--out", help="write the catalog here")
    args = ap.parse_args()

    root = Path(args.widgets)
    entries, failed = [], False
    folders = sorted(p for p in root.iterdir() if p.is_dir() and not p.name.startswith(".")) if root.is_dir() else []
    for folder in folders:
        m, files, problems = check_widget(folder)
        if problems:
            failed = True
            for p in problems:
                print(f"::error file={folder}/widget.yaml::{folder.name}: {p}")
            continue
        print(f"ok  {m['id']} {m['version']}  permissions: {json.dumps(m.get('permissions') or {})}")
        if args.out:
            data = package(folder, files)
            name = f"{m['id']}-{m['version']}.forgewidget"
            out = Path(args.out) / "packages"
            out.mkdir(parents=True, exist_ok=True)
            (out / name).write_bytes(data)
            icon = m.get("icon")
            icon_url = None
            if isinstance(icon, str) and (folder / icon).is_file() and icon.lower().endswith((".svg", ".png")):
                (Path(args.out) / "icons").mkdir(parents=True, exist_ok=True)
                ext = icon.rsplit(".", 1)[1].lower()
                (Path(args.out) / "icons" / f"{m['id']}.{ext}").write_bytes((folder / icon).read_bytes())
                icon_url = f"icons/{m['id']}.{ext}"
            entries.append({
                "id": m["id"], "name": m["name"], "version": str(m["version"]), "author": m["author"],
                "description": m["description"], "category": m.get("category", "other"),
                "permissions": m.get("permissions") or {},
                "html": bool((m.get("ui") or {}).get("entry")),
                "download": f"packages/{name}", "sha256": hashlib.sha256(data).hexdigest(), "size": len(data),
                "icon": icon_url, "homepage": m.get("homepage"),
            })
    if failed:
        sys.exit(1)
    if args.out:
        index = {
            "format": "widgetforge-index", "version": 1,
            "generated": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
            "widgets": sorted(entries, key=lambda e: e["name"].lower()),
        }
        Path(args.out, "index.json").write_text(json.dumps(index, indent=1), encoding="utf-8")
        print(f"wrote {len(entries)} widgets to {args.out}/index.json")


if __name__ == "__main__":
    main()
