// Renders declarative `ui.blocks` into DOM. Each block returns an element and an
// update function that is called whenever data or settings change.

import { render, renderText, truthy, type Scope } from "../lib/template";
import type { Block } from "../lib/types";

export interface Host {
  /** Per-instance storage, already loaded. */
  storage: Record<string, unknown>;
  saveStorage(key: string, value: unknown): void;
  openUrl(url: string): void;
  /** Resolves a relative asset path to a URL inside the widget folder. */
  asset(path: string): string;
}

interface Rendered {
  el: HTMLElement;
  update(scope: Scope): void;
}

const SVG = "http://www.w3.org/2000/svg";

function h<K extends keyof HTMLElementTagNameMap>(tag: K, cls?: string, text?: string): HTMLElementTagNameMap[K] {
  const el = document.createElement(tag);
  if (cls) el.className = cls;
  if (text !== undefined) el.textContent = text;
  return el;
}

function svg<K extends keyof SVGElementTagNameMap>(tag: K, attrs: Record<string, string | number>): SVGElementTagNameMap[K] {
  const el = document.createElementNS(SVG, tag);
  for (const [k, v] of Object.entries(attrs)) el.setAttribute(k, String(v));
  return el;
}

const NAMED_COLORS: Record<string, string> = {
  accent: "var(--wf-accent)",
  muted: "var(--wf-muted)",
  text: "var(--wf-text)",
};

function color(v: unknown): string | null {
  if (typeof v !== "string" || !v) return null;
  if (NAMED_COLORS[v]) return NAMED_COLORS[v];
  if (/^#[0-9a-f]{3,8}$/i.test(v) || /^(rgb|hsl)a?\([\d\s.,%]+\)$/i.test(v)) return v;
  return null;
}

function num(v: unknown, fallback = 0): number {
  const n = typeof v === "number" ? v : Number(v);
  return Number.isFinite(n) ? n : fallback;
}

function setText(el: HTMLElement, text: string) {
  if (el.textContent !== text) el.textContent = text;
}

/** Applies a block's `color` (may be a template) to a CSS variable on an element. */
function applyColor(el: HTMLElement, b: Block, scope: Scope, prop = "--c") {
  const c = color(render(b.color, scope));
  if (c) el.style.setProperty(prop, c);
  else el.style.removeProperty(prop);
}

const builders: Record<string, (b: Block, host: Host) => Rendered> = {
  column: (b, host) => container("wf-column", b, host),
  row: (b, host) => container("wf-row", b, host),
  grid: (b, host) => {
    const r = container("wf-grid", b, host);
    r.el.style.gridTemplateColumns = `repeat(${Math.max(1, num(b.columns, 2))}, minmax(0, 1fr))`;
    return r;
  },

  spacer: (b) => {
    const el = h("div", "wf-spacer");
    if (b.size !== undefined) el.style.flex = `0 0 ${num(b.size, 8)}px`;
    return { el, update() {} };
  },

  text: (b) => {
    const el = h("div", `wf-text size-${String(b.size ?? "md")}`);
    if (b.weight === "bold") el.classList.add("bold");
    if (b.mono) el.classList.add("mono");
    el.style.textAlign = String(b.align ?? "left");
    return {
      el,
      update(scope) {
        setText(el, renderText(b.text, scope));
        applyColor(el, b, scope, "color");
      },
    };
  },

  gauge: (b) => {
    const el = h("div", "wf-gauge");
    const R = 40;
    const C = 2 * Math.PI * R;
    const s = svg("svg", { viewBox: "0 0 100 100" });
    s.append(svg("circle", { cx: 50, cy: 50, r: R, class: "track" }));
    const arc = svg("circle", { cx: 50, cy: 50, r: R, class: "arc", "stroke-dasharray": `${C} ${C}`, transform: "rotate(-90 50 50)" });
    s.append(arc);
    const value = h("div", "wf-gauge-value");
    const label = h("div", "wf-gauge-label");
    const ring = h("div", "wf-gauge-ring");
    ring.append(s, value);
    el.append(ring, label);
    return {
      el,
      update(scope) {
        const v = num(render(b.value, scope));
        const max = num(render(b.max ?? 100, scope), 100) || 100;
        const f = Math.max(0, Math.min(1, v / max));
        arc.setAttribute("stroke-dashoffset", String(C * (1 - f)));
        setText(value, b.format !== undefined ? renderText(b.format, scope) : String(Math.round(v)));
        setText(label, renderText(b.label ?? "", scope));
        applyColor(el, b, scope);
      },
    };
  },

  bar: (b) => {
    const el = h("div", "wf-bar");
    const top = h("div", "wf-bar-top");
    const label = h("span", "wf-bar-label");
    const value = h("span", "wf-bar-value");
    top.append(label, value);
    const track = h("div", "wf-bar-track");
    const fill = h("div", "wf-bar-fill");
    track.append(fill);
    el.append(top, track);
    return {
      el,
      update(scope) {
        const v = num(render(b.value, scope));
        const max = num(render(b.max ?? 100, scope), 100) || 100;
        fill.style.width = `${Math.max(0, Math.min(100, (v / max) * 100))}%`;
        setText(label, renderText(b.label ?? "", scope));
        setText(value, b.format !== undefined ? renderText(b.format, scope) : String(Math.round(v)));
        applyColor(el, b, scope);
      },
    };
  },

  graph: (b) => {
    const el = h("div", "wf-graph");
    const top = h("div", "wf-bar-top");
    const label = h("span", "wf-bar-label");
    const value = h("span", "wf-bar-value");
    top.append(label, value);
    const s = svg("svg", { viewBox: "0 0 100 30", preserveAspectRatio: "none" });
    const area = svg("path", { class: "area" });
    const line = svg("path", { class: "line" });
    s.append(area, line);
    el.append(top, s);
    const points = Math.max(5, Math.min(600, num(b.points, 60)));
    const history: number[] = [];
    let lastData: unknown = undefined;
    return {
      el,
      update(scope) {
        const raw = render(b.value, scope);
        // Only record a new point when the data actually ticked, not on settings changes.
        const tick = scope.__tick;
        if (tick !== lastData) {
          lastData = tick;
          history.push(num(raw));
          if (history.length > points) history.shift();
        }
        const max = b.max !== undefined ? num(render(b.max, scope), 100) || 100 : Math.max(1, ...history);
        const step = 100 / (points - 1);
        const start = points - history.length;
        const pts = history.map((v, i) => `${((start + i) * step).toFixed(2)},${(30 - Math.max(0, Math.min(1, v / max)) * 28 - 1).toFixed(2)}`);
        if (pts.length) {
          line.setAttribute("d", `M${pts.join(" L")}`);
          area.setAttribute("d", `M${(start * step).toFixed(2)},30 L${pts.join(" L")} L100,30 Z`);
        }
        setText(label, renderText(b.label ?? "", scope));
        setText(value, b.format !== undefined ? renderText(b.format, scope) : String(Math.round(num(raw))));
        applyColor(el, b, scope);
      },
    };
  },

  list: (b) => {
    const el = h("ul", "wf-list");
    return {
      el,
      update(scope) {
        const items = render(b.items, scope);
        const arr = Array.isArray(items) ? items.slice(0, num(b.max, 50)) : [];
        const html = arr.map((item, index) => renderText(b.item ?? "{{ item }}", { ...scope, item, index }));
        if (el.childElementCount !== html.length) {
          el.replaceChildren(...html.map(() => h("li")));
        }
        html.forEach((t, i) => setText(el.children[i] as HTMLElement, t));
      },
    };
  },

  image: (b, host) => {
    const el = h("img", "wf-image");
    el.alt = "";
    el.draggable = false;
    el.style.objectFit = String(b.fit ?? "contain");
    let last = "";
    return {
      el,
      update(scope) {
        const src = renderText(b.src, scope);
        if (src !== last) {
          last = src;
          // Only files inside the widget folder; remote images need the net source (M2).
          el.src = src && !/^[a-z]+:/i.test(src) && !src.startsWith("/") ? host.asset(src) : "";
        }
      },
    };
  },

  clock: (b) => {
    const el = h("div", "wf-clock");
    const s = svg("svg", { viewBox: "0 0 100 100" });
    s.append(svg("circle", { cx: 50, cy: 50, r: 47, class: "face" }));
    for (let i = 0; i < 12; i++) {
      const a = (i / 12) * Math.PI * 2;
      const r1 = i % 3 === 0 ? 36 : 39;
      s.append(
        svg("line", {
          x1: 50 + Math.sin(a) * r1,
          y1: 50 - Math.cos(a) * r1,
          x2: 50 + Math.sin(a) * 43,
          y2: 50 - Math.cos(a) * 43,
          class: i % 3 === 0 ? "tick major" : "tick",
        }),
      );
    }
    const hour = svg("line", { x1: 50, y1: 50, x2: 50, y2: 27, class: "hand hour" });
    const min = svg("line", { x1: 50, y1: 50, x2: 50, y2: 15, class: "hand minute" });
    const sec = svg("line", { x1: 50, y1: 58, x2: 50, y2: 12, class: "hand second" });
    s.append(hour, min, sec, svg("circle", { cx: 50, cy: 50, r: 2.5, class: "pin" }));
    el.append(s);
    return {
      el,
      update(scope) {
        const d = new Date();
        const secs = d.getSeconds();
        const mins = d.getMinutes() + secs / 60;
        const hrs = (d.getHours() % 12) + mins / 60;
        hour.setAttribute("transform", `rotate(${hrs * 30} 50 50)`);
        min.setAttribute("transform", `rotate(${mins * 6} 50 50)`);
        sec.setAttribute("transform", `rotate(${secs * 6} 50 50)`);
        const showSec = b.seconds === undefined ? true : truthy(render(b.seconds, scope));
        sec.style.display = showSec ? "" : "none";
      },
    };
  },

  button: (b, host) => {
    const el = h("button", "wf-button");
    el.type = "button";
    let url = "";
    el.addEventListener("click", () => {
      if (/^https?:\/\//i.test(url)) host.openUrl(url);
    });
    return {
      el,
      update(scope) {
        setText(el, renderText(b.label ?? "Open", scope));
        url = renderText(b.open ?? "", scope);
        el.title = url;
        applyColor(el, b, scope);
      },
    };
  },

  notes: (b, host) => {
    const key = String(b.key ?? "notes");
    const el = h("textarea", "wf-notes");
    el.spellcheck = true;
    el.value = typeof host.storage[key] === "string" ? (host.storage[key] as string) : "";
    let timer: number | undefined;
    el.addEventListener("input", () => {
      clearTimeout(timer);
      timer = window.setTimeout(() => host.saveStorage(key, el.value), 400);
    });
    el.addEventListener("blur", () => {
      clearTimeout(timer);
      host.saveStorage(key, el.value);
    });
    return {
      el,
      update(scope) {
        el.placeholder = renderText(b.placeholder ?? "", scope);
        const paper = renderText(b.paper ?? "", scope);
        if (paper) document.documentElement.dataset.paper = paper;
      },
    };
  },

  checklist: (b, host) => {
    type Item = { text: string; done: boolean };
    const key = String(b.key ?? "items");
    const el = h("div", "wf-checklist");
    const list = h("ul");
    const form = h("form", "wf-checklist-add");
    const input = h("input");
    input.type = "text";
    form.append(input);
    const footer = h("div", "wf-checklist-footer");
    const count = h("span");
    const clear = h("button", "wf-link", "Clear done");
    clear.type = "button";
    footer.append(count, clear);
    el.append(list, form, footer);

    let items: Item[] = Array.isArray(host.storage[key]) ? (host.storage[key] as Item[]) : [];
    const save = () => host.saveStorage(key, items);
    const draw = () => {
      list.replaceChildren(
        ...items.map((it, i) => {
          const li = h("li", it.done ? "done" : "");
          const cb = h("input");
          cb.type = "checkbox";
          cb.checked = it.done;
          cb.addEventListener("change", () => {
            items[i].done = cb.checked;
            save();
            draw();
          });
          const span = h("span", "", it.text);
          const del = h("button", "wf-del", "×");
          del.type = "button";
          del.title = "Delete";
          del.addEventListener("click", () => {
            items.splice(i, 1);
            save();
            draw();
          });
          li.append(cb, span, del);
          return li;
        }),
      );
      const left = items.filter((i) => !i.done).length;
      count.textContent = items.length ? `${left} left` : "";
      clear.style.display = items.some((i) => i.done) ? "" : "none";
    };
    form.addEventListener("submit", (e) => {
      e.preventDefault();
      const t = input.value.trim();
      if (!t) return;
      items.push({ text: t, done: false });
      input.value = "";
      save();
      draw();
    });
    clear.addEventListener("click", () => {
      items = items.filter((i) => !i.done);
      save();
      draw();
    });
    draw();
    return {
      el,
      update(scope) {
        input.placeholder = renderText(b.placeholder ?? "Add…", scope);
      },
    };
  },
};

function container(cls: string, b: Block, host: Host): Rendered {
  const el = h("div", cls);
  if (b.gap !== undefined) el.style.gap = `${num(b.gap, 8)}px`;
  if (b.align) el.style.alignItems = String(b.align);
  const children = (b.children ?? []).map((c) => build(c, host));
  el.append(...children.map((c) => c.el));
  return { el, update: (scope) => children.forEach((c) => c.update(scope)) };
}

export function build(b: Block, host: Host): Rendered {
  const make = builders[b.type];
  const inner: Rendered = make
    ? make(b, host)
    : { el: h("div", "wf-unknown", `Unknown block: ${b.type}`), update() {} };
  if (b.visible === undefined) return inner;
  return {
    el: inner.el,
    update(scope) {
      const show = truthy(render(b.visible, scope));
      inner.el.style.display = show ? "" : "none";
      if (show) inner.update(scope);
    },
  };
}

export function buildAll(blocks: Block[], host: Host): Rendered {
  const root = container("wf-column wf-root", { type: "column", children: blocks }, host);
  return root;
}
