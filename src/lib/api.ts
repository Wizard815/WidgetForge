// Thin wrappers around Tauri IPC. When the page runs in a normal browser (UI work,
// screenshots), calls go to an in-memory mock instead.

import type { AppSettings, HubState, ImportPreview, Instance, Layer, WidgetContext } from "./types";

export const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

type Invoke = <T>(cmd: string, args?: Record<string, unknown>) => Promise<T>;

let invokeImpl: Promise<Invoke> | null = null;

function getInvoke(): Promise<Invoke> {
  if (!invokeImpl) {
    invokeImpl = inTauri
      ? import("@tauri-apps/api/core").then((m) => m.invoke as Invoke)
      : import("./mock").then((m) => m.mockInvoke as Invoke);
  }
  return invokeImpl;
}

export async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const invoke = await getInvoke();
  return invoke<T>(cmd, args);
}

/** Subscribe to a backend event. Returns an unsubscribe function. */
export async function listen<T>(event: string, cb: (payload: T) => void): Promise<() => void> {
  if (!inTauri) {
    const m = await import("./mock");
    return m.mockListen(event, cb as (p: unknown) => void);
  }
  const { getCurrentWebviewWindow } = await import("@tauri-apps/api/webviewWindow");
  return getCurrentWebviewWindow().listen<T>(event, (e) => cb(e.payload));
}

/** Base URL of the widget:// scheme ("widget://localhost/" or "http://widget.localhost/"). */
export async function widgetBaseUrl(): Promise<string> {
  if (!inTauri) return "/widgets/";
  const { convertFileSrc } = await import("@tauri-apps/api/core");
  return convertFileSrc("x", "widget").slice(0, -1);
}

export const hub = {
  state: () => call<HubState>("hub_state"),
  addInstance: (widgetId: string) => call<Instance>("instance_add", { widgetId }),
  updateInstance: (id: string, patch: Partial<Instance>) => call<Instance>("instance_update", { id, patch }),
  removeInstance: (id: string) => call<void>("instance_remove", { id }),
  setOpen: (id: string, open: boolean) => call<void>("instance_set_open", { id, open }),
  setHidden: (hidden: boolean) => call<void>("widgets_set_hidden", { hidden }),
  reload: () => call<void>("registry_reload"),
  updateSettings: (patch: Partial<AppSettings>) => call<AppSettings>("settings_update", { patch }),
  openAddonsFolder: () => call<void>("addons_open_folder"),
  importPackage: (path: string, install: boolean) => call<ImportPreview>("package_import", { path, install }),
  exportPackage: (widgetId: string, dest: string) => call<void>("package_export", { widgetId, dest }),
  uninstall: (widgetId: string) => call<void>("widget_uninstall", { widgetId }),
  quit: () => call<void>("app_quit"),
};

export const widget = {
  context: () => call<WidgetContext>("widget_context"),
  system: () => call<Record<string, unknown>>("widget_system"),
  storageAll: () => call<Record<string, unknown>>("widget_storage_all"),
  storageSet: (key: string, value: unknown) => call<void>("widget_storage_set", { key, value }),
  fetch: (req: { url: string; method?: string; headers?: Record<string, string>; body?: string }) =>
    call<{ status: number; content_type: string | null; body: string }>("widget_fetch", { req }),
  drawer: (open: boolean) => call<void>("widget_drawer", { open }),
  close: () => call<void>("widget_close"),
  openSettings: () => call<void>("widget_open_settings"),
  setLayer: (layer: Layer) => call<void>("widget_set_layer", { layer }),
  setLocked: (locked: boolean) => call<void>("widget_set_locked", { locked }),
  openUrl: (url: string) => call<void>("open_url", { url }),
};

export async function startDragging(): Promise<void> {
  if (!inTauri) return;
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  await getCurrentWindow().startDragging();
}

export async function startResizing(): Promise<void> {
  if (!inTauri) return;
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  await getCurrentWindow().startResizeDragging("SouthEast");
}

export function errorText(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}
