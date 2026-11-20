// Widget host: one per widget window. Renders declarative blocks, or runs an HTML
// widget in a sandboxed iframe and answers its SDK calls.

import "./widget.css";
import { errorText, inTauri, listen, startDragging, startResizing, widget, widgetBaseUrl } from "../lib/api";
import type { Layer, WidgetContext } from "../lib/types";
import type { Scope } from "../lib/template";
import { buildAll } from "./blocks";

const root = document.getElementById("app")!;
let ctx: WidgetContext;
let base = "";
let frame: HTMLIFrameElement | null = null;
let drawerOpen = false;
let refreshBlocks: (() => void) | null = null;
const timers: number[] = [];

const ICONS = {
  gear: '<path d="M12 8.5a3.5 3.5 0 1 0 0 7 3.5 3.5 0 0 0 0-7Zm8.4 4.6-1.8 1 .1 2-2 1.2-1.6-1.2-1.9.8-.6 1.9h-2.4l-.6-1.9-1.9-.8-1.6 1.2-2-1.2.1-2-1.8-1v-2.2l1.8-1-.1-2 2-1.2 1.6 1.2 1.9-.8.6-1.9h2.4l.6 1.9 1.9.8 1.6-1.2 2 1.2-.1 2 1.8 1Z" fill="none" stroke="currentColor" stroke-width="1.6"/>',
  close: '<path d="M7 7l10 10M17 7L7 17" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>',
  top: '<path d="M12 4l6 7h-4v9h-4v-9H6z" fill="currentColor"/>',
  normal: '<rect x="5" y="5" width="14" height="14" rx="3" fill="none" stroke="currentColor" stroke-width="2"/>',
  desktop: '<path d="M12 20l6-7h-4V4h-4v9H6z" fill="currentColor"/>',
  lock: '<rect x="6" y="11" width="12" height="9" rx="2" fill="currentColor"/><path d="M8.5 11V8a3.5 3.5 0 0 1 7 0v3" fill="none" stroke="currentColor" stroke-width="2"/>',
  unlock: '<rect x="6" y="11" width="12" height="9" rx="2" fill="currentColor"/><path d="M8.5 11V8a3.5 3.5 0 0 1 6.8-1.2" fill="none" stroke="currentColor" stroke-width="2"/>',
};
const LAYER_NEXT: Record<Layer, Layer> = { desktop: "normal", normal: "top", top: "desktop" };
const LAYER_TITLE: Record<Layer, string> = {
  top: "Always on top (click: on desktop)",
  normal: "Normal window (click: always on top)",
  desktop: "On the desktop (click: normal window)",
};

function icon(name: keyof typeof ICONS) {
  return `<svg viewBox="0 0 24 24" width="14" height="14" aria-hidden="true">${ICONS[name]}</svg>`;
}

function applyStyle() {
  const de = document.documentElement;
  de.dataset.theme = ctx.manifest.style.theme;
  de.dataset.mode = ctx.instance.mode;
  de.dataset.edge = ctx.instance.edge;
  de.style.setProperty("--wf-accent", ctx.manifest.style.accent || "#7cc4ff");
  de.style.setProperty("--wf-opacity", String(ctx.instance.opacity ?? 1));
  de.classList.toggle("locked", ctx.instance.locked);
}

function toolbar(): HTMLElement {
  const bar = document.createElement("div");
  bar.className = "wf-toolbar";
  const btn = (name: keyof typeof ICONS, title: string, onClick: () => void) => {
    const b = document.createElement("button");
    b.type = "button";
    b.title = title;
    b.innerHTML = icon(name);
    b.addEventListener("click", (e) => {
      e.stopPropagation();
      onClick();
    });
    return b;
  };
  bar.append(
    btn("close", "Close widget", () => widget.close()),
    btn("gear", "Settings", () => widget.openSettings()),
    btn(ctx.instance.layer, LAYER_TITLE[ctx.instance.layer], () => widget.setLayer(LAYER_NEXT[ctx.instance.layer])),
    btn(ctx.instance.locked ? "lock" : "unlock", ctx.instance.locked ? "Unlock position" : "Lock position", () =>
      widget.setLocked(!ctx.instance.locked),
    ),
  );
  return bar;
}

/** Drag the window from anywhere that is not a control. */
function enableDrag(el: HTMLElement) {
  el.addEventListener("mousedown", (e) => {
    if (e.button !== 0 || ctx.instance.locked || ctx.instance.mode === "drawer") return;
    const t = e.target as HTMLElement;
    if (t.closest("button, input, textarea, select, a, iframe, [data-no-drag], .wf-resize")) return;
    e.preventDefault();
    startDragging();
  });
}

async function renderBlocks(content: HTMLElement) {
  const m = ctx.manifest;
  const storage: Record<string, unknown> = m.permissions.storage ? await widget.storageAll().catch(() => ({})) : {};
  const scope: Scope = { settings: ctx.settings, instance: ctx.instance, storage, __tick: 0 };
  const view = buildAll(m.ui.blocks ?? [], {
    storage,
    saveStorage: (k, v) => {
      storage[k] = v;
      widget.storageSet(k, v).catch((e) => console.error("storage", e));
    },
    openUrl: (url) => widget.openUrl(url),
    asset: (p) => `${base}${m.id}/${p}`,
  });
  content.append(view.el);
  const refresh = () => view.update({ ...scope, settings: ctx.settings, instance: ctx.instance });

  // Data sources. Each one updates its slot in the scope and re-renders.
  for (const [name, def] of Object.entries(m.data)) {
    const interval = Math.max(250, def.interval ?? 1000);
    const tick = async () => {
      try {
        if (def.source === "clock") scope[name] = { now: Date.now() };
        else if (def.source === "system") scope[name] = await widget.system();
        scope.__tick = (scope.__tick as number) + 1;
        refresh();
      } catch (e) {
        console.warn(`data source ${name}:`, errorText(e));
      }
    };
    await tick();
    timers.push(window.setInterval(tick, interval));
  }
  // The analog clock block animates on its own even without a clock source.
  if (JSON.stringify(m.ui.blocks).includes('"clock"') && !Object.values(m.data).some((d) => d.source === "clock")) {
    timers.push(window.setInterval(refresh, 1000));
  }
  refresh();
  return refresh;
}

/** HTML widgets: sandboxed iframe + SDK bridge. */
async function renderFrame(content: HTMLElement) {
  const m = ctx.manifest;
  frame = document.createElement("iframe");
  frame.className = "wf-frame";
  frame.setAttribute("sandbox", "allow-scripts");
  frame.setAttribute("referrerpolicy", "no-referrer");
  if (inTauri) {
    frame.src = `${base}${m.id}/${m.ui.entry}`;
  } else {
    // Browser preview: inject the SDK ourselves (the real app does it in Rust).
    const [html, sdk] = await Promise.all([
      fetch(`/widgets/${m.id}/${m.ui.entry}`).then((r) => r.text()),
      fetch("/src/sdk/forge-sdk.js").then((r) => r.text()),
    ]);
    frame.srcdoc = html.replace(/<script[^>]*@vite\/client[^>]*><\/script>/g, "").replace(/<head>/i, `<head><base href="/widgets/${m.id}/"><script>${sdk}</script>`);
  }
  content.append(frame);
}

// SDK calls from the HTML widget iframe.
window.addEventListener("message", async (e) => {
  if (!frame || e.source !== frame.contentWindow) return;
  const msg = e.data;
  if (!msg || msg.__forge !== 1 || typeof msg.id !== "number") return;
  const reply = (result: unknown, error?: string) =>
    frame?.contentWindow?.postMessage({ __forge: 1, id: msg.id, result, error }, "*");
  try {
    const p = msg.params ?? {};
    switch (msg.method) {
      case "context":
        return reply({ instance: ctx.instance, manifest: ctx.manifest, settings: ctx.settings, platform: ctx.platform });
      case "system":
        return reply(await widget.system());
      case "fetch":
        return reply(await widget.fetch({ url: p.url, method: p.method, headers: p.headers, body: p.body }));
      case "storage.all":
        return reply(await widget.storageAll());
      case "storage.get":
        return reply((await widget.storageAll())[p.key] ?? null);
      case "storage.set":
        return reply(await widget.storageSet(String(p.key), p.value ?? null));
      case "openUrl":
        return reply(await widget.openUrl(String(p.url)));
      default:
        return reply(null, `unknown method ${msg.method}`);
    }
  } catch (err) {
    reply(null, errorText(err));
  }
});

function drawerHandle(): HTMLElement {
  const handle = document.createElement("button");
  handle.className = "wf-drawer-handle";
  handle.type = "button";
  handle.title = ctx.manifest.name;
  handle.innerHTML = "<span></span>";
  // Hovering the handle opens the drawer after a short pause; clicking toggles it.
  // A click right after a hover-open must not immediately close it again.
  let hoverTimer: number | undefined;
  let openedAt = 0;
  handle.addEventListener("mouseenter", () => {
    if (drawerOpen) return;
    hoverTimer = window.setTimeout(() => {
      openedAt = Date.now();
      setDrawer(true);
    }, 220);
  });
  handle.addEventListener("mouseleave", () => clearTimeout(hoverTimer));
  handle.addEventListener("click", () => {
    clearTimeout(hoverTimer);
    if (drawerOpen && Date.now() - openedAt < 600) return;
    openedAt = Date.now();
    setDrawer(!drawerOpen);
  });
  return handle;
}

function setDrawer(open: boolean) {
  drawerOpen = open;
  document.documentElement.classList.toggle("drawer-open", open);
  widget.drawer(open).catch(() => {});
}

async function mount() {
  root.replaceChildren();
  timers.splice(0).forEach(clearInterval);
  try {
    ctx = await widget.context();
  } catch (e) {
    root.textContent = `Could not load widget: ${errorText(e)}`;
    return;
  }
  base = await widgetBaseUrl();
  document.title = ctx.manifest.name;
  applyStyle();

  const shell = document.createElement("div");
  shell.className = "wf-shell";
  const content = document.createElement("div");
  content.className = "wf-content";
  shell.append(content, toolbar());
  if (ctx.manifest.window.resizable && ctx.instance.mode === "tile") {
    const grip = document.createElement("div");
    grip.className = "wf-resize";
    grip.addEventListener("mousedown", (e) => {
      if (e.button === 0 && !ctx.instance.locked) {
        e.preventDefault();
        startResizing();
      }
    });
    shell.append(grip);
  }
  if (ctx.instance.mode === "drawer") shell.append(drawerHandle());
  enableDrag(shell);
  root.append(shell);

  frame = null;
  refreshBlocks = null;
  if (ctx.manifest.ui.entry) await renderFrame(content);
  else refreshBlocks = await renderBlocks(content);
}

// Settings or instance options changed in the Hub.
listen("forge://instance", async () => {
  const next = await widget.context();
  const rebuild = next.instance.mode !== ctx.instance.mode || next.instance.edge !== ctx.instance.edge;
  ctx = next;
  if (rebuild) return mount();
  applyStyle();
  root.querySelector(".wf-toolbar")?.replaceWith(toolbar());
  if (frame) frame.contentWindow?.postMessage({ __forge: 1, event: "settings", data: ctx.settings }, "*");
  else window.dispatchEvent(new Event("wf-refresh"));
});

// Collapse drawers when the user clicks elsewhere.
window.addEventListener("blur", () => {
  if (ctx?.instance.mode === "drawer" && drawerOpen) setDrawer(false);
});

window.addEventListener("wf-refresh", () => refreshBlocks?.());

mount();
