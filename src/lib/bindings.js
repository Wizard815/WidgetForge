// Template bindings: "{{ path | filter(arg) }}" (DESIGN.md §5.3).
// No arbitrary code runs: only paths, literals and the filters below.

const FULL = /^\s*\{\{([^}]*)\}\}\s*$/;
const ANY = /\{\{([^}]*)\}\}/g;

function literal(s) {
  s = s.trim();
  if (/^-?\d+(\.\d+)?$/.test(s)) return { ok: true, v: Number(s) };
  if (/^(['"]).*\1$/.test(s)) return { ok: true, v: s.slice(1, -1) };
  if (s === "true") return { ok: true, v: true };
  if (s === "false") return { ok: true, v: false };
  if (s === "null") return { ok: true, v: null };
  return { ok: false };
}

export function resolvePath(ctx, path) {
  const lit = literal(path);
  if (lit.ok) return lit.v;
  // a[0] indexes by number; a[index] indexes by another value in the context.
  const p = path.trim().replace(/\[([^\]]+)\]/g, (_, k) => {
    k = k.trim();
    return "." + (/^\d+$/.test(k) ? k : String(resolvePath(ctx, k)));
  });
  return getPath(ctx, p);
}

/** Reads a dotted path (no literals, no lookups) from an object. */
export function getPath(obj, path) {
  if (path === undefined || path === null || path === "") return obj;
  let v = obj;
  for (const part of String(path).split(".")) {
    if (v == null) return undefined;
    v = v[part];
  }
  return v;
}

// Collects every value at `path`, walking into arrays on the way.
function collect(v, parts, out) {
  if (v == null) return;
  if (Array.isArray(v)) { for (const x of v) collect(x, parts, out); return; }
  if (parts.length === 0) { out.push(v); return; }
  collect(v[parts[0]], parts.slice(1), out);
}

// Splits on `sep` outside quotes and parentheses.
function split(s, sep) {
  const out = [];
  let cur = "", quote = null, depth = 0;
  for (const ch of s) {
    if (quote) { if (ch === quote) quote = null; cur += ch; continue; }
    if (ch === "'" || ch === '"') { quote = ch; cur += ch; continue; }
    if (ch === "(") depth++;
    if (ch === ")") depth--;
    if (ch === sep && depth === 0) { out.push(cur); cur = ""; continue; }
    cur += ch;
  }
  out.push(cur);
  return out;
}

export function evaluate(expr, ctx) {
  const [head, ...filters] = split(expr, "|");
  let v = resolvePath(ctx, head);
  for (const f of filters) {
    const m = f.trim().match(/^(\w+)(?:\((.*)\))?$/);
    if (!m || !FILTERS[m[1]]) continue;
    // Arguments are literals, or paths read from the context.
    const args = m[2] !== undefined && m[2].trim() !== ""
      ? split(m[2], ",").map((a) => (literal(a).ok ? literal(a).v : evaluate(a, ctx)))
      : [];
    v = FILTERS[m[1]].call(ctx, v, ...args);
  }
  return v;
}

/** Binds one value. A string that is exactly one {{ … }} keeps its type. */
export function bind(value, ctx) {
  if (typeof value !== "string") return value;
  const full = value.match(FULL);
  if (full) return evaluate(full[1], ctx);
  if (!value.includes("{{")) return value;
  return value.replace(ANY, (_, e) => {
    const v = evaluate(e, ctx);
    return v == null ? "" : String(v);
  });
}

/** Binds every string inside an object or array (request bodies, reminder lists). */
export function bindDeep(value, ctx) {
  if (typeof value === "string") return bind(value, ctx);
  if (Array.isArray(value)) return value.map((v) => bindDeep(v, ctx));
  if (value && typeof value === "object") {
    const out = {};
    for (const [k, v] of Object.entries(value)) out[k] = bindDeep(v, ctx);
    return out;
  }
  return value;
}

export function isVisible(block, ctx) {
  if (block.visible === undefined) return true;
  const v = bind(block.visible, ctx);
  if (Array.isArray(v)) return v.length > 0;
  return !!v;
}

// ---------- filters ----------

const num = (v) => (typeof v === "number" ? v : Number(v) || 0);

export function formatBytes(v, digits = 1) {
  const units = ["B", "KB", "MB", "GB", "TB", "PB"];
  let n = num(v), i = 0;
  while (Math.abs(n) >= 1024 && i < units.length - 1) { n /= 1024; i++; }
  return `${n.toFixed(i === 0 ? 0 : digits)} ${units[i]}`;
}

export function formatDuration(seconds) {
  let s = Math.floor(num(seconds));
  const d = Math.floor(s / 86400); s %= 86400;
  const h = Math.floor(s / 3600); s %= 3600;
  const m = Math.floor(s / 60);
  if (d) return `${d}d ${h}h`;
  if (h) return `${h}h ${m}m`;
  return `${m}m`;
}

const pad = (n) => String(n).padStart(2, "0");
const names = (opt, d) => new Intl.DateTimeFormat(undefined, opt).format(d);
const TOKENS = /\[([^\]]*)\]|YYYY|MMMM|MMM|MM|M|dddd|ddd|DD|D|HH|H|hh|h|mm|ss|A/g;

/** Accepts a Date, milliseconds, or a date string ("2026-11-26" is read as a local day). */
export function toDate(v) {
  if (v instanceof Date) return v;
  if (typeof v === "string" && /^\d{4}-\d{2}-\d{2}$/.test(v)) {
    const [y, m, d] = v.split("-").map(Number);
    return new Date(y, m - 1, d);
  }
  return new Date(v ?? Date.now());
}

/**
 * Wall-clock time in another time zone, as a timestamp whose local fields
 * (hours, day, ...) read as that zone's time. Lets every date filter and the
 * analog clock show a world clock without knowing about zones.
 */
export function shiftToZone(ms, tz) {
  if (!tz) return ms;
  const parts = Object.fromEntries(
    new Intl.DateTimeFormat("en-US", {
      timeZone: tz, hourCycle: "h23", year: "numeric", month: "2-digit", day: "2-digit",
      hour: "2-digit", minute: "2-digit", second: "2-digit",
    }).formatToParts(new Date(ms)).map((p) => [p.type, p.value]),
  );
  return new Date(+parts.year, parts.month - 1, +parts.day, +parts.hour % 24, +parts.minute, +parts.second).getTime();
}

/** The next time a date comes round: itself, or next year's (birthdays). */
export function nextOccurrence(target, now = Date.now(), yearly = false) {
  const t = toDate(target);
  if (!yearly || isNaN(t)) return t.getTime();
  const n = new Date(now);
  let d = new Date(n.getFullYear(), t.getMonth(), t.getDate(), t.getHours(), t.getMinutes());
  if (d.getTime() < now - 86400000) d = new Date(n.getFullYear() + 1, t.getMonth(), t.getDate(), t.getHours(), t.getMinutes());
  return d.getTime();
}

export function formatDate(v, fmt = "YYYY-MM-DD", utc = false) {
  let d = toDate(v);
  if (isNaN(d)) return "";
  // For UTC output, shift so the local getters read UTC values.
  if (utc) d = new Date(d.getTime() + d.getTimezoneOffset() * 60000);
  const h12 = d.getHours() % 12 || 12;
  return String(fmt).replace(TOKENS, (tok, lit) => {
    if (lit !== undefined) return lit;
    switch (tok) {
      case "YYYY": return String(d.getFullYear());
      case "MMMM": return names({ month: "long" }, d);
      case "MMM": return names({ month: "short" }, d);
      case "MM": return pad(d.getMonth() + 1);
      case "M": return String(d.getMonth() + 1);
      case "dddd": return names({ weekday: "long" }, d);
      case "ddd": return names({ weekday: "short" }, d);
      case "DD": return pad(d.getDate());
      case "D": return String(d.getDate());
      case "HH": return pad(d.getHours());
      case "H": return String(d.getHours());
      case "hh": return pad(h12);
      case "h": return String(h12);
      case "mm": return pad(d.getMinutes());
      case "ss": return pad(d.getSeconds());
      case "A": return d.getHours() < 12 ? "AM" : "PM";
    }
    return tok;
  });
}

export const FILTERS = {
  fixed: (v, n = 0) => num(v).toFixed(n),
  round: (v) => Math.round(num(v)),
  percent: (v, n = 0) => `${(num(v) * 100).toFixed(n)}%`,
  bytes: (v, n = 1) => formatBytes(v, n),
  rate: (v, n = 1) => `${formatBytes(v, n)}/s`,
  duration: (v) => formatDuration(v),
  /** Seconds as m:ss or h:mm:ss, for media positions. */
  duration_clock(v) {
    const t = Math.max(0, Math.floor(num(v)));
    const h = Math.floor(t / 3600), m = Math.floor((t % 3600) / 60), s = t % 60;
    return h ? `${h}:${pad(m)}:${pad(s)}` : `${m}:${pad(s)}`;
  },
  // null means "no time" (a sunset that does not happen): show nothing.
  date: (v, fmt = "YYYY-MM-DD") => (v === null ? "" : formatDate(v, fmt)),
  time: (v, fmt = "HH:mm") => (v === null ? "" : formatDate(v, fmt)),
  upper: (v) => String(v ?? "").toUpperCase(),
  lower: (v) => String(v ?? "").toLowerCase(),
  default: (v, x) => (v == null || v === "" ? x : v),
  count: (v) => (Array.isArray(v) ? v.length : v && typeof v === "object" ? Object.keys(v).length : 0),
  not: (v) => !v,
  url: (v) => encodeURIComponent(v ?? ""),
  choose: (v, yes = "", no = "") => (v ? yes : no),
  eq: (v, x) => v == x,
  gt: (v, x) => num(v) > num(x),
  gte: (v, x) => num(v) >= num(x),
  lt: (v, x) => num(v) < num(x),
  lte: (v, x) => num(v) <= num(x),
  /** True when the value is any of the listed ones: state | among('on', 'off'). */
  among: (v, ...xs) => xs.some((x) => v == x),
  mul: (v, x = 1) => num(v) * num(x),
  div: (v, x = 1) => (num(x) ? num(v) / num(x) : 0),
  add: (v, x = 0) => num(v) + num(x),
  min: (v, x) => Math.min(num(v), num(x)),
  max: (v, x) => Math.max(num(v), num(x)),
  /** Sum of every number at `path` inside an array (walks nested arrays). */
  sum(v, path = "") {
    const out = [];
    collect(v, path ? String(path).split(".") : [], out);
    return out.reduce((a, x) => a + num(x), 0);
  },
  /** Reads a path given as a value, e.g. a path the user typed into settings. */
  path: (v, p) => (p ? getPath(v, String(p).replace(/\[(\d+)\]/g, ".$1")) : v),
  first: (v) => (Array.isArray(v) ? v[0] : v),
  slice: (v, a = 0, b) => (Array.isArray(v) || typeof v === "string" ? v.slice(a, b) : v),
  utcdate: (v, fmt = "YYYY-MM-DD") => formatDate(v, fmt, true),
  /** Unix seconds for the start of the current UTC day or month. */
  unix(v, period = "now") {
    const d = toDate(v);
    if (period === "day") return Date.UTC(d.getUTCFullYear(), d.getUTCMonth(), d.getUTCDate()) / 1000;
    if (period === "month") return Date.UTC(d.getUTCFullYear(), d.getUTCMonth(), 1) / 1000;
    return Math.floor(d.getTime() / 1000);
  },
  /** Parses a date-time written in UTC without a zone, e.g. "2026-11-29T16:30:00". */
  utcparse(v) {
    if (v == null || v === "") return v;
    const t = Date.parse(/[zZ]|[+-]\d\d:?\d\d$/.test(v) ? v : `${v}Z`);
    return isNaN(t) ? v : t;
  },
  /** Whole days from now until a date (negative once it has passed). */
  days_until(v, yearly = false) {
    const t = nextOccurrence(v, Date.now(), yearly);
    const start = new Date(); start.setHours(0, 0, 0, 0);
    const day = new Date(t); day.setHours(0, 0, 0, 0);
    return Math.round((day - start) / 86400000);
  },
  /** Time left as "12d 4h 3m" (or "3h 2m 10s" when close); "" once passed. */
  countdown(v, yearly = false) {
    let s = Math.floor((nextOccurrence(v, Date.now(), yearly) - Date.now()) / 1000);
    if (isNaN(s) || s < 0) return "";
    const d = Math.floor(s / 86400); s %= 86400;
    const h = Math.floor(s / 3600); s %= 3600;
    const m = Math.floor(s / 60); s %= 60;
    return d ? `${d}d ${h}h ${m}m` : h ? `${h}h ${m}m ${s}s` : `${m}m ${s}s`;
  },
  /** "+5h" / "−3h 30m" difference between a zone and this computer's time. */
  zone_offset(v, tz) {
    if (!tz) return "";
    const now = Date.now();
    let mins = Math.round((shiftToZone(now, tz) - now) / 60000);
    if (Math.abs(mins) < 1) return "same time";
    const sign = mins > 0 ? "+" : "−";
    mins = Math.abs(mins);
    return `${sign}${Math.floor(mins / 60)}h${mins % 60 ? ` ${mins % 60}m` : ""}`;
  },
  /** A moment as the wall clock of a time zone reads it, for `time`/`date`: sunrise | zone('Europe/London') | time. */
  zone(v, tz) {
    if (v == null || v === "" || !tz) return v;
    try {
      return shiftToZone(typeof v === "number" ? v : Date.parse(v), tz);
    } catch {
      return v;
    }
  },
  /** Looks a value up in the manifest's `lookups` table. */
  lookup(v, table) {
    const t = this?.lookups?.[table];
    if (!t) return v;
    return t[String(v)] ?? t.default ?? v;
  },
  json: (v) => JSON.stringify(v),
  /** Sorts an array by a path inside each item; 'desc' for largest first. */
  sort(v, path = "", order = "asc") {
    if (!Array.isArray(v)) return v;
    const key = (x) => getPath(x, path);
    const dir = order === "desc" ? -1 : 1;
    return [...v].sort((a, b) => {
      const x = key(a), y = key(b);
      if (typeof x === "number" && typeof y === "number") return (x - y) * dir;
      return String(x ?? "").localeCompare(String(y ?? "")) * dir;
    });
  },
  /** Every value at `path` inside an array: [{a:1},{a:2}] | pluck('a') → [1, 2]. */
  pluck(v, path = "") {
    const out = [];
    collect(v, path ? String(path).split(".") : [], out);
    return out;
  },
  join: (v, sep = ", ") => (Array.isArray(v) ? v.filter((x) => x != null && x !== "").join(sep) : v),
  split: (v, sep = ",") => (v == null || v === "" ? [] : String(v).split(sep).map((x) => x.trim()).filter(Boolean)),
  /** Non-empty lines of text, trimmed. */
  lines: (v) => (v == null ? [] : String(v).split(/\r?\n/).map((x) => x.trim()).filter(Boolean)),
  /** Items whose `path` is truthy, or equals `value` when given. */
  where(v, path = "", value) {
    if (!Array.isArray(v)) return v;
    return v.filter((x) => (value === undefined ? !!getPath(x, path) : getPath(x, path) == value));
  },
  /** Items whose `path` is NOT `value` (or falsy). */
  except(v, path = "", value) {
    if (!Array.isArray(v)) return v;
    return v.filter((x) => (value === undefined ? !getPath(x, path) : getPath(x, path) != value));
  },
  /** Keeps items whose `path` is one of a comma-separated list, in the list's order. */
  pick(v, list = "", path = "id") {
    if (!Array.isArray(v)) return [];
    const wanted = String(list ?? "").split(/[,\n]/).map((x) => x.trim()).filter(Boolean);
    return wanted.map((w) => v.find((x) => String(getPath(x, path)) === w)).filter(Boolean);
  },
  /** An object as a list: {a: 1} → [{key: "a", value: 1}]. Objects get their key merged in. */
  entries: (v) =>
    v && typeof v === "object" && !Array.isArray(v)
      ? Object.entries(v).map(([key, value]) => (value && typeof value === "object" && !Array.isArray(value) ? { key, ...value, value } : { key, value }))
      : [],
  keys: (v) => (v && typeof v === "object" ? Object.keys(v) : []),
  includes: (v, x) => (Array.isArray(v) || typeof v === "string" ? v.includes(x) : false),
  sub: (v, x = 0) => num(v) - num(x),
  abs: (v) => Math.abs(num(v)),
  /** 1234567.8 → "1,234,568" in the computer's own style. */
  number: (v, digits = 0) =>
    v == null || v === "" || isNaN(Number(v)) ? v : Number(v).toLocaleString(undefined, { maximumFractionDigits: digits, minimumFractionDigits: digits }),
  /** Signed: 2.5 → "+2.5". */
  signed: (v, digits = 1) => (v == null || v === "" ? "" : `${num(v) >= 0 ? "+" : "−"}${Math.abs(num(v)).toFixed(digits)}`),
  /** "5 min ago" / "in 3 h". Accepts ms, seconds (small numbers) or a date string. */
  ago(v) {
    if (v == null || v === "") return "";
    let t = typeof v === "number" ? (v < 1e11 ? v * 1000 : v) : Date.parse(v);
    if (isNaN(t)) return "";
    const s = Math.round((Date.now() - t) / 1000);
    const a = Math.abs(s);
    const text = a < 60 ? "just now" : a < 3600 ? `${Math.floor(a / 60)} min` : a < 86400 ? `${Math.floor(a / 3600)} h` : `${Math.floor(a / 86400)} d`;
    if (text === "just now") return text;
    return s >= 0 ? `${text} ago` : `in ${text}`;
  },
  /** Parses a date string to ms (for `time`, `ago`, `countdown`). */
  parse: (v) => (v == null || v === "" ? v : typeof v === "number" ? v : Date.parse(v)),
  /** Seconds as "2h 05m" for screen time and long durations. */
  hm(v) {
    const s = Math.max(0, Math.round(num(v)));
    const h = Math.floor(s / 3600), m = Math.floor((s % 3600) / 60);
    return h ? `${h}h ${pad(m)}m` : `${m}m`;
  },
  /** Reads a key built from parts: prices | get(currency, '_24h_change'). */
  get: (v, ...parts) => (v && typeof v === "object" ? v[parts.join("")] : undefined),
  /** 'good' for a rise, 'bad' for a fall, 'muted' for none (colours). */
  trend: (v) => (v == null || v === "" || isNaN(Number(v)) || Number(v) === 0 ? "muted" : Number(v) > 0 ? "good" : "bad"),
  /** First character, upper-cased (letter tiles). */
  initial: (v) => String(v ?? "").trim().charAt(0).toUpperCase(),
  /** Replaces matches of a pattern (a regular expression) in text. */
  replace(v, pattern, with_ = "") {
    if (v == null) return v;
    try {
      return String(v).replace(new RegExp(String(pattern), "g"), String(with_));
    } catch {
      return v;
    }
  },
};

/** Manifest defaults overlaid with the instance's own values. */
export function effectiveSettings(manifest, instance) {
  const out = {};
  for (const s of manifest?.settings ?? []) out[s.key] = s.default ?? null;
  return { ...out, ...(instance?.settings ?? {}) };
}
