#!/usr/bin/env python3
"""Builds the third-party credits from what the app actually ships.

Writes:
  public/credits/third-party.json      list shown on the Hub's Credits page
  public/credits/THIRD-PARTY-NOTICES.txt  full license texts, as MIT/Apache/BSD require

Rust packages come from `cargo metadata` (everything compiled into the app). JavaScript
packages are the ones bundled into the page (svelte, @tauri-apps/api); build tools
such as Vite are not shipped and are not listed. License files are read from the local
cargo registry and node_modules, so nothing is downloaded.

Run from the project folder:  python3 tools/gen_credits.py
"""
import json
import os
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = os.path.join(ROOT, "public", "credits")
SHIPPED_JS = ["svelte", "@tauri-apps/api"]
LICENSE_NAMES = ("license", "licence", "copying", "notice", "copyright", "unlicense")


def license_files(folder):
    found = []
    try:
        names = sorted(os.listdir(folder))
    except OSError:
        return found
    for name in names:
        low = name.lower()
        if low.startswith(LICENSE_NAMES) and os.path.isfile(os.path.join(folder, name)):
            found.append(os.path.join(folder, name))
    return found


def rust_packages():
    meta = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--format-version", "1", "--filter-platform", host_triple()],
        cwd=os.path.join(ROOT, "src-tauri")))
    # Only packages reachable from the app in the resolved graph for this platform.
    nodes = {n["id"]: n for n in meta["resolve"]["nodes"]}
    root = meta["resolve"]["root"]
    seen, stack = set(), [root]
    while stack:
        pid = stack.pop()
        if pid in seen:
            continue
        seen.add(pid)
        for dep in nodes[pid]["deps"]:
            kinds = {k["kind"] for k in dep["dep_kinds"]}
            if kinds - {"dev"}:
                stack.append(dep["pkg"])
    out = []
    for p in meta["packages"]:
        if p["id"] not in seen or p["id"] == root:
            continue
        out.append({
            "name": p["name"], "version": p["version"], "kind": "rust",
            "license": p.get("license") or "see license file",
            "url": p.get("repository") or p.get("homepage") or f"https://crates.io/crates/{p['name']}",
            "authors": p.get("authors") or [],
            "files": license_files(os.path.dirname(p["manifest_path"])),
        })
    return out


def host_triple():
    v = subprocess.check_output(["rustc", "-vV"], text=True)
    return next(l.split(": ")[1] for l in v.splitlines() if l.startswith("host: "))


def js_packages():
    out = []
    for name in SHIPPED_JS:
        folder = os.path.join(ROOT, "node_modules", name)
        with open(os.path.join(folder, "package.json")) as f:
            pj = json.load(f)
        repo = pj.get("repository")
        if isinstance(repo, dict):
            repo = repo.get("url")
        out.append({
            "name": name, "version": pj["version"], "kind": "javascript",
            "license": pj.get("license", "see license file"),
            "url": (repo or pj.get("homepage") or f"https://www.npmjs.com/package/{name}").replace("git+", ""),
            "authors": [pj["author"]["name"] if isinstance(pj.get("author"), dict) else pj.get("author", "")],
            "files": license_files(folder),
        })
    return out


def main():
    packages = sorted(rust_packages() + js_packages(), key=lambda p: (p["kind"], p["name"].lower()))
    os.makedirs(OUT, exist_ok=True)
    missing = [p["name"] for p in packages if not p["files"]]
    with open(os.path.join(OUT, "THIRD-PARTY-NOTICES.txt"), "w", encoding="utf-8") as f:
        f.write("WidgetForge includes the following third-party software.\n")
        f.write("Each is used under the license shown; the license texts follow.\n\n")
        for p in packages:
            f.write("=" * 78 + "\n")
            f.write(f"{p['name']} {p['version']} ({p['kind']})\nLicense: {p['license']}\nSource: {p['url']}\n")
            if p["authors"]:
                f.write("Authors: " + ", ".join(a for a in p["authors"] if a) + "\n")
            f.write("\n")
            for path in p["files"]:
                with open(path, encoding="utf-8", errors="replace") as lf:
                    f.write(f"--- {os.path.basename(path)} ---\n{lf.read().strip()}\n\n")
            if not p["files"]:
                f.write(f"(No license file in the package; licensed as: {p['license']})\n\n")
    listing = [{k: p[k] for k in ("name", "version", "kind", "license", "url")} for p in packages]
    with open(os.path.join(OUT, "third-party.json"), "w", encoding="utf-8") as f:
        json.dump({"packages": listing, "missing_license_files": missing}, f, indent=1)
    print(f"{len(packages)} packages, {len(missing)} without a license file: {', '.join(missing) or 'none'}")


if __name__ == "__main__":
    sys.exit(main())
