// Hub state shared by all pages.

import { errorText, hub, inTauri, listen, widgetBaseUrl } from "../lib/api";
import type { HubState, ImportPreview, Manifest } from "../lib/types";

export type Page = "desktop" | "library" | "addons" | "create" | "settings";

export const app = $state({
  state: null as HubState | null,
  page: "desktop" as Page,
  base: "",
  focus: null as string | null,
  toast: null as { text: string; kind: "info" | "error" } | null,
  importing: null as { path: string; preview: ImportPreview } | null,
});

let toastTimer: number | undefined;
export function toast(text: string, kind: "info" | "error" = "info") {
  app.toast = { text, kind };
  clearTimeout(toastTimer);
  toastTimer = window.setTimeout(() => (app.toast = null), kind === "error" ? 6000 : 2600);
}

/** Runs an action, shows errors as a toast, and optionally a success message. */
export async function act<T>(fn: () => Promise<T>, ok?: string): Promise<T | undefined> {
  try {
    const r = await fn();
    if (ok) toast(ok);
    return r;
  } catch (e) {
    toast(errorText(e), "error");
    return undefined;
  }
}

export async function refresh() {
  try {
    app.state = await hub.state();
  } catch (e) {
    toast(errorText(e), "error");
  }
}

export function iconUrl(m: Manifest): string | null {
  return m.icon ? `${app.base}${m.id}/${m.icon}` : null;
}

export function manifestFor(widgetId: string): Manifest | undefined {
  return app.state?.widgets.find((w) => w.manifest.id === widgetId)?.manifest;
}

export async function startImport(path: string) {
  const preview = await act(() => hub.importPackage(path, false));
  if (preview) app.importing = { path, preview };
}

export async function confirmImport() {
  if (!app.importing) return;
  const { path, preview } = app.importing;
  app.importing = null;
  await act(() => hub.importPackage(path, true), `${preview.manifest.name} installed`);
  await refresh();
}

function applyTheme() {
  const pref = app.state?.settings.theme ?? "system";
  const dark = pref === "dark" || (pref === "system" && matchMedia("(prefers-color-scheme: dark)").matches);
  document.documentElement.dataset.theme = dark ? "dark" : "light";
}

export async function init() {
  app.base = await widgetBaseUrl();
  await refresh();
  $effect.root(() => {
    $effect(() => applyTheme());
  });
  matchMedia("(prefers-color-scheme: dark)").addEventListener("change", applyTheme);
  await listen("forge://changed", refresh);
  await listen<string>("forge://focus-instance", (id) => {
    app.page = "desktop";
    app.focus = id;
  });
  await listen<string>("forge://import-request", (path) => startImport(path));
  if (inTauri) {
    const { getCurrentWebview } = await import("@tauri-apps/api/webview");
    await getCurrentWebview().onDragDropEvent((e) => {
      if (e.payload.type === "drop" && e.payload.paths.length) startImport(e.payload.paths[0]);
    });
  }
}

export const PERMISSION_TEXT: Record<string, string> = {
  "system:cpu": "Read CPU usage",
  "system:memory": "Read memory usage",
  "system:disk": "Read disk space",
  "system:network": "Read network speed",
  "system:os": "Read computer name and uptime",
  "system:battery": "Read battery level",
  "system:gpu": "Read GPU usage",
  storage: "Save its own data",
};

export function permissionList(m: Manifest): { key: string; text: string }[] {
  const out = m.permissions.system.map((s) => ({ key: `system:${s}`, text: PERMISSION_TEXT[`system:${s}`] ?? s }));
  for (const h of m.permissions.network) out.push({ key: `network:${h}`, text: `Connect to ${h}` });
  if (m.permissions.storage) out.push({ key: "storage", text: PERMISSION_TEXT.storage });
  return out;
}

export function describePermission(key: string): string {
  if (key.startsWith("network:")) return `Connect to ${key.slice(8)}`;
  return PERMISSION_TEXT[key] ?? key;
}
