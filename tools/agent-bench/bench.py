#!/usr/bin/env python3
"""Forge Assistant model benchmark (stdlib only).

Runs candidate models over a fixed set of widget-building tasks through an
OpenAI-compatible API (OpenRouter by default), validates each manifest against
widget.schema.json plus WidgetForge's semantic rules, scores whether the widget
matches the request, and writes results.json + RESULTS.md.

See README.md for usage.
"""
import argparse
import hashlib
import colorsys
import concurrent.futures as cf
import json
import os
import re
import sys
import time
import urllib.error
import urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
API = os.environ.get("BENCH_API", "https://openrouter.ai/api/v1")
TOKEN = os.environ.get("OPENROUTER_API_KEY", "sk-or-dummy")  # a proxy may inject the real one
MAX_TOKENS = 1500
TEMPERATURE = 0.2

# id -> metadata. params in billions (text model); vram = Q4_K_M weights + ~1 GB context/runtime.
# Licences are from general knowledge and MUST be verified before redistribution.
MODELS = {
    "google/gemma-3-4b-it":            {"params": 3.9, "vram": "~2.5 + 1 = ~3.5 GB", "licence": "Gemma Terms of Use (custom, redistribution allowed with use policy) - verify", "local": True},
    "mistralai/ministral-3b-2512":     {"params": 3.4, "vram": "~2.1 + 1 = ~3.1 GB", "licence": "Apache 2.0 - verify", "local": True},
    "meta-llama/llama-3.2-3b-instruct": {"params": 3.2, "vram": "~2.0 + 1 = ~3.0 GB", "licence": "Llama 3.2 Community Licence (attribution, AUP, 700M MAU) - verify", "local": True},
    "liquid/lfm-3-1.7b":               {"params": 1.7, "vram": "~1.1 + 1 = ~2.1 GB", "licence": "LFM Open Licence (revenue cap for commercial use) - verify", "local": True},
    "qwen/qwen3.5-9b":                 {"params": 9.0, "vram": "~5.6 + 1 = ~6.6 GB (over 6 GB; partial offload)", "licence": "Apache 2.0 - verify", "local": True},
    "mistralai/ministral-8b-2512":     {"params": 8.4, "vram": "~5.1 + 1 = ~6.1 GB (borderline)", "licence": "Apache 2.0 - verify", "local": True},
    "ibm-granite/granite-4.2-8b":      {"params": 8.0, "vram": "~4.9 + 1 = ~5.9 GB (borderline)", "licence": "Apache 2.0 - verify", "local": True},
    "google/gemini-3.1-flash-lite":    {"params": None, "vram": "cloud", "licence": "proprietary API", "local": False},
}
# Models that think by default: ask the provider to turn reasoning off (the shipped
# assistant runs in non-thinking mode to keep latency and tokens low).
NO_REASONING = {"qwen/qwen3.5-9b", "ibm-granite/granite-4.2-8b", "liquid/lfm-3-1.7b", "google/gemini-3.1-flash-lite"}

# --------------------------------------------------------------------------- HTTP

def http(method, path, body=None, timeout=180):
    data = json.dumps(body).encode() if body is not None else None
    req = urllib.request.Request(API + path, data=data, method=method, headers={
        "Authorization": "Bearer " + TOKEN, "Content-Type": "application/json",
        "X-Title": "widgetforge-agent-bench"})
    try:
        with urllib.request.urlopen(req, timeout=timeout) as r:
            return r.status, json.loads(r.read().decode() or "null")
    except urllib.error.HTTPError as e:
        txt = e.read().decode(errors="replace")
        try:
            return e.code, json.loads(txt)
        except ValueError:
            return e.code, {"error": {"message": txt[:500]}}
    except Exception as e:  # timeout, connection reset
        return 0, {"error": {"message": f"{type(e).__name__}: {e}"}}


def key_remaining():
    st, d = http("GET", "/key", timeout=30)
    if st != 200:
        raise SystemExit(f"cannot read /key: {st} {d}")
    return float(d["data"]["limit_remaining"])


def fetch_pricing():
    st, d = http("GET", "/models", timeout=60)
    if st != 200:
        return {}
    return {m["id"]: {"prompt": float(m["pricing"]["prompt"]), "completion": float(m["pricing"]["completion"])}
            for m in d["data"]}

# --------------------------------------------------------------------- validation

def load_schema():
    with open(os.path.join(HERE, "widget.schema.json")) as f:
        return json.load(f)


def request_schema(schema):
    """Copy sent in response_format: without $id/title, which some providers echo
    into the output when they do not enforce the schema."""
    return {k: v for k, v in schema.items() if k not in ("$id", "title", "$schema")}


class MiniValidator:
    """Tiny JSON Schema validator for the subset widget.schema.json uses.

    anyOf branches that discriminate on a `type` const are resolved by that
    const, so errors name the real problem instead of "matches no branch".
    """
    TYPES = {"object": dict, "array": list, "string": str, "boolean": bool, "integer": int, "number": (int, float)}

    def __init__(self, schema):
        self.root = schema

    def _deref(self, s):
        while "$ref" in s:
            node = self.root
            for part in s["$ref"].lstrip("#/").split("/"):
                node = node[part]
            s = node
        return s

    def _type_ok(self, v, t):
        if isinstance(t, list):
            return any(self._type_ok(v, x) for x in t)
        if t in ("integer", "number") and isinstance(v, bool):
            return False
        if t == "integer":
            return isinstance(v, int) or (isinstance(v, float) and v.is_integer())
        return isinstance(v, self.TYPES[t])

    def _discriminator(self, branches):
        out = {}
        for b in branches:
            b2 = self._deref(b)
            if "anyOf" in b2:
                sub = self._discriminator(b2["anyOf"])
                if sub is None:
                    return None
                out.update(sub)
                continue
            c = b2.get("properties", {}).get("type", {}).get("const")
            if c is None:
                return None
            out.setdefault(c, []).append(b2)
        return out

    def errors(self, v, s=None, path="$"):
        s = self._deref(self.root if s is None else s)
        errs = []
        if "anyOf" in s:
            disc = self._discriminator(s["anyOf"])
            if disc is not None and isinstance(v, dict):
                cands = disc.get(v.get("type"))
                if not cands:
                    return [f"{path}.type: unknown or not-allowed block type {v.get('type')!r} here (allowed: {', '.join(sorted(disc))})"]
                best = None
                for c in cands:
                    e = self.errors(v, c, path)
                    if not e:
                        return []
                    if best is None or len(e) < len(best):
                        best = e
                return best
            per = [self.errors(v, b, path) for b in s["anyOf"]]
            if any(not e for e in per):
                return []
            return [f"{path}: does not match any allowed form ({min(per, key=len)[0]})"]
        if "const" in s and v != s["const"]:
            errs.append(f"{path}: must be {json.dumps(s['const'])}")
        if "enum" in s and v not in s["enum"]:
            errs.append(f"{path}: {json.dumps(v)} not one of {s['enum']}")
        if "type" in s and not self._type_ok(v, s["type"]):
            return errs + [f"{path}: expected {s['type']}, got {type(v).__name__}"]
        if isinstance(v, str):
            if "pattern" in s and not re.search(s["pattern"], v):
                errs.append(f"{path}: {v!r} does not match {s['pattern']}")
            if "minLength" in s and len(v) < s["minLength"]:
                errs.append(f"{path}: too short")
            if "maxLength" in s and len(v) > s["maxLength"]:
                errs.append(f"{path}: longer than {s['maxLength']} chars")
        if isinstance(v, (int, float)) and not isinstance(v, bool):
            if "minimum" in s and v < s["minimum"]:
                errs.append(f"{path}: {v} < minimum {s['minimum']}")
            if "maximum" in s and v > s["maximum"]:
                errs.append(f"{path}: {v} > maximum {s['maximum']}")
        if isinstance(v, list):
            if "minItems" in s and len(v) < s["minItems"]:
                errs.append(f"{path}: needs at least {s['minItems']} items")
            if "maxItems" in s and len(v) > s["maxItems"]:
                errs.append(f"{path}: at most {s['maxItems']} items")
            if s.get("uniqueItems") and len({json.dumps(x, sort_keys=True) for x in v}) != len(v):
                errs.append(f"{path}: items must be unique")
            if "items" in s:
                for i, x in enumerate(v):
                    errs += self.errors(x, s["items"], f"{path}[{i}]")
        if isinstance(v, dict):
            props = s.get("properties", {})
            for r in s.get("required", []):
                if r not in v:
                    errs.append(f"{path}: missing required '{r}'")
            for k, x in v.items():
                if k in props:
                    errs += self.errors(x, props[k], f"{path}.{k}")
                else:
                    ap = s.get("additionalProperties", True)
                    if ap is False:
                        errs.append(f"{path}: unknown key '{k}'")
                    elif isinstance(ap, dict):
                        errs += self.errors(x, ap, f"{path}.{k}")
                if "propertyNames" in s and not re.search(s["propertyNames"].get("pattern", ""), k):
                    errs.append(f"{path}: key {k!r} does not match {s['propertyNames']['pattern']}")
        return errs


SYSTEM_FIELDS = {
    "cpu": ["cpu.usage", "cpu.cores", "cpu.name"],
    "memory": ["memory.used", "memory.total", "memory.usage", "swap.used", "swap.total"],
    "disk": ["disk.used", "disk.total", "disk.usage", "disks"],
    "network": ["network.rx", "network.tx"],
    "os": ["uptime", "os.name", "os.version", "os.hostname"],
}
FIELD_PERM = {f: p for p, fs in SYSTEM_FIELDS.items() for f in fs}
CLOCK_FIELDS = {"now", "timezone", "locale"}
ARRAY_FIELDS = {"cpu.cores": {"usage"}, "disks": {"name", "mount", "used", "total", "usage"}}
FILTERS = {"fixed", "round", "percent", "bytes", "rate", "date", "time", "upper", "lower", "default"}
TPL_RE = re.compile(r"\{\{(.*?)\}\}")
EXPR_RE = re.compile(r"^\s*([A-Za-z_][\w.]*)\s*((?:\|\s*[A-Za-z_]\w*\s*(?:\([^()]*\))?\s*)*)$")
FILTER_RE = re.compile(r"\|\s*([A-Za-z_]\w*)\s*(\([^()]*\))?")
TEMPLATE_KEYS = ("text", "value", "max", "format", "label", "items", "item", "visible", "src")


def walk(blocks, depth=0, parent=None):
    for b in blocks or []:
        if not isinstance(b, dict):
            continue
        yield b, depth, parent
        if isinstance(b.get("children"), list):
            yield from walk(b["children"], depth + 1, b)


def parse_templates(s):
    """-> list of (path, [filters]) or raises ValueError."""
    if not isinstance(s, str):
        return []
    if s.count("{{") != s.count("}}"):
        raise ValueError(f"unbalanced braces in {s!r}")
    out = []
    for expr in TPL_RE.findall(s):
        m = EXPR_RE.match(expr)
        if not m:
            raise ValueError(f"bad binding {{{{{expr}}}}}")
        out.append((m.group(1), [f for f, _ in FILTER_RE.findall(m.group(2) or "")]))
    return out


class Manifest:
    """Helpers for checks: resolves bindings to (source, field)."""

    def __init__(self, m):
        self.m = m
        data = m.get("data") if isinstance(m.get("data"), dict) else {}
        self.sources = {k: v.get("source") for k, v in data.items() if isinstance(v, dict)}
        self.blocks = list(walk((m.get("ui") or {}).get("blocks")))

    def refs(self, s, in_item=None):
        """Set of 'system:<field>' / 'clock:<field>' / 'item:<field>' / 'settings:<key>' bound in s."""
        out = set()
        try:
            exprs = parse_templates(s)
        except ValueError:
            return out
        for path, _ in exprs:
            root, _, rest = path.partition(".")
            if root in ("settings", "item", "instance"):
                out.add(f"{root}:{rest}")
            elif root in self.sources:
                out.add(f"{self.sources[root]}:{rest}")
        return out

    def filters(self, s):
        try:
            return {f for _, fs in parse_templates(s) for f in fs}
        except ValueError:
            return set()

    def of_type(self, t):
        return [b for b, _, _ in self.blocks if b.get("type") == t]

    def perms(self):
        p = self.m.get("permissions") or {}
        return set(p.get("system") or []), list(p.get("network") or []), bool(p.get("storage"))

    def used_perms(self):
        used = set()
        for b, _, _ in self.blocks:
            for k in TEMPLATE_KEYS:
                for r in self.refs(b.get(k)):
                    src, _, f = r.partition(":")
                    if src == "system":
                        for fld, p in FIELD_PERM.items():
                            if f == fld or f.startswith(fld + "."):
                                used.add(p)
        return used

    def colors(self):
        out = [((self.m.get("style") or {}).get("accent"))]
        out += [b.get("color") for b, _, _ in self.blocks]
        return [c for c in out if isinstance(c, str)]


def lint(m):
    """WidgetForge rules beyond the schema. -> (errors, warnings)."""
    errs, warns = [], []
    if not isinstance(m, dict):
        return ["manifest is not an object"], warns
    M = Manifest(m)
    win = m.get("window") or {}
    if win.get("mode") == "drawer" and "edge" not in win:
        errs.append("window.edge is required when window.mode is 'drawer'")
    if win.get("mode") == "tile" and "edge" in win:
        warns.append("window.edge is ignored for a tile")
    # Checks kept out of the schema because grammar engines reject the keywords
    # (uniqueItems, propertyNames).
    for name in M.sources:
        if name in ("settings", "item", "instance"):
            errs.append(f"data name '{name}' is reserved")
        elif not re.match(r"^[a-z][a-z0-9_]*$", name):
            errs.append(f"data name '{name}' must match ^[a-z][a-z0-9_]*$")
    sysp = (m.get("permissions") or {}).get("system") or []
    if isinstance(sysp, list) and len(sysp) != len(set(map(str, sysp))):
        errs.append("permissions.system has duplicate entries")
    setting_keys = {s.get("key") for s in (m.get("settings") or []) if isinstance(s, dict)}
    perms, net, storage = M.perms()
    for b, depth, parent in M.blocks:
        t = b.get("type")
        where = f"{t} block"
        for k in TEMPLATE_KEYS:
            v = b.get(k)
            if not isinstance(v, str):
                continue
            try:
                exprs = parse_templates(v)
            except ValueError as e:
                errs.append(f"{where}.{k}: {e}")
                continue
            for path, fl in exprs:
                for f in fl:
                    if f not in FILTERS:
                        errs.append(f"{where}.{k}: unknown filter '{f}' (allowed: {', '.join(sorted(FILTERS))})")
                root, _, rest = path.partition(".")
                if root == "settings":
                    if rest not in setting_keys:
                        errs.append(f"{where}.{k}: settings.{rest} is not defined in settings")
                elif root == "item":
                    if not (t == "list" and k == "item"):
                        errs.append(f"{where}.{k}: item.* is only valid inside list.item")
                    elif rest not in set().union(*ARRAY_FIELDS.values()):
                        errs.append(f"{where}.{k}: unknown item field 'item.{rest}'")
                elif root == "instance":
                    pass
                elif root not in M.sources:
                    errs.append(f"{where}.{k}: '{root}' in {{{{ {path} }}}} is not a declared data name (declared: {', '.join(M.sources) or 'none'})")
                elif M.sources[root] == "clock":
                    if rest not in CLOCK_FIELDS:
                        errs.append(f"{where}.{k}: clock field '{rest or '(none)'}' does not exist; use {root}.now")
                elif M.sources[root] == "system":
                    if rest not in FIELD_PERM and not rest.startswith("cpu.cores.") and not rest.startswith("disks."):
                        errs.append(f"{where}.{k}: system field '{rest}' does not exist")
                    else:
                        p = FIELD_PERM.get(rest) or ("cpu" if rest.startswith("cpu.") else "disk")
                        if p not in perms:
                            errs.append(f"{where}.{k}: '{rest}' needs \"{p}\" in permissions.system")
        if t == "list":
            r = M.refs(b.get("items"))
            if not any(x.split(":", 1)[1] in ARRAY_FIELDS for x in r if x.startswith("system:")):
                errs.append(f"list.items must bind an array field (disks or cpu.cores), got {b.get('items')!r}")
        if t in ("notes", "checklist") and not storage:
            errs.append(f"{t} block needs permissions.storage: true")
        if b.get("children") is not None and depth >= 2:
            errs.append("containers nested deeper than 2 levels")
    keys = [b.get("key") for b, _, _ in M.blocks if b.get("type") in ("notes", "checklist")]
    if len(keys) != len(set(keys)):
        errs.append("notes/checklist keys must be unique")
    if any(src == "system" for src in M.sources.values()) and not perms:
        errs.append("a system data source is declared but permissions.system is empty")
    unused = perms - M.used_perms()
    if unused:
        warns.append(f"unused system permissions: {sorted(unused)}")
    if net:
        warns.append("permissions.network should be [] for declarative widgets")
    if storage and not keys:
        warns.append("storage requested but no notes/checklist block")
    return errs, warns

# ---------------------------------------------------------------- semantic checks

def hue_of(c):
    try:
        r, g, b = (int(c[i:i + 2], 16) / 255 for i in (1, 3, 5))
    except (ValueError, TypeError, IndexError):
        return None, 0, 0
    h, l, s = colorsys.rgb_to_hls(r, g, b)
    return h * 360, s, l


def is_red(c):
    h, s, l = hue_of(c)
    return h is not None and s > 0.4 and 0.2 < l < 0.8 and (h >= 340 or h <= 20)


def is_yellow(c):
    h, s, l = hue_of(c)
    return h is not None and s > 0.4 and l > 0.3 and 40 <= h <= 65


def is_blue(c):
    h, s, l = hue_of(c)
    return h is not None and s > 0.3 and 190 <= h <= 250


def any_ref(M, blocks, keys, fields):
    return any(f"system:{f}" in M.refs(b.get(k)) for b in blocks for k in keys for f in fields)


def least_privilege(M, needs_storage=False):
    perms, net, storage = M.perms()
    return perms == M.used_perms() and not net and storage == needs_storage


def inside(M, child_types, container_types=("row", "grid"), n=2):
    for b, _, _ in M.blocks:
        if b.get("type") in container_types:
            kids = [k for k, _, _ in walk(b.get("children"))]
            if sum(1 for k in kids if k.get("type") in child_types) >= n:
                return True
    return False


def order_ok(M, first, second):
    """Index of first matching block precedes index of first matching second block."""
    seq = [b for b, _, _ in M.blocks]
    i = next((n for n, b in enumerate(seq) if first(b)), None)
    j = next((n for n, b in enumerate(seq) if second(b)), None)
    return i is not None and j is not None and i < j


def chk_cpu_ram(M):
    g = M.of_type("gauge")
    return {
        "gauge_cpu": any_ref(M, g, ["value"], ["cpu.usage"]),
        "gauge_ram": any_ref(M, g, ["value"], ["memory.usage", "memory.used"]),
        "side_by_side": inside(M, {"gauge"}),
        "percent_shown": any(("%" in str(b.get("format", ""))) or "percent" in M.filters(b.get("format")) for b in g),
        "least_privilege": least_privilege(M),
    }


def chk_clock_date(M):
    has_date = lambda b: b.get("type") == "text" and "date" in M.filters(b.get("text")) and "clock:now" in M.refs(b.get("text"))
    return {
        "analog_clock": bool(M.of_type("clock")),
        "date_text": any(has_date(b) for b in M.of_type("text")),
        "date_below_clock": order_ok(M, lambda b: b.get("type") == "clock", has_date),
        "category_time": M.m.get("category") == "time",
        "least_privilege": least_privilege(M),
    }


def chk_sticky(M):
    return {
        "notes_block": bool(M.of_type("notes")),
        "storage": M.perms()[2],
        "yellow": any(is_yellow(c) for c in M.colors()),
        "category_productivity": M.m.get("category") == "productivity",
        "least_privilege": least_privilege(M, needs_storage=True),
    }


def chk_todo(M):
    return {
        "checklist_block": bool(M.of_type("checklist")),
        "storage": M.perms()[2],
        "title_today": any("today" in str(b.get("text", "")).lower() for b in M.of_type("text")) or "today" in str(M.m.get("name", "")).lower() and bool(M.of_type("text")),
        "least_privilege": least_privilege(M, needs_storage=True),
    }


def chk_drawer(M):
    w = M.m.get("window") or {}
    bars = M.of_type("bar")
    return {
        "drawer": w.get("mode") == "drawer",
        "edge_right": w.get("edge") == "right",
        "layer_top": w.get("layer") == "top",
        "bar_cpu": any_ref(M, bars, ["value"], ["cpu.usage"]),
        "bar_memory": any_ref(M, bars, ["value"], ["memory.usage", "memory.used"]),
        "bar_disk": any_ref(M, bars, ["value"], ["disk.usage", "disk.used"]),
        "least_privilege": least_privilege(M),
    }


def chk_uptime(M):
    txt = M.of_type("text")
    host = lambda b: b.get("type") == "text" and "system:os.hostname" in M.refs(b.get("text"))
    up = lambda b: b.get("type") == "text" and "system:uptime" in M.refs(b.get("text"))
    return {
        "hostname_text": any(host(b) for b in txt),
        "hostname_big_bold": any(host(b) and b.get("size") in ("lg", "xl", "xxl") and b.get("weight") == "bold" for b in txt),
        "uptime_text": any(up(b) for b in txt),
        "uptime_below": order_ok(M, host, up),
        "least_privilege": least_privilege(M),
    }


def chk_net(M):
    gr = M.of_type("graph")
    return {
        "graph_download": any_ref(M, gr, ["value"], ["network.rx"]),
        "graph_upload": any_ref(M, gr, ["value"], ["network.tx"]),
        "rate_filter": any("rate" in M.filters(b.get(k)) for b in gr + M.of_type("text") for k in ("format", "text", "label")),
        "least_privilege": least_privilege(M),
    }


def chk_disks(M):
    lists = [b for b in M.of_type("list") if "system:disks" in M.refs(b.get("items"))]
    item_refs = set().union(*(M.refs(b.get("item")) for b in lists)) if lists else set()
    return {
        "list_of_disks": bool(lists),
        "shows_mount": "item:mount" in item_refs,
        "shows_fullness": "item:usage" in item_refs or {"item:used", "item:total"} <= item_refs,
        "least_privilege": least_privilege(M),
    }


QL = {"github.com": "github", "www.youtube.com": "youtube", "mail.google.com": "gmail"}


def host_of(u):
    m = re.match(r"^https?://([^/]+)", str(u or ""))
    return m.group(1).lower() if m else ""


def chk_quick(M):
    btn = M.of_type("button")
    hosts = {host_of(b.get("open")).removeprefix("www.") for b in btn}
    return {
        "three_buttons": len(btn) == 3,
        "in_a_row": inside(M, {"button"}, ("row",), 3),
        "urls_correct": hosts == {"github.com", "youtube.com", "mail.google.com"},
        "labels": all(any(lbl in str(b.get("label", "")).lower() for b in btn) for lbl in QL.values()),
        "least_privilege": least_privilege(M),
    }


EDIT_BASE = {
    "schema": 1, "id": "user.cpu-gauge", "name": "CPU Gauge", "version": "1.0.0",
    "description": "CPU usage as a ring gauge.", "category": "system",
    "window": {"mode": "tile", "size": [200, 200], "layer": "desktop"},
    "style": {"theme": "glass", "accent": "#3b82f6"},
    "permissions": {"system": ["cpu"], "network": [], "storage": False},
    "settings": [], "data": {"sys": {"source": "system", "interval": 1000}},
    "ui": {"blocks": [{"type": "column", "gap": 8, "children": [
        {"type": "text", "text": "CPU", "size": "md", "weight": "bold", "align": "center"},
        {"type": "gauge", "label": "CPU", "value": "{{ sys.cpu.usage }}", "max": 100,
         "format": "{{ sys.cpu.usage | fixed(0) }}%", "color": "#3b82f6"}]}]},
}


def chk_edit(M):
    perms = M.perms()[0]
    return {
        "id_kept": M.m.get("id") == EDIT_BASE["id"],
        "accent_red": is_red((M.m.get("style") or {}).get("accent")),
        "no_blue_left": not any(is_blue(c) for c in M.colors()),
        "cpu_gauge_kept": any_ref(M, M.of_type("gauge"), ["value"], ["cpu.usage"]),
        "ram_bar": any_ref(M, M.of_type("bar"), ["value"], ["memory.usage", "memory.used"]),
        "perms_cpu_memory": {"cpu", "memory"} <= perms,
    }


TASKS = [
    {"id": "cpu_ram_gauges", "prompt": "A compact widget with two ring gauges side by side: CPU usage and RAM usage, with percentages.", "check": chk_cpu_ram},
    {"id": "clock_date", "prompt": "An analog clock with today's date written underneath it, like 'Friday, 20 November'.", "check": chk_clock_date},
    {"id": "sticky_note", "prompt": "A yellow sticky note I can type quick notes into.", "check": chk_sticky},
    {"id": "todo_checklist", "prompt": "A to-do list titled 'Today' where I can tick items off.", "check": chk_todo},
    {"id": "right_drawer", "prompt": "A drawer that slides out from the right edge of the screen and shows CPU, memory and disk usage as horizontal bars. Keep it above other windows.", "check": chk_drawer},
    {"id": "uptime_hostname", "prompt": "Show my computer's hostname in large bold text, with the system uptime underneath.", "check": chk_uptime},
    {"id": "network_graph", "prompt": "A network monitor with two sparkline graphs, download speed and upload speed, shown as rates.", "check": chk_net},
    {"id": "disk_list", "prompt": "List all my disks, showing each one's mount point and how full it is in percent.", "check": chk_disks},
    {"id": "quick_launch", "prompt": "A quick-launch bar: a row of three buttons that open GitHub (https://github.com), YouTube (https://www.youtube.com) and Gmail (https://mail.google.com).", "check": chk_quick},
    {"id": "edit_red_ram", "prompt": "Make it red and add a RAM bar.", "base": EDIT_BASE, "check": chk_edit},
]

# --------------------------------------------------------------------- model calls

def user_message(task):
    if task.get("base"):
        return ("Current manifest:\n" + json.dumps(task["base"], separators=(",", ":")) +
                "\n\nRequest: " + task["prompt"])
    return "Request: " + task["prompt"]


def build_body(model, messages, mode, schema, reasoning_off=True):
    body = {"model": model, "messages": messages, "max_tokens": MAX_TOKENS,
            "temperature": TEMPERATURE, "usage": {"include": True}}
    if mode == "json_schema":
        body["response_format"] = {"type": "json_schema", "json_schema": {"name": "widget_manifest", "strict": False, "schema": request_schema(schema)}}
        body["provider"] = {"require_parameters": True}
    elif mode == "json_object":
        body["response_format"] = {"type": "json_object"}
    if reasoning_off and model in NO_REASONING:
        body["reasoning"] = {"enabled": False}
    return body


def system_for(mode, base_prompt, schema):
    if mode == "json_schema":
        return base_prompt
    return base_prompt + "\n\n# JSON Schema the reply must satisfy\n" + json.dumps(request_schema(schema), separators=(",", ":"))


FALLBACK = {"json_schema": "json_object", "json_object": "prompt"}


def chat(model, messages_wo_system, mode, schema, base_prompt):
    """Call the model; downgrade mode on a capability rejection. Returns a dict."""
    reasoning_off = True
    tries = 0
    while True:
        tries += 1
        msgs = [{"role": "system", "content": system_for(mode, base_prompt, schema)}] + messages_wo_system
        body = build_body(model, msgs, mode, schema, reasoning_off)
        t0 = time.time()
        st, d = http("POST", "/chat/completions", body)
        lat = time.time() - t0
        err = (d or {}).get("error") if isinstance(d, dict) else None
        content = None
        if st == 200 and not err:
            ch = (d.get("choices") or [{}])[0]
            content = (ch.get("message") or {}).get("content")
            if content:
                return {"ok": True, "mode": mode, "content": content, "latency": lat, "usage": d.get("usage") or {},
                        "provider": d.get("provider"), "finish": ch.get("finish_reason"), "reasoning_off": reasoning_off}
            err = {"message": f"empty content (finish={ch.get('finish_reason')})"}
        msg = json.dumps(err)[:400] if err else f"HTTP {st}"
        low = msg.lower()
        if reasoning_off and model in NO_REASONING and "reason" in low and st in (400, 404):
            reasoning_off = False
            continue
        if st in (400, 404) and mode in FALLBACK and ("response_format" in low or "json" in low or "no endpoints" in low or "schema" in low or "grammar" in low or "parameter" in low):
            mode = FALLBACK[mode]
            continue
        if tries < 4 and (st in (0, 408, 429, 500, 502, 503, 504) or ("empty content" in low and "length" not in low)):
            time.sleep(5 * tries)
            continue
        return {"ok": False, "mode": mode, "error": f"HTTP {st}: {msg}", "latency": lat, "usage": (d or {}).get("usage") or {} if isinstance(d, dict) else {}}


def extract_json(text):
    """Strict parse first; then strip code fences / surrounding prose. -> (obj|None, how)."""
    try:
        return json.loads(text), "strict"
    except (ValueError, TypeError):
        pass
    t = re.sub(r"^\s*```(?:json)?\s*|\s*```\s*$", "", text or "")
    try:
        return json.loads(t), "fence_stripped"
    except ValueError:
        pass
    i, j = (text or "").find("{"), (text or "").rfind("}")
    if i >= 0 and j > i:
        try:
            return json.loads(text[i:j + 1]), "extracted"
        except ValueError:
            pass
    return None, "unparseable"


def cost_of(usage, model, pricing):
    if usage.get("cost") is not None:
        return float(usage["cost"]), "reported"
    p = pricing.get(model)
    if not p:
        return 0.0, "unknown"
    return usage.get("prompt_tokens", 0) * p["prompt"] + usage.get("completion_tokens", 0) * p["completion"], "computed"


def evaluate(task, content, validator):
    obj, how = extract_json(content)
    rec = {"parse": how}
    if obj is None:
        rec.update(schema_errors=["not valid JSON"], lint_errors=[], warnings=[], checks={}, valid=False, semantic=0.0)
        return rec, None
    se = validator.errors(obj)
    le, w = lint(obj) if isinstance(obj, dict) else (["not an object"], [])
    checks = task["check"](Manifest(obj)) if isinstance(obj, dict) else {}
    rec.update(schema_errors=se[:20], lint_errors=le[:20], warnings=w, checks=checks,
               valid=not se and not le,
               semantic=round(sum(checks.values()) / len(checks), 3) if checks else 0.0)
    return rec, obj


def run_one(model, task, run_idx, mode, schema, base_prompt, validator, pricing, repair):
    msgs = [{"role": "user", "content": user_message(task)}]
    r = chat(model, msgs, mode, schema, base_prompt)
    rec = {"model": model, "task": task["id"], "run": run_idx, "mode": r["mode"], "latency_s": round(r["latency"], 2),
           "prompt_tokens": r["usage"].get("prompt_tokens"), "completion_tokens": r["usage"].get("completion_tokens"),
           "provider": r.get("provider"), "finish": r.get("finish"), "reasoning_off": r.get("reasoning_off")}
    c, how = cost_of(r["usage"], model, pricing)
    rec["cost_usd"], rec["cost_source"] = round(c, 7), how
    if not r["ok"]:
        rec.update(error=r["error"], parse="no_response", valid=False, semantic=0.0, checks={}, schema_errors=[], lint_errors=[], warnings=[])
        return rec
    ev, _ = evaluate(task, r["content"], validator)
    rec.update(ev)
    rec["raw"] = r["content"][:6000]
    rec["full_pass"] = rec["valid"] and rec["semantic"] == 1.0
    if repair:
        rec["repair"] = None
        if not rec["valid"]:
            errs = (rec["schema_errors"] + rec["lint_errors"])[:12]
            fb = ("Your manifest failed validation:\n- " + "\n- ".join(errs) +
                  "\nFix these problems and reply with the complete corrected manifest JSON only.")
            msgs2 = msgs + [{"role": "assistant", "content": r["content"]}, {"role": "user", "content": fb}]
            r2 = chat(model, msgs2, r["mode"], schema, base_prompt)
            c2, _ = cost_of(r2["usage"], model, pricing)
            rep = {"latency_s": round(r2["latency"], 2), "cost_usd": round(c2, 7),
                   "completion_tokens": r2["usage"].get("completion_tokens"), "feedback": errs}
            if r2["ok"]:
                ev2, _ = evaluate(task, r2["content"], validator)
                rep.update(ev2)
                rep["raw"] = r2["content"][:6000]
                rep["full_pass"] = rep["valid"] and rep["semantic"] == 1.0
            else:
                rep.update(error=r2["error"], valid=False, semantic=0.0, full_pass=False)
            rec["repair"] = rep
    return rec

# ----------------------------------------------------------------------- reporting

def summarize(records):
    by = {}
    for r in records:
        by.setdefault(r["model"], []).append(r)
    rows = []
    for model, rs in by.items():
        n = len(rs)
        lat = [r["latency_s"] for r in rs if r.get("latency_s") is not None and r.get("parse") != "no_response"]
        out = [r["completion_tokens"] for r in rs if r.get("completion_tokens")]
        modes = sorted({r["mode"] for r in rs})
        rows.append({
            "model": model, "n": n, "modes": "/".join(modes),
            "parse": sum(r.get("parse") in ("strict", "fence_stripped", "extracted") for r in rs) / n,
            "schema_ok": sum(r.get("parse") in ("strict", "fence_stripped", "extracted") and not r.get("schema_errors") for r in rs) / n,
            "valid": sum(bool(r.get("valid")) for r in rs) / n,
            "semantic": sum(r.get("semantic", 0) for r in rs) / n,
            "full_pass": sum(bool(r.get("full_pass")) for r in rs) / n,
            "latency": sum(lat) / len(lat) if lat else None,
            "out_tok": sum(out) / len(out) if out else None,
            "cost": sum(r.get("cost_usd") or 0 for r in rs),
            **{k: MODELS.get(model, {}).get(k) for k in ("params", "vram", "licence", "local")},
        })
    rows.sort(key=lambda x: (-(x["valid"] + x["semantic"] + x["full_pass"]), x["model"]))
    return rows


def pct(x):
    return f"{100 * x:.0f}%"


def table(rows):
    h = ("| Model | Params | Runs | Mode | Parse | Schema ok | Valid (schema+rules) | Semantic | Full pass | "
         "Avg latency | Avg out tok | Cost | VRAM Q4_K_M (est.) |\n|---|---|---|---|---|---|---|---|---|---|---|---|---|\n")
    for r in rows:
        h += (f"| `{r['model']}` | {str(r['params']) + 'B' if r['params'] else 'cloud'} | {r['n']} | {r['modes']} | "
              f"{pct(r['parse'])} | {pct(r['schema_ok'])} | {pct(r['valid'])} | {pct(r['semantic'])} | {pct(r['full_pass'])} | "
              f"{r['latency']:.1f}s | {r['out_tok']:.0f} | ${r['cost']:.4f} | {r['vram']} |\n") if r["latency"] else (
              f"| `{r['model']}` | - | {r['n']} | {r['modes']} | all calls failed | | | | | | | | |\n")
    return h


def per_task_table(records):
    models = []
    for r in records:
        if r["model"] not in models:
            models.append(r["model"])
    tasks = [t["id"] for t in TASKS]
    h = "| Task | " + " | ".join(f"`{m.split('/')[1]}`" for m in models) + " |\n|---|" + "---|" * len(models) + "\n"
    for t in tasks:
        cells = []
        for m in models:
            rs = [r for r in records if r["model"] == m and r["task"] == t]
            cells.append(" ".join(("V" if r.get("valid") else "x") + f"{r.get('semantic', 0):.0%}".replace("%", "") for r in rs) or "-")
        h += f"| {t} | " + " | ".join(cells) + " |\n"
    return h


NOTES_MARK = "<!-- notes: hand-written below this line, kept when the report is regenerated -->"


def write_report(res):
    main = [r for r in res["records"] if r.get("tag", "main") != "repair"]
    rep = [r for r in res["records"] if r.get("tag") == "repair"]
    md = ["# Forge Assistant benchmark results", "",
          f"Generated {res.get('generated')}. Endpoint: OpenRouter (provider-hosted, so latency is network + provider, "
          f"**not** local-GPU latency). temperature={TEMPERATURE}, max_tokens={MAX_TOKENS}, reasoning disabled where supported.", "",
          "**Valid** = JSON parses, matches `widget.schema.json`, and passes the WidgetForge rules (permissions match the "
          "fields used, bindings point at declared data / real fields / known filters, storage for notes/checklist, drawer has an edge). "
          "**Semantic** = share of per-task checks passed (does the widget do what was asked, least privilege). "
          "**Full pass** = valid and every semantic check passed.", "",
          "## Overall (all non-repair runs)", "", table(summarize(main)),
          "## Per task", "", "Each cell is one run: `V`/`x` = valid / invalid, then the semantic score (0-100).", "",
          per_task_table(main)]
    if rep:
        md += ["## Self-repair loop", "",
               "On an invalid manifest the validation errors (max 12) are sent back once as a follow-up user turn; "
               "the corrected manifest is validated again.", "",
               "| Model | Task runs | Needed repair | Valid before | Valid after | Full pass before | Full pass after | Extra cost | Avg extra latency |",
               "|---|---|---|---|---|---|---|---|---|"]
        for model in sorted({r["model"] for r in rep}):
            rs = [r for r in rep if r["model"] == model]
            n = len(rs)
            fixed = [r for r in rs if r.get("repair")]
            after = lambda r, k: bool((r.get("repair") or {}).get(k)) if r.get("repair") else bool(r.get(k))
            lat = [r["repair"]["latency_s"] for r in fixed]
            md.append(f"| `{model}` | {n} | {len(fixed)} | {sum(bool(r.get('valid')) for r in rs)}/{n} | "
                      f"{sum(after(r, 'valid') for r in rs)}/{n} | {sum(bool(r.get('full_pass')) for r in rs)}/{n} | "
                      f"{sum(after(r, 'full_pass') for r in rs)}/{n} | "
                      f"${sum((r.get('repair') or {}).get('cost_usd') or 0 for r in rs):.5f} | "
                      f"{(sum(lat) / len(lat)) if lat else 0:.1f}s |")
        md.append("")
        for r in rep:
            rr = r.get("repair")
            if not rr:
                continue
            md.append(f"- `{r['model'].split('/')[1]}` `{r['task']}` run {r['run']}: {len(rr.get('feedback', []))} error(s) fed back -> "
                      f"{'valid' if rr.get('valid') else 'still invalid'} (semantic {rr.get('semantic', 0):.0%}). "
                      f"First error: {rr.get('feedback', [''])[0][:120]}")
        md.append("")
    md += ["## Common validation errors", ""]
    errs = {}
    for r in main:
        for e in (r.get("schema_errors") or []) + (r.get("lint_errors") or []) + ([r["error"][:120]] if r.get("error") else []):
            key = re.sub(r"\[\d+\]", "[]", e)[:150]
            errs.setdefault(r["model"], {}).setdefault(key, 0)
            errs[r["model"]][key] += 1
    for m, d in errs.items():
        md.append(f"- `{m}`: " + "; ".join(f"{k} (x{v})" for k, v in sorted(d.items(), key=lambda kv: -kv[1])[:4]))
    md += ["", "## Spend", "",
           f"OpenRouter key `limit_remaining`: start {res.get('remaining_start')} -> end {res.get('remaining_end')} "
           f"(sum of per-call cost in results: ${sum((r.get('cost_usd') or 0) + ((r.get('repair') or {}).get('cost_usd') or 0) for r in res['records']):.4f}).", "",
           "## Models: VRAM and licence", "", "| Model | Params | VRAM at Q4_K_M (weights + ~1 GB ctx) | Licence |", "|---|---|---|---|"]
    for m, meta in MODELS.items():
        md.append(f"| `{m}` | {str(meta['params']) + 'B' if meta['params'] else 'cloud'} | {meta['vram']} | {meta['licence']} |")
    md += ["", NOTES_MARK, ""]
    path = os.path.join(HERE, "RESULTS.md")
    notes = ""
    if os.path.exists(path):
        old = open(path).read()
        if NOTES_MARK in old:
            notes = old.split(NOTES_MARK, 1)[1].lstrip("\n")
    with open(path, "w") as f:
        f.write("\n".join(md) + notes)

# ---------------------------------------------------------------------------- main

def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--models", default=",".join(MODELS), help="comma-separated model ids")
    ap.add_argument("--tasks", default="", help="comma-separated task ids (default: all)")
    ap.add_argument("--runs", type=int, default=1, help="runs per task per model")
    ap.add_argument("--repair", action="store_true", help="feed validation errors back once (records tagged 'repair')")
    ap.add_argument("--floor", type=float, default=float(os.environ.get("BENCH_BUDGET_FLOOR", "41.10")),
                    help="stop before a model batch if /key limit_remaining is below this")
    ap.add_argument("--workers", type=int, default=5)
    ap.add_argument("--fresh", action="store_true", help="discard previous results.json instead of appending")
    ap.add_argument("--report-only", action="store_true", help="only regenerate RESULTS.md from results.json")
    ap.add_argument("--tag", default=None, help="tag for these records (default: main, or repair with --repair)")
    ap.add_argument("--retry-failed", action="store_true",
                    help="re-run records that failed for infrastructure reasons (HTTP 429/5xx/timeouts) and replace them")
    a = ap.parse_args()

    res_path = os.path.join(HERE, "results.json")
    res = {"records": []}
    if os.path.exists(res_path) and not a.fresh:
        res = json.load(open(res_path))
    if a.report_only:
        write_report(res)
        return
    schema = load_schema()
    base_prompt = open(os.path.join(HERE, "system_prompt.md")).read().strip()
    validator = MiniValidator(schema)
    pricing = fetch_pricing()
    available = set(pricing)
    tasks = [t for t in TASKS if not a.tasks or t["id"] in a.tasks.split(",")]
    tag = a.tag or ("repair" if a.repair else "main")
    start = key_remaining()
    res.setdefault("remaining_start", start)
    print(f"limit_remaining at start: {start:.6f} (floor {a.floor})")
    if a.retry_failed:
        infra = re.compile(r"^HTTP (0|408|429|5\d\d):")
        bad = [r for r in res["records"] if infra.match(r.get("error") or "")]
        task_by_id = {t["id"]: t for t in TASKS}
        if bad and key_remaining() < a.floor:
            raise SystemExit("STOP: below budget floor")
        for r in bad:
            new = run_one(r["model"], task_by_id[r["task"]], r["run"], r["mode"], schema, base_prompt, validator, pricing,
                          r.get("tag") == "repair")
            new["tag"] = r.get("tag", "main")
            new["prompt_sha"] = hashlib.sha1(base_prompt.encode()).hexdigest()[:8]
            res["records"][res["records"].index(r)] = new
            print(f"  retried {r['model']} {r['task']} run{r['run']}: {'V' if new.get('valid') else 'x'} {(new.get('error') or '')[:100]}")
        res["remaining_end"] = key_remaining()
        with open(res_path, "w") as f:
            json.dump(res, f, indent=1)
        write_report(res)
        return
    for model in [m.strip() for m in a.models.split(",") if m.strip()]:
        if pricing and model not in available:
            print(f"SKIP {model}: not in /models")
            res.setdefault("skipped", []).append(model)
            continue
        rem = key_remaining()
        if rem < a.floor:
            print(f"STOP: limit_remaining {rem:.4f} < floor {a.floor}")
            break
        prev = [r for r in res["records"] if r["model"] == model and r.get("tag", "main") == tag]
        first_run = 1 + max([r["run"] for r in prev], default=0)
        jobs = [(t, first_run + i) for i in range(a.runs) for t in tasks]
        print(f"== {model} ({len(jobs)} calls, remaining ${rem:.4f})", flush=True)
        # Probe the response mode with the first job, then run the rest in parallel with it.
        recs = [run_one(model, jobs[0][0], jobs[0][1], "json_schema", schema, base_prompt, validator, pricing, a.repair)]
        mode = recs[0]["mode"]
        with cf.ThreadPoolExecutor(max_workers=a.workers) as ex:
            futs = [ex.submit(run_one, model, t, i, mode, schema, base_prompt, validator, pricing, a.repair) for t, i in jobs[1:]]
            recs += [f.result() for f in futs]
        psha = hashlib.sha1(base_prompt.encode()).hexdigest()[:8]
        for r in recs:
            r["tag"] = tag
            r["prompt_sha"] = psha
            rep = r.get("repair")
            extra = f" -> repair {'V' if rep and rep.get('valid') else 'x'}" if rep else ""
            print(f"  {r['task']:<16} run{r['run']} {r['mode']:<11} {'V' if r.get('valid') else 'x'} sem={r.get('semantic', 0):.2f} "
                  f"{r['latency_s']:>5.1f}s out={r.get('completion_tokens')} ${r.get('cost_usd', 0):.5f}{extra} "
                  f"{(r.get('error') or '; '.join((r.get('schema_errors') or [])[:1] + (r.get('lint_errors') or [])[:1]))[:150]}")
        res["records"] += recs
        res["generated"] = time.strftime("%Y-%m-%d %H:%M:%S")
        res["remaining_end"] = key_remaining()
        with open(res_path, "w") as f:
            json.dump(res, f, indent=1)
    write_report(res)
    print(f"limit_remaining now: {res.get('remaining_end')}")


if __name__ == "__main__":
    main()
