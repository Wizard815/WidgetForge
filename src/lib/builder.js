// The visual widget builder's logic, kept free of UI so it can be tested.
// A "draft" is what the builder edits; buildManifest turns it into a normal
// widget.yaml manifest (the same format hand-written widgets use).

// PLACEHOLDER wording: block names, hints and field labels.

const SIZES = ["xs", "sm", "md", "lg", "xl", "xxl"];
const COLORS = ["inherit", "muted", "accent", "good", "bad", "warn"];
const ALIGN = ["left", "center", "right"];

// Each field: key, label, kind (bind = text that may hold {{ values }}, text,
// number, bool, choice, color), and for choice its options.
export const BLOCK_TYPES = {
  text: {
    name: "Text", group: "Show", hint: "Words, or a value from your data.",
    make: () => ({ type: "text", text: "Hello" }),
    fields: [
      { key: "text", label: "Text", kind: "bind" },
      { key: "size", label: "Size", kind: "choice", options: SIZES, default: "md" },
      { key: "weight", label: "Weight", kind: "choice", options: ["normal", "bold"], default: "normal" },
      { key: "color", label: "Colour", kind: "color", default: "inherit" },
      { key: "align", label: "Align", kind: "choice", options: ALIGN, default: "left" },
      { key: "mono", label: "Fixed-width font", kind: "bool" },
      { key: "clip", label: "One line (cut off)", kind: "bool" },
    ],
  },
  stat: {
    name: "Big number", group: "Show", hint: "A label with a large value under it.",
    make: () => ({ type: "stat", label: "Label", value: "42" }),
    fields: [
      { key: "label", label: "Label", kind: "bind" },
      { key: "value", label: "Value", kind: "bind" },
      { key: "note", label: "Small note", kind: "bind" },
      { key: "size", label: "Size", kind: "choice", options: SIZES, default: "xl" },
      { key: "color", label: "Colour", kind: "color", default: "inherit" },
      { key: "align", label: "Align", kind: "choice", options: ALIGN, default: "left" },
    ],
  },
  gauge: {
    name: "Ring gauge", group: "Show", hint: "A ring that fills up, like CPU use.",
    make: () => ({ type: "gauge", value: "50", max: 100, label: "Gauge" }),
    fields: [
      { key: "value", label: "Value", kind: "bind" },
      { key: "max", label: "Maximum", kind: "bind", default: "100" },
      { key: "label", label: "Label", kind: "bind" },
      { key: "format", label: "Text inside (empty = percent)", kind: "bind" },
      { key: "color", label: "Colour", kind: "color", default: "accent" },
    ],
  },
  bar: {
    name: "Progress bar", group: "Show", hint: "A bar that fills up.",
    make: () => ({ type: "bar", value: "50", max: 100, label: "Progress" }),
    fields: [
      { key: "value", label: "Value", kind: "bind" },
      { key: "max", label: "Maximum", kind: "bind", default: "100" },
      { key: "label", label: "Label", kind: "bind" },
      { key: "format", label: "Text on the right (empty = percent)", kind: "bind" },
      { key: "color", label: "Colour", kind: "color", default: "accent" },
    ],
  },
  graph: {
    name: "Live graph", group: "Show", hint: "A line of the last minute of a value.",
    make: () => ({ type: "graph", value: "0", max: 100, label: "Graph" }),
    fields: [
      { key: "value", label: "Value", kind: "bind" },
      { key: "max", label: "Maximum (or auto)", kind: "bind", default: "100" },
      { key: "label", label: "Label", kind: "bind" },
      { key: "format", label: "Text on the right", kind: "bind" },
      { key: "color", label: "Colour", kind: "color", default: "accent" },
    ],
  },
  table: {
    name: "Table", group: "Show", hint: "One row per item of a list in your data.",
    make: () => ({ type: "table", items: "", header: true, columns: [{ label: "Name", value: "{{ row.name }}" }] }),
    fields: [
      { key: "items", label: "List (pick a list from your data)", kind: "bind" },
      { key: "header", label: "Show column names", kind: "bool", default: true },
      { key: "limit", label: "Rows at most", kind: "number", default: 50 },
      { key: "empty", label: "Text when empty", kind: "text" },
    ],
    columns: true,
  },
  clock: {
    name: "Analog clock", group: "Show", hint: "A clock face.",
    make: () => ({ type: "clock", seconds: true }),
    fields: [
      { key: "seconds", label: "Second hand", kind: "bool", default: true },
      { key: "timezone", label: "Time zone (empty = this computer)", kind: "text" },
    ],
  },
  image: {
    name: "Picture", group: "Show", hint: "A picture from a web address.",
    make: () => ({ type: "image", src: "", fit: "contain" }),
    fields: [
      { key: "src", label: "Picture address (https://…)", kind: "bind" },
      { key: "fit", label: "Fit", kind: "choice", options: ["contain", "cover"], default: "contain" },
    ],
  },
  button: {
    name: "Button", group: "Do", hint: "Opens a web page or copies text.",
    make: () => ({ type: "button", label: "Open", open: "https://" }),
    fields: [
      { key: "label", label: "Label", kind: "bind" },
      { key: "icon", label: "Symbol (an emoji)", kind: "text" },
      { key: "open", label: "Opens this web address", kind: "bind" },
      { key: "copy", label: "…or copies this text", kind: "bind" },
      { key: "style", label: "Shape", kind: "choice", options: ["", "tile"], default: "" },
    ],
  },
  notes: {
    name: "Notes", group: "Do", hint: "A text box that is saved.",
    make: () => ({ type: "notes", key: "notes", placeholder: "Write something…" }),
    fields: [{ key: "placeholder", label: "Hint text", kind: "text" }],
    storage: true,
  },
  checklist: {
    name: "Checklist", group: "Do", hint: "A to-do list that is saved.",
    make: () => ({ type: "checklist", key: "items", placeholder: "Add an item" }),
    fields: [{ key: "placeholder", label: "Hint text", kind: "text" }],
    storage: true,
  },
  timer: {
    name: "Timer", group: "Do", hint: "Countdown, stopwatch or focus timer.",
    make: () => ({ type: "timer", mode: "countdown", minutes: 5 }),
    fields: [
      { key: "mode", label: "Kind", kind: "choice", options: ["countdown", "stopwatch", "pomodoro"], default: "countdown" },
      { key: "minutes", label: "Minutes", kind: "number", default: 5 },
    ],
    storage: true,
  },
  row: {
    name: "Side by side", group: "Layout", hint: "Puts the blocks inside it next to each other.",
    make: () => ({ type: "row", children: [] }),
    fields: [{ key: "gap", label: "Space between", kind: "number", default: 8 }],
    container: true,
  },
  column: {
    name: "Stacked", group: "Layout", hint: "Puts the blocks inside it under each other.",
    make: () => ({ type: "column", children: [] }),
    fields: [{ key: "gap", label: "Space between", kind: "number", default: 8 }],
    container: true,
  },
  grid: {
    name: "Grid", group: "Layout", hint: "Puts the blocks inside it in columns.",
    make: () => ({ type: "grid", columns: 2, children: [] }),
    fields: [
      { key: "columns", label: "Columns", kind: "number", default: 2 },
      { key: "gap", label: "Space between", kind: "number", default: 8 },
    ],
    container: true,
  },
  divider: { name: "Line", group: "Layout", hint: "A thin line.", make: () => ({ type: "divider" }), fields: [] },
  spacer: {
    name: "Space", group: "Layout", hint: "Empty space that pushes things apart.",
    make: () => ({ type: "spacer" }),
    fields: [{ key: "size", label: "At least (px)", kind: "number", default: 0 }],
  },
};

/** Fields every block has. */
export const COMMON_FIELDS = [{ key: "visible", label: "Show only when (empty = always)", kind: "bind" }];

export const COLOR_NAMES = COLORS;

// ---------- data sources ----------

export const SYSTEM_CATEGORIES = [
  ["cpu", "Processor use"], ["memory", "Memory"], ["disk", "Main disk"], ["network", "Network speed"],
  ["os", "System name"], ["processes", "Busiest programs"], ["gpu", "Graphics card"],
  ["temperature", "Temperatures"], ["battery", "Battery"], ["disks", "All drives"],
  ["sensors", "Fans and power"], ["interfaces", "Network adapters"],
];

// PLACEHOLDER wording.
export const SOURCE_KINDS = {
  clock: { name: "Time and date", hint: "The current time, for clocks and dates." },
  system: { name: "This computer", hint: "Processor, memory, disks, network, battery…" },
  net: { name: "Web address (JSON)", hint: "Any web service or homelab app that answers with JSON, RSS or a calendar." },
  command: { name: "Command output", hint: "What a command prints. You type the command when you add the widget." },
};

export function newSource(kind, taken = []) {
  const base = { clock: "time", system: "pc", net: "web", command: "cmd" }[kind] ?? "data";
  let name = base, n = 2;
  while (taken.includes(name)) name = `${base}${n++}`;
  const every = { clock: 1, system: 2, net: 300, command: 30 }[kind] ?? 60;
  const s = { name, kind, every };
  if (kind === "system") s.categories = ["cpu", "memory"];
  if (kind === "net") Object.assign(s, { url: "", format: "json", use_key: false, key_header: "Authorization", key_prefix: "Bearer " });
  if (kind === "command") s.command = "";
  return s;
}

// ---------- tree editing: a block is found by its path of indexes ----------

export function getAt(blocks, path) {
  let list = blocks, b = null;
  for (const i of path) {
    b = list?.[i];
    list = b?.children;
  }
  return b;
}

function listAt(blocks, parentPath) {
  return parentPath.length === 0 ? blocks : getAt(blocks, parentPath)?.children;
}

const clone = (v) => JSON.parse(JSON.stringify(v));

/** Adds a block after the selected one (or inside it, if it is a container). Returns [blocks, newPath]. */
export function insertBlock(blocks, selected, block) {
  const out = clone(blocks);
  const sel = selected ? getAt(out, selected) : null;
  if (sel && BLOCK_TYPES[sel.type]?.container) {
    sel.children = sel.children ?? [];
    sel.children.push(block);
    return [out, [...selected, sel.children.length - 1]];
  }
  if (!selected || selected.length === 0) {
    out.push(block);
    return [out, [out.length - 1]];
  }
  const parent = selected.slice(0, -1);
  const list = listAt(out, parent);
  const at = selected[selected.length - 1] + 1;
  list.splice(at, 0, block);
  return [out, [...parent, at]];
}

export function removeBlock(blocks, path) {
  const out = clone(blocks);
  const list = listAt(out, path.slice(0, -1));
  list.splice(path[path.length - 1], 1);
  return out;
}

/** Moves a block up (-1) or down (+1) among its neighbours. Returns [blocks, newPath]. */
export function moveBlock(blocks, path, dir) {
  const out = clone(blocks);
  const list = listAt(out, path.slice(0, -1));
  const i = path[path.length - 1], j = i + dir;
  if (j < 0 || j >= list.length) return [out, path];
  [list[i], list[j]] = [list[j], list[i]];
  return [out, [...path.slice(0, -1), j]];
}

export function updateBlock(blocks, path, patch) {
  const out = clone(blocks);
  const b = getAt(out, path);
  for (const [k, v] of Object.entries(patch)) {
    if (v === undefined || v === "" || v === null) delete b[k];
    else b[k] = v;
  }
  return out;
}

/** Every block, depth first, with its path and depth (for the outline). */
export function outline(blocks, path = [], depth = 0, out = []) {
  (blocks ?? []).forEach((b, i) => {
    out.push({ block: b, path: [...path, i], depth });
    if (b.children) outline(b.children, [...path, i], depth + 1, out);
  });
  return out;
}

// ---------- draft ⇄ manifest ----------

export function slug(text) {
  return String(text ?? "").toLowerCase().normalize("NFKD").replace(/[^\w\s-]/g, "").trim().replace(/[\s_]+/g, "-").replace(/-+/g, "-").slice(0, 30) || "widget";
}

export function newDraft() {
  return {
    id: "my.new-widget",
    name: "My widget",
    description: "",
    category: "custom",
    accent: "#7cc4ff",
    width: 260,
    height: 160,
    multi: true,
    sources: [newSource("system")],
    blocks: [
      { type: "text", text: "My widget", weight: "bold" },
      { type: "stat", label: "Processor", value: "{{ pc.cpu.usage | fixed(0) }}%" },
    ],
    extra: {},
  };
}

function hostOf(url) {
  try {
    const u = new URL(String(url).trim());
    if (!/^https?:$/.test(u.protocol)) return null;
    return u.port ? `${u.hostname}:${u.port}` : u.hostname;
  } catch {
    return null;
  }
}

function walk(blocks, fn) {
  for (const b of blocks ?? []) {
    fn(b);
    walk(b.children, fn);
  }
}

/** The manifest a draft makes. Permissions are worked out from what it uses. */
export function buildManifest(d) {
  const data = {};
  const permissions = {};
  const settings = [];
  const system = new Set();
  const network = new Set();
  for (const s of d.sources ?? []) {
    const every = Math.max(1, Number(s.every) || 1) * 1000;
    if (s.kind === "clock") data[s.name] = { source: "clock", interval: every };
    else if (s.kind === "system") {
      data[s.name] = { source: "system", interval: every };
      for (const c of s.categories ?? []) system.add(c);
    } else if (s.kind === "net") {
      const src = { source: "net", url: s.url ?? "", interval: every };
      if (s.format && s.format !== "json") src.format = s.format;
      if (s.use_key) {
        const key = `${s.name}_key`;
        src.headers = { [s.key_header || "Authorization"]: `${s.key_prefix ?? ""}[[secret:${key}]]` };
        settings.push({ key, type: "secret", label: `Key for ${hostOf(s.url) ?? s.name}` });
      }
      data[s.name] = src;
      const h = hostOf(s.url);
      if (h) network.add(h);
    } else if (s.kind === "command") {
      // The command itself is never in the widget file, only in the user's own setting.
      const key = s.name === "cmd" ? "command" : `${s.name}_command`;
      data[s.name] = { source: "command", setting: key, interval: every };
      settings.push({ key, type: "command", label: "Command" });
      permissions.run = [...(permissions.run ?? []), `setting:${key}`];
    } else if (s.raw) data[s.name] = s.raw;
  }
  if (system.size) permissions.system = [...system];
  if (network.size) permissions.network = [...network];
  let storage = false;
  walk(d.blocks, (b) => {
    if (BLOCK_TYPES[b.type]?.storage) storage = true;
  });
  if (storage) permissions.storage = true;
  // Permissions from the original widget that the builder does not manage are kept.
  for (const [k, v] of Object.entries(d.extra?.permissions ?? {})) {
    if (!(k in permissions)) permissions[k] = v;
  }

  const m = {
    schema: 1,
    id: d.id,
    name: d.name || "My widget",
    version: d.extra?.version ?? "1.0.0",
    author: d.extra?.author ?? "Me",
    description: d.description ?? "",
    category: d.category || "custom",
    window: { size: [Number(d.width) || 260, Number(d.height) || 160], layer: "desktop", multi_instance: !!d.multi },
    style: { theme: "glass", accent: d.accent || "#7cc4ff" },
  };
  if (Object.keys(permissions).length) m.permissions = permissions;
  const keptSettings = (d.extra?.settings ?? []).filter((s) => !settings.some((x) => x.key === s.key));
  if (settings.length || keptSettings.length) m.settings = [...keptSettings, ...settings];
  if (Object.keys(data).length) m.data = data;
  if (d.extra?.lookups) m.lookups = d.extra.lookups;
  m.ui = { blocks: clone(d.blocks ?? []) };
  return m;
}

/** A draft from an existing manifest (an add-on to edit, or a built-in to copy). */
export function draftFromManifest(m, copy = false) {
  const sources = [];
  for (const [name, src] of Object.entries(m.data ?? {})) {
    const every = Math.round((Number(src.interval) || 1000) / 1000) || 1;
    const plainInterval = typeof src.interval !== "string";
    if (src.source === "clock" && !src.timezone && plainInterval) sources.push({ name, kind: "clock", every });
    else if (src.source === "system" && plainInterval) sources.push({ name, kind: "system", every, categories: [...(m.permissions?.system ?? [])] });
    else if (src.source === "net" && plainInterval && !src.when && !src.for_each && !src.manual && !src.body && typeof src.url === "string" && !src.url.includes("{{")) {
      const header = Object.entries(src.headers ?? {}).find(([, v]) => /\[\[secret:/.test(String(v)));
      sources.push({
        name, kind: "net", every, url: src.url, format: src.format ?? "json",
        use_key: !!header, key_header: header?.[0] ?? "Authorization",
        key_prefix: header ? String(header[1]).replace(/\[\[secret:[^\]]*\]\]/, "") : "Bearer ",
      });
    } else sources.push({ name, kind: "raw", raw: src });
  }
  const managed = new Set();
  for (const s of sources) {
    if (s.kind === "net" && s.use_key) managed.add(`${s.name}_key`);
  }
  const id = copy ? `my.${String(m.id).split(".").pop()}` : m.id;
  return {
    id,
    name: copy ? `${m.name} (copy)` : m.name,
    description: m.description ?? "",
    category: m.category ?? "custom",
    accent: m.style?.accent ?? "#7cc4ff",
    width: m.window?.size?.[0] ?? 260,
    height: m.window?.size?.[1] ?? 160,
    multi: m.window?.multi_instance ?? true,
    sources,
    blocks: clone(m.ui?.blocks ?? []),
    extra: {
      version: m.version,
      author: copy ? "Me" : m.author,
      lookups: m.lookups,
      settings: (m.settings ?? []).filter((s) => !managed.has(s.key)),
      permissions: Object.fromEntries(Object.entries(m.permissions ?? {}).filter(([k]) => !["system", "network"].includes(k))),
    },
  };
}

/** Problems that stop saving, in plain words. */
export function problems(d) {
  const out = [];
  const parts = String(d.id ?? "").split(".");
  if (parts.length < 2 || parts.some((p) => !/^[a-z0-9-]+$/.test(p))) out.push("The id needs lowercase words joined by dots, like my.cpu-card.");
  if (String(d.id).startsWith("forge.")) out.push("Ids starting with forge. belong to the built-in widgets; pick another.");
  if (!String(d.name ?? "").trim()) out.push("Give the widget a name.");
  const names = (d.sources ?? []).map((s) => s.name);
  for (const s of d.sources ?? []) {
    if (!/^[a-z][a-z0-9_]*$/.test(s.name)) out.push(`Data name “${s.name}” must be lowercase letters, numbers or _.`);
    if (names.filter((n) => n === s.name).length > 1) out.push(`Two data sources are called “${s.name}”.`);
    if (["settings", "input", "row", "item", "index"].includes(s.name)) out.push(`“${s.name}” is a reserved name; choose another.`);
    if (s.kind === "net" && !hostOf(s.url)) out.push(`Data “${s.name}”: type a web address starting with https:// or http://.`);
    if (s.kind === "system" && !(s.categories ?? []).length) out.push(`Data “${s.name}”: tick at least one thing to read.`);
  }
  if (!(d.blocks ?? []).length) out.push("Add at least one block.");
  return [...new Set(out)];
}

// ---------- value picker ----------

/**
 * Paths to every value in sample data, for the "insert a value" list.
 * Lists show as one entry (for tables) plus their first item's fields.
 */
export function valuePaths(data, prefix = "", depth = 0, out = []) {
  if (depth > 6 || out.length > 400) return out;
  if (Array.isArray(data)) {
    out.push({ path: prefix, kind: "list", sample: `${data.length} items` });
    if (data.length && typeof data[0] === "object" && data[0] !== null) valuePaths(data[0], `${prefix}[0]`, depth + 1, out);
    return out;
  }
  if (data && typeof data === "object") {
    for (const [k, v] of Object.entries(data)) {
      if (!/^[A-Za-z_$][\w$-]*$/.test(k)) continue;
      valuePaths(v, prefix ? `${prefix}.${k}` : k, depth + 1, out);
    }
    return out;
  }
  if (prefix) out.push({ path: prefix, kind: typeof data, sample: data == null ? "–" : String(data).slice(0, 40) });
  return out;
}

/** Fields of one item of a list, for table columns ({{ row.x }}). */
export function rowPaths(list) {
  const first = Array.isArray(list) ? list.find((x) => x && typeof x === "object") : null;
  return first ? valuePaths(first, "row") : [];
}

// PLACEHOLDER wording: filter descriptions.
export const FILTER_CHOICES = [
  { id: "", name: "As it is" },
  { id: "fixed(0)", name: "Whole number (42)" },
  { id: "fixed(1)", name: "One decimal (42.5)" },
  { id: "number", name: "With separators (1,234)" },
  { id: "percent", name: "Fraction as % (0.42 → 42%)" },
  { id: "bytes", name: "Bytes (1.2 GB)" },
  { id: "rate", name: "Speed (1.2 MB/s)" },
  { id: "duration", name: "Seconds as time (2h 5m)" },
  { id: "time", name: "Time of day (14:05)" },
  { id: "date('MMM D')", name: "Date (Nov 26)" },
  { id: "ago", name: "How long ago (5 min ago)" },
  { id: "upper", name: "UPPER CASE" },
  { id: "count", name: "How many items" },
];

/** "{{ path | filter }}" */
export function binding(path, filter = "") {
  return `{{ ${path}${filter ? ` | ${filter}` : ""} }}`;
}
