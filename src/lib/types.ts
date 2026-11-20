// Mirrors of the Rust types sent over IPC (src-tauri/src/*.rs).

export type Mode = "tile" | "drawer";
export type Layer = "top" | "normal" | "desktop";
export type Edge = "left" | "right" | "top" | "bottom";
export type Origin = "builtin" | "addon";

export interface SettingDef {
  key: string;
  type: "text" | "number" | "bool" | "choice" | "color";
  label?: string;
  default: unknown;
  options?: unknown[];
  min?: number;
  max?: number;
}

export interface Block {
  type: string;
  visible?: string;
  children?: Block[];
  [key: string]: unknown;
}

export interface Manifest {
  schema: number;
  id: string;
  name: string;
  version: string;
  author?: string | null;
  description?: string | null;
  category: string;
  icon?: string | null;
  window: {
    mode: Mode;
    size: [number, number];
    min_size?: [number, number] | null;
    resizable: boolean;
    layer: Layer;
    edge: Edge;
    multi_instance: boolean;
  };
  style: { theme: "glass" | "solid" | "none"; accent?: string | null };
  permissions: { system: string[]; network: string[]; storage: boolean };
  settings: SettingDef[];
  data: Record<string, { source: string; interval?: number | null }>;
  ui: { blocks?: Block[]; entry?: string };
}

export interface Issue {
  level: "error" | "warning";
  path: string;
  message: string;
}

export interface Instance {
  id: string;
  widget_id: string;
  x: number;
  y: number;
  w: number;
  h: number;
  monitor?: string | null;
  mode: Mode;
  layer: Layer;
  edge: Edge;
  start_with_app: boolean;
  show_in_taskbar: boolean;
  opacity: number;
  locked: boolean;
  settings: Record<string, unknown>;
}

export interface InstanceInfo extends Instance {
  open: boolean;
  missing: boolean;
}

export interface WidgetInfo {
  manifest: Manifest;
  origin: Origin;
  warnings: Issue[];
  instances: number;
}

export interface AppSettings {
  launch_on_startup: boolean;
  show_hub_on_launch: boolean;
  theme: "system" | "dark" | "light";
}

export interface HubState {
  widgets: WidgetInfo[];
  errors: { folder: string; issues: Issue[] }[];
  instances: InstanceInfo[];
  settings: AppSettings;
  addons_dir: string;
  version: string;
  platform: string;
}

export interface ImportPreview {
  manifest: Manifest;
  warnings: Issue[];
  replaces_version: string | null;
  new_permissions: string[];
}

export interface WidgetContext {
  instance: Instance;
  manifest: Manifest;
  settings: Record<string, unknown>;
  origin: Origin;
  platform: string;
}
