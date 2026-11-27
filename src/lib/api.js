// Thin wrappers around the Rust commands.
import { invoke, convertFileSrc } from "@tauri-apps/api/core";

// Svelte state objects are proxies; send plain copies.
const plain = (v) => (v === undefined ? v : JSON.parse(JSON.stringify(v)));

export const api = {
  // Hub
  listWidgets: () => invoke("list_widgets"),
  reloadWidgets: () => invoke("reload_widgets"),
  listInstances: () => invoke("list_instances"),
  addInstance: (widgetId) => invoke("add_instance", { widgetId }),
  removeInstance: (id) => invoke("remove_instance", { id }),
  showInstance: (id, show) => invoke("show_instance", { id, show }),
  updateInstance: (id, patch) => invoke("update_instance", { id, patch: plain(patch) }),
  moveToStack: (id, stackId) => invoke("move_to_stack", { id, stackId: stackId || null }),
  setSecret: (id, key, value) => invoke("set_secret", { id, key, value }),
  openHub: (instanceId) => invoke("open_hub", { instanceId }),
  openAddonsFolder: () => invoke("open_addons_folder"),
  openLink: (url) => invoke("hub_open_link", { url }),
  appPaths: () => invoke("app_paths"),
  listPresets: () => invoke("list_presets"),
  savePreset: (name) => invoke("save_preset", { name }),
  applyPreset: (file) => invoke("apply_preset", { file }),
  deletePreset: (file) => invoke("delete_preset", { file }),
  openPresetsFolder: () => invoke("open_presets_folder"),
  discordLogin: () => invoke("discord_login"),
  discordStatus: () => invoke("discord_status"),
  discordLogout: () => invoke("discord_logout"),
  getTheme: () => invoke("get_theme"),
  exportWidget: (id) => invoke("export_widget", { id }),
  revealFile: (path) => invoke("reveal_file", { path }),
  inspectPackage: (path) => invoke("inspect_package", { path }),
  installPackage: (path, replace) => invoke("install_package", { path, replace }),
  pendingImports: () => invoke("pending_imports"),
  registryIndex: () => invoke("registry_index"),
  setRegistryUrl: (url) => invoke("set_registry_url", { url }),
  registryDownload: (url, sha256, size) => invoke("registry_download", { url, sha256, size }),
  drawerSet: (open) => invoke("drawer_set", { open }),
  drawerSlide: (delta) => invoke("drawer_slide", { delta }),
  setTheme: (theme) => invoke("set_theme", { theme: plain(theme) }),
  builderSave: (manifest, replace) => invoke("builder_save", { manifest: plain(manifest), replace }),
  builderSampleSystem: () => invoke("builder_sample_system"),
  builderFetch: (url, headers) => invoke("builder_fetch", { url, headers: plain(headers) }),
  builderRun: (command) => invoke("builder_run", { command }),
  setDiscordClientId: (clientId) => invoke("set_discord_client_id", { clientId }),

  // Widget windows. `instance` is the widget being served; a stack window
  // passes the id of the child widget it is showing.
  widgetContext: () => invoke("widget_context"),
  systemStats: (instance) => invoke("system_stats", { instance }),
  storageGet: (instance, key) => invoke("storage_get", { instance, key }),
  storageSet: (instance, key, value) => invoke("storage_set", { instance, key, value: plain(value) }),
  netFetch: (instance, req) => invoke("net_fetch", { instance, ...plain(req) }),
  mediaStatus: (instance) => invoke("media_status", { instance }),
  mediaControl: (instance, action) => invoke("media_control", { instance, action }),
  clipboardHistory: (instance) => invoke("clipboard_history", { instance }),
  discordApi: (instance, endpoint) => invoke("discord_api", { instance, endpoint }),
  folderList: (instance, key, kind) => invoke("folder_list", { instance, key, kind }),
  openFolderItem: (instance, key, name) => invoke("open_folder_item", { instance, key, name }),
  tailFile: (instance, key, lines) => invoke("tail_file", { instance, key, lines }),
  wakeOnLan: (instance, key) => invoke("wake_on_lan", { instance, key }),
  minecraftStatus: (instance, key) => invoke("minecraft_status", { instance, key }),
  screenTime: (instance) => invoke("screen_time", { instance }),
  screenTimeClear: (instance) => invoke("screen_time_clear", { instance }),
  mailCheck: (instance) => invoke("mail_check", { instance }),
  notify: (instance, title, body) => invoke("notify", { instance, title, body }),
  speedTest: (instance) => invoke("speed_test", { instance }),
  netLatency: (instance, host, port) => invoke("net_latency", { instance, host, port }),
  audioStatus: (instance) => invoke("audio_status", { instance }),
  audioSet: (instance, change) => invoke("audio_set", { instance, change: plain(change) }),
  dockerList: (instance) => invoke("docker_list", { instance }),
  dockerAction: (instance, id, action) => invoke("docker_action", { instance, id, action }),
  runCommand: (instance, key) => invoke("run_command", { instance, key }),
  clipboardCopy: (instance, text) => invoke("clipboard_copy", { instance, text }),
  clipboardClear: (instance) => invoke("clipboard_clear", { instance }),
  openUrl: (url) => invoke("open_url", { url }),
};

// widget://localhost/ on Linux and macOS, http://widget.localhost/ on Windows.
const base = convertFileSrc("", "widget");

/** URL of a file in a folder the user chose in a widget's settings. */
export function userFile(instanceId, key, name) {
  return `${base}__folder/${encodeURIComponent(instanceId)}/${encodeURIComponent(key)}/${encodeURIComponent(name)}`;
}

/** URL of a file inside a widget's folder. */
export function widgetFile(widgetId, path) {
  return `${base}${widgetId}/${String(path).split("/").map(encodeURIComponent).join("/")}`;
}
