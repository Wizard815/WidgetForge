// Bindings: `{{ path | filter(arg) | filter }}`. No arbitrary code is evaluated.

export type Scope = Record<string, unknown>;

const TEMPLATE = /\{\{([\s\S]*?)\}\}/g;
const SINGLE = /^\s*\{\{([\s\S]*?)\}\}\s*$/;

/** Reads `a.b[0].c` from a scope. Unknown paths give `undefined`. */
export function lookup(scope: Scope, path: string): unknown {
  const parts = path.trim().replace(/\[(\d+)\]/g, ".$1").split(".").filter(Boolean);
  let cur: unknown = scope;
  for (const p of parts) {
    if (cur == null || typeof cur !== "object") return undefined;
    cur = (cur as Record<string, unknown>)[p];
  }
  return cur;
}

function parseArgs(src: string): unknown[] {
  const out: unknown[] = [];
  const re = /\s*("([^"]*)"|'([^']*)'|[^,]+)\s*(,|$)/g;
  let m: RegExpExecArray | null;
  while ((m = re.exec(src)) && m[0] !== "") {
    if (m[2] !== undefined) out.push(m[2]);
    else if (m[3] !== undefined) out.push(m[3]);
    else {
      const raw = m[1].trim();
      const n = Number(raw);
      out.push(raw !== "" && !Number.isNaN(n) ? n : raw);
    }
  }
  return out;
}

const UNITS = ["B", "KB", "MB", "GB", "TB", "PB"];

export function formatBytes(n: number, digits = 1): string {
  if (!Number.isFinite(n)) return "–";
  let i = 0;
  let v = Math.abs(n);
  while (v >= 1024 && i < UNITS.length - 1) {
    v /= 1024;
    i++;
  }
  return `${(Math.sign(n) * v).toFixed(i === 0 ? 0 : digits)} ${UNITS[i]}`;
}

export function formatDuration(seconds: number): string {
  if (!Number.isFinite(seconds)) return "–";
  const s = Math.floor(seconds);
  const d = Math.floor(s / 86400);
  const h = Math.floor((s % 86400) / 3600);
  const m = Math.floor((s % 3600) / 60);
  if (d > 0) return `${d}d ${h}h`;
  if (h > 0) return `${h}h ${m}m`;
  return `${m}m`;
}

const pad = (n: number, w = 2) => String(n).padStart(w, "0");

/**
 * Date formatting with tokens: YYYY MMMM MMM MM M DD D dddd ddd HH H hh h mm ss A.
 * Text in [brackets] is kept literally.
 */
export function formatDate(value: unknown, fmt: string, locale?: string): string {
  const d = value instanceof Date ? value : new Date(typeof value === "number" ? value : String(value ?? Date.now()));
  if (Number.isNaN(d.getTime())) return "";
  const lang = locale || undefined;
  const h12 = d.getHours() % 12 || 12;
  const tokens: Record<string, () => string> = {
    YYYY: () => String(d.getFullYear()),
    MMMM: () => d.toLocaleString(lang, { month: "long" }),
    MMM: () => d.toLocaleString(lang, { month: "short" }),
    MM: () => pad(d.getMonth() + 1),
    M: () => String(d.getMonth() + 1),
    DD: () => pad(d.getDate()),
    D: () => String(d.getDate()),
    dddd: () => d.toLocaleString(lang, { weekday: "long" }),
    ddd: () => d.toLocaleString(lang, { weekday: "short" }),
    HH: () => pad(d.getHours()),
    H: () => String(d.getHours()),
    hh: () => pad(h12),
    h: () => String(h12),
    mm: () => pad(d.getMinutes()),
    ss: () => pad(d.getSeconds()),
    A: () => (d.getHours() < 12 ? "AM" : "PM"),
  };
  return fmt.replace(/\[([^\]]*)\]|YYYY|MMMM|MMM|MM|M|DD|D|dddd|ddd|HH|H|hh|h|mm|ss|A/g, (tok, lit) =>
    lit !== undefined ? lit : tokens[tok](),
  );
}

type Filter = (v: unknown, ...args: unknown[]) => unknown;

const num = (v: unknown) => (typeof v === "number" ? v : Number(v));

export const FILTERS: Record<string, Filter> = {
  fixed: (v, n = 1) => (Number.isFinite(num(v)) ? num(v).toFixed(Number(n)) : "–"),
  round: (v) => (Number.isFinite(num(v)) ? Math.round(num(v)) : v),
  percent: (v, n = 0) => (Number.isFinite(num(v)) ? `${num(v).toFixed(Number(n))}%` : "–"),
  bytes: (v, n = 1) => formatBytes(num(v), Number(n)),
  rate: (v, n = 1) => `${formatBytes(num(v), Number(n))}/s`,
  duration: (v) => formatDuration(num(v)),
  date: (v, fmt = "ddd, MMM D") => formatDate(v, String(fmt)),
  time: (v, fmt = "HH:mm") => formatDate(v, String(fmt)),
  upper: (v) => String(v ?? "").toUpperCase(),
  lower: (v) => String(v ?? "").toLowerCase(),
  default: (v, d = "") => (v === undefined || v === null || v === "" ? d : v),
  count: (v) => (Array.isArray(v) ? v.length : 0),
  not: (v) => !truthy(v),
};

/** Splits on `|` that are not inside quotes or parentheses. */
function splitPipes(expr: string): string[] {
  const out: string[] = [];
  let depth = 0;
  let quote = "";
  let cur = "";
  for (const ch of expr) {
    if (quote) {
      if (ch === quote) quote = "";
    } else if (ch === '"' || ch === "'") quote = ch;
    else if (ch === "(") depth++;
    else if (ch === ")") depth--;
    else if (ch === "|" && depth === 0) {
      out.push(cur);
      cur = "";
      continue;
    }
    cur += ch;
  }
  out.push(cur);
  return out;
}

export function evalExpr(expr: string, scope: Scope): unknown {
  const [head, ...filters] = splitPipes(expr);
  const h = head.trim();
  let value: unknown;
  if (/^-?\d+(\.\d+)?$/.test(h)) value = Number(h);
  else if (/^(["']).*\1$/.test(h)) value = h.slice(1, -1);
  else if (h === "true" || h === "false") value = h === "true";
  else value = lookup(scope, h);
  for (const f of filters) {
    const m = /^\s*([a-z_]+)\s*(?:\(([\s\S]*)\))?\s*$/i.exec(f);
    if (!m) continue;
    const fn = FILTERS[m[1]];
    if (!fn) continue;
    value = fn(value, ...(m[2] !== undefined ? parseArgs(m[2]) : []));
  }
  return value;
}

/**
 * Renders a template. A string that is exactly one `{{ … }}` returns the raw value
 * (so `value: "{{ sys.cpu.usage }}"` stays a number); anything else becomes a string.
 * Non-strings are returned unchanged.
 */
export function render(tpl: unknown, scope: Scope): unknown {
  if (typeof tpl !== "string") return tpl;
  const single = SINGLE.exec(tpl);
  if (single && !single[1].includes("}}")) return evalExpr(single[1], scope);
  if (!tpl.includes("{{")) return tpl;
  return tpl.replace(TEMPLATE, (_, e: string) => {
    const v = evalExpr(e, scope);
    return v === undefined || v === null ? "" : String(v);
  });
}

export function renderText(tpl: unknown, scope: Scope): string {
  const v = render(tpl, scope);
  return v === undefined || v === null ? "" : String(v);
}

export function truthy(v: unknown): boolean {
  if (typeof v === "string") return v !== "" && v !== "false" && v !== "0";
  return Boolean(v);
}
