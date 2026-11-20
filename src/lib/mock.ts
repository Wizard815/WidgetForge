// In-browser stand-in for the Rust backend, used when the UI runs outside Tauri
// (`npm run dev` in a browser, screenshots, UI tests). Not loaded in the app.

import { parse } from "yaml";
import type { AppSettings, HubState, Instance, InstanceInfo, Manifest, WidgetInfo } from "./types";

const files = import.meta.glob("../../widgets/*/widget.yaml", { query: "?raw", import: "default", eager: true }) as Record<
  string,
  string
>;

function withDefaults(raw: any): Manifest {
  return {
    category: "other",
    settings: [],
    data: {},
    ...raw,
    window: {
      mode: "tile",
      size: [240, 160],
      resizable: true,
      layer: "desktop",
      edge: "right",
      multi_instance: false,
      ...(raw.window ?? {}),
    },
    style: { theme: "glass", ...(raw.style ?? {}) },
    permissions: { system: [], network: [], storage: false, ...(raw.permissions ?? {}) },
  };
}

const manifests: Manifest[] = Object.values(files).map((t) => withDefaults(parse(t)));
const byId = new Map(manifests.map((m) => [m.id, m]));

const settings: AppSettings = { launch_on_startup: true, show_hub_on_launch: true, theme: "system" };
let seq = 0;
const instances: InstanceInfo[] = [];
const storage = new Map<string, Record<string, unknown>>();

function newInstance(m: Manifest): InstanceInfo {
  const i: InstanceInfo = {
    id: `mock${++seq}`,
    widget_id: m.id,
    x: 1500 - seq * 28,
    y: 40 + seq * 28,
    w: m.window.size[0],
    h: m.window.size[1],
    mode: m.window.mode,
    layer: m.window.layer,
    edge: m.window.edge,
    start_with_app: true,
    show_in_taskbar: false,
    opacity: 1,
    locked: false,
    settings: Object.fromEntries(m.settings.map((s) => [s.key, s.default])),
    open: true,
    missing: false,
  };
  instances.push(i);
  return i;
}

for (const id of ["forge.clock", "forge.system-monitor", "forge.sticky-notes"]) {
  const m = byId.get(id);
  if (m) newInstance(m);
}
if (instances[2]) instances[2].open = false;

function state(): HubState {
  const widgets: WidgetInfo[] = manifests.map((m) => ({
    manifest: m,
    origin: "builtin",
    warnings: [],
    instances: instances.filter((i) => i.widget_id === m.id).length,
  }));
  return {
    widgets,
    errors: [],
    instances: structuredClone(instances),
    settings: { ...settings },
    addons_dir: "C:\\Users\\you\\AppData\\Roaming\\com.widgetforge.app\\addons",
    version: "0.1.0",
    platform: "windows",
  };
}

const listeners = new Map<string, Set<(p: unknown) => void>>();
function emit(event: string, payload: unknown = null) {
  listeners.get(event)?.forEach((cb) => cb(payload));
}
export function mockListen(event: string, cb: (p: unknown) => void): () => void {
  if (!listeners.has(event)) listeners.set(event, new Set());
  listeners.get(event)!.add(cb);
  return () => listeners.get(event)?.delete(cb);
}

let cpuSeed = 30;
function fakeSystem() {
  cpuSeed = Math.max(3, Math.min(97, cpuSeed + (Math.random() - 0.5) * 18));
  const gb = 1024 ** 3;
  return {
    cpu: { usage: cpuSeed, name: "AMD Ryzen 7 7800X3D", count: 16, cores: Array.from({ length: 16 }, () => ({ usage: Math.random() * 100 })) },
    memory: { used: 13.4 * gb, total: 32 * gb, usage: 41.9 },
    swap: { used: 0, total: 8 * gb, usage: 0 },
    disk: { name: "C:", mount: "C:\\", used: 612 * gb, total: 931 * gb, usage: 65.7 },
    disks: [{ name: "C:", mount: "C:\\", used: 612 * gb, total: 931 * gb, usage: 65.7 }],
    network: { rx: 1.8e6 * Math.random(), tx: 2.4e5 * Math.random(), rx_total: 0, tx_total: 0 },
    uptime: 200000,
    os: { name: "Windows", version: "11", hostname: "DESKTOP-FORGE" },
  };
}

/** The widget shown by widget.html?mock=<id>. */
function mockWidgetId(): string {
  return new URLSearchParams(location.search).get("mock") ?? "forge.clock";
}

export async function mockInvoke<T>(cmd: string, args: Record<string, any> = {}): Promise<T> {
  const find = (id: string) => {
    const i = instances.find((x) => x.id === id);
    if (!i) throw new Error("instance not found");
    return i;
  };
  const changed = () => emit("forge://changed");
  const r = (v: unknown) => v as T;
  switch (cmd) {
    case "hub_state":
      return r(state());
    case "instance_add": {
      const m = byId.get(args.widgetId)!;
      if (!m.window.multi_instance && instances.some((i) => i.widget_id === m.id)) {
        throw new Error(`${m.name} can only be added once`);
      }
      const i = newInstance(m);
      changed();
      return r(i);
    }
    case "instance_update": {
      const i = find(args.id);
      const { settings: s, ...rest } = args.patch as Partial<Instance>;
      Object.assign(i, rest);
      if (s) Object.assign(i.settings, s);
      changed();
      return r(i);
    }
    case "instance_remove":
      instances.splice(instances.indexOf(find(args.id)), 1);
      changed();
      return r(undefined);
    case "instance_set_open":
      find(args.id).open = args.open;
      changed();
      return r(undefined);
    case "settings_update":
      Object.assign(settings, args.patch);
      changed();
      return r({ ...settings });
    case "widgets_set_hidden":
    case "registry_reload":
    case "addons_open_folder":
    case "app_quit":
      return r(undefined);
    case "package_import":
      throw new Error("Importing needs the desktop app.");

    // Widget side
    case "widget_context": {
      const m = byId.get(mockWidgetId())!;
      const inst = instances.find((i) => i.widget_id === m.id) ?? newInstance(m);
      return r({ instance: inst, manifest: m, settings: { ...inst.settings }, origin: "builtin", platform: "windows" });
    }
    case "widget_system":
      return r(fakeSystem());
    case "widget_storage_all":
      return r({ ...(storage.get(mockWidgetId()) ?? {}) });
    case "widget_storage_set": {
      const s = storage.get(mockWidgetId()) ?? {};
      s[args.key] = args.value;
      storage.set(mockWidgetId(), s);
      return r(undefined);
    }
    case "widget_drawer":
    case "widget_close":
    case "widget_open_settings":
    case "widget_set_layer":
    case "widget_set_locked":
    case "open_url":
      return r(undefined);
    default:
      throw new Error(`mock: unknown command ${cmd}`);
  }
}
