# WidgetForge — Design Document

Status: draft v0.1 · Scope: core runtime, main menu ("Hub"), widget format, distribution, builder assistant.

## 1. What we are building

WidgetForge is a desktop widget platform in the spirit of Windows 7 gadgets: small,
always-available panels (clock, weather, system stats, notes, music…) that live on the
desktop. It is sold on Steam but **runs without Steam**, and anyone can make widgets.

Goals

- Windows first; macOS and Linux builds from the same codebase.
- Each widget behaves like its own little app (own window, settings, storage, permissions,
  optional taskbar icon) while one shared core runs them all.
- Widgets are described by a `widget.yaml` manifest. Simple widgets are pure YAML; complex
  ones add HTML/JS.
- Easy to make, share, import and export widgets: local add-ons folder, a GitHub registry,
  and Steam Workshop.
- An optional small AI assistant helps people build widgets, and fits on a 6 GB GPU.

Non-goals (for v1)

- Running KDE Plasma / Rainmeter / Windows gadget packages directly.
- A widget marketplace with paid widgets.
- Mobile.

## 2. Glossary

| Term | Meaning |
|---|---|
| Core | The WidgetForge process: Rust backend + webviews. |
| Hub | The main menu window: library, desktop layout, add-ons, settings, builder. |
| Widget | A package (folder or `.forgewidget` file) with a `widget.yaml`. |
| Instance | One placed copy of a widget, with its own position, settings and storage. A widget may allow many instances (e.g. sticky notes). |
| Block | A built-in UI element a manifest can use without code (text, gauge, graph…). |
| Source | A built-in data provider (clock, system, network fetch, media…). |

## 3. Architecture

```
┌──────────────────────────── WidgetForge process ────────────────────────────┐
│ Rust core (Tauri 2)                                                         │
│  ├─ registry      scan built-in + add-ons folders, parse/validate manifests │
│  ├─ layout        instances, positions per monitor, startup set (JSON)      │
│  ├─ windows       create/destroy widget windows, layers, drawers, taskbar   │
│  ├─ sources       clock (in JS), system (sysinfo), net fetch (allowlisted)  │
│  ├─ storage       per-instance key/value JSON                               │
│  ├─ packages      import/export .forgewidget, path-safety checks            │
│  ├─ tray/autostart/single-instance                                          │
│  └─ permissions   every command checks the calling window's manifest        │
│                                                                             │
│ Webviews                                                                    │
│  ├─ hub.html      main menu (Svelte)                                        │
│  └─ widget.html   widget host (one window per instance)                     │
│        ├─ blocks renderer for declarative widgets                           │
│        └─ sandboxed <iframe> + postMessage SDK for HTML/JS widgets          │
└─────────────────────────────────────────────────────────────────────────────┘
```

Why Tauri 2: small install (system webview, not bundled Chromium), Rust backend for
system access, built-in tray/autostart/transparent frameless windows, one codebase for
three OSes, and web tech is what most widget authors already know.

Memory budget: every widget window is a webview. Target < 40 MB per extra widget on
Windows (WebView2 shares one browser process per app). This is measured in milestone M1;
if it fails, the fallback is "compositor mode": several widgets rendered in one
transparent window per monitor.

## 4. Windows and behaviour

### 4.1 Window kinds

- **Hub** — normal decorated window. Closing hides it to the tray; quitting is from the
  tray or Hub.
- **Widget instance** — frameless, transparent, label `w-<instance id>`. Draggable
  anywhere that is not an interactive control. Hover shows a small side toolbar
  (close, settings, layer, lock).

### 4.2 Shapes (`window.mode`)

- `tile` — free-floating panel.
- `drawer` — docked to a screen edge (`left|right|top|bottom`). Collapsed, only a handle
  is visible; clicking or hovering the handle slides it open, and it closes when focus
  leaves (unless pinned).

### 4.3 Layers (`layer`)

| Layer | Behaviour | Implementation |
|---|---|---|
| `top` | Stays above other apps ("focus priority"). | always-on-top |
| `normal` | Behaves like a regular window: comes forward when clicked, goes behind when another app is focused. | default |
| `desktop` | Lives on the desktop behind all apps, like Win7 gadgets. | always-on-bottom |

Users can change the layer per instance; the manifest supplies the default.
Planned: "hide when a fullscreen app/game is focused" (per instance, default on for
`top`).

### 4.4 Taskbar, tray, startup

- Each instance can opt in to its own taskbar button (`show_in_taskbar`). Default off.
  On Windows each widget gets its own AppUserModelID (`WidgetForge.<widget id>`) so pinned
  widgets group separately — planned for M2.
- Tray icon: open Hub, show/hide all widgets, lock positions, quit.
- Run on startup (OS login item) is a Hub setting. Startup launches with `--background`:
  no Hub, only instances marked `start_with_app` are opened at their saved positions.
- Single instance: launching the app again opens the Hub of the running copy.

### 4.5 Positions and monitors

Positions are stored with the monitor they were on (name + resolution). On start, if
that monitor is missing, the instance is moved onto the primary monitor and clamped
inside its work area.

## 5. Widget packages

### 5.1 Layout on disk

```
forge.clock/
  widget.yaml        required
  icon.svg           optional (svg/png)
  index.html, *.js   only for HTML widgets (ui.entry)
  assets/…
```

A `.forgewidget` file is the same folder zipped. Import rejects absolute paths, `..`,
symlinks, files > 20 MB and packages > 100 MB.

### 5.2 Manifest (`widget.yaml`, schema 1)

```yaml
schema: 1
id: forge.system-monitor        # lowercase, dot-separated, ≥ 2 parts, unique
name: System Monitor
version: 1.0.0                  # semver
author: WidgetForge
description: CPU, memory, disk and network at a glance.
category: system                # time|productivity|system|media|info|fun|other
icon: icon.svg

window:
  mode: tile                    # tile | drawer
  size: [260, 220]
  min_size: [180, 120]
  resizable: true
  layer: desktop                # top | normal | desktop
  edge: right                   # drawer only
  multi_instance: false

style:
  theme: glass                  # glass | solid | none
  accent: "#6cb8ff"

permissions:
  system: [cpu, memory, disk, network]   # cpu|memory|disk|network|os|battery|gpu
  network: []                   # exact hostnames for net.fetch
  storage: true                 # per-instance storage

settings:                       # rendered as a form in the Hub
  - { key: show_disk, type: bool, label: Show disk, default: true }

data:                           # named sources the UI can bind to
  sys:  { source: system, interval: 1000 }
  now:  { source: clock,  interval: 1000 }

ui:
  blocks:                       # declarative UI …
    - type: gauge
      label: CPU
      value: "{{ sys.cpu.usage }}"
      max: 100
      format: "{{ sys.cpu.usage | fixed(0) }}%"
  # … or an HTML/JS entry point instead:
  # entry: index.html
```

Unknown keys are kept (forward compatibility) but warned about in the builder.

### 5.3 Blocks (v1)

Layout: `column`, `row`, `grid`, `spacer`.
Display: `text`, `gauge` (ring), `bar`, `graph` (sparkline with history), `list`,
`image`, `clock` (analog).
Interactive: `button` (runs an action), `notes` (editable text saved to storage),
`checklist`.

Bindings use `{{ path | filter(arg) }}`. Paths read from `data` sources, `settings.*`
and `instance.*`. Filters: `fixed(n)`, `round`, `percent`, `bytes`, `rate` (bytes/s),
`date(fmt)`, `time(fmt)`, `upper`, `lower`, `default(x)`. No arbitrary code runs in
bindings.

`visible: "{{ settings.show_disk }}"` hides a block.

### 5.4 HTML/JS widgets

`ui.entry` is loaded in a sandboxed `<iframe>` (`sandbox="allow-scripts"`, no same-origin)
served from the `widget://` scheme. It has **no direct access** to the Tauri API. It talks to
the host through a small SDK (`forge.js`, injected) over `postMessage`:

```js
const sys = await forge.system();             // needs permissions.system
const res = await forge.fetch("https://api.met.no/…"); // host must be in permissions.network
await forge.storage.set("note", text);
forge.settings.onChange(s => render(s));
```

The host forwards each call to the core, which checks the manifest again (defence in
depth).

## 6. Sources and permissions

| Source | Data | Permission |
|---|---|---|
| `clock` | now, timezone, locale | none |
| `system` | cpu (total, per core), memory, swap, disks, network rates, uptime, os, hostname | `system: [...]`, filtered per category |
| `net` | HTTP GET/POST to listed hosts, response size ≤ 2 MB | `network: [hosts]` |
| `storage` | per-instance JSON (≤ 1 MB) | `storage: true` |
| `media` (M2) | now playing + controls via Windows SMTC / Linux MPRIS / macOS adapter | `media: true` |
| `gpu` (M2) | NVIDIA via NVML, AMD/Intel via platform APIs | `system: [gpu]` |
| `battery` (M2) | charge, state | `system: [battery]` |

Brand integrations (Spotify, Apple Music, YouTube Music) are widgets that use `net` with
OAuth handled by the core (tokens kept in the OS keychain, never visible to widget
code). Spotify uses "bring your own Client ID" because Spotify limits unapproved apps
to 5 users.

Install shows the permission list. An update that asks for more permissions is held
until the user approves it.

## 7. Storage and files

Per-OS app directories (identifier `com.widgetforge.app`):

| File | Content |
|---|---|
| `config/settings.json` | app settings (startup, background start, theme, hotkey) |
| `config/layout.json` | instances: id, widget id, monitor, x, y, w, h, layer, mode, start_with_app, taskbar, opacity, locked, setting values |
| `data/addons/<id>/` | user-installed widgets |
| `data/storage/<instance>.json` | per-instance storage |
| `data/models/` | assistant model files (optional download) |

Writes are atomic (write temp file, then rename). Corrupt files are renamed `*.bad` and
replaced with defaults instead of crashing.

## 8. Distribution

One package format, three sources, all ending up as folders the core reads:

1. **Local add-ons folder** — drag a `.forgewidget` onto the Hub, double-click it, or drop
   a folder into `addons/`. Works offline. Hub has "Open add-ons folder" and "Reload".
2. **GitHub registry** — a public `WidgetForge/widgets` repo. Authors open a pull request
   adding their folder. CI validates the manifest, lints, lists permissions, and builds
   `index.json`. The Hub's "Browse" tab reads that index and downloads releases.
3. **Steam Workshop** — upload/download through Steamworks UGC when Steam is running.
   Downloaded items are plain folders, so the core can load them even when Steam is
   closed; only updates need Steam.

Import/export:

- **Widget** — export a widget folder to `.forgewidget`.
- **Desk profile** — export the whole layout (instances, positions, settings, but never
  secrets or tokens) as `.forgedesk`, so people can share full setups.

## 9. Hub (main menu)

Sections:

- **Desktop** — the instances you have: show/hide, layer, shape, start-with-app, taskbar,
  opacity, lock, settings form, remove.
- **Library** — every installed widget with icon, description and permissions; "Add to
  desktop".
- **Add-ons** — import/export, open folder, reload, source of each widget (built-in,
  local, GitHub, Workshop).
- **Create** — the builder and the assistant (M3).
- **Settings** — run on startup, start in background, theme, global show/hide hotkey,
  hide widgets while a fullscreen app is focused.

## 10. Builder and guide (M3)

- Visual editor: canvas with blocks, property panel, live preview with sample data,
  YAML/code view side by side. Saves to the add-ons folder; "Publish" exports or uploads.
- `forge dev <folder>` hot-reloads a widget while editing it in any editor.
- Guide site: a five-minute first widget, block and source reference, permissions,
  publishing, and annotated copies of every built-in widget.

## 11. Forge Assistant (widget-building AI)

Purpose: turn "a red gauge for my GPU temperature on the right edge" into a working
manifest, and edit existing ones on request. It is used for a moment, then unloaded.

Design:

- **Output is constrained to the manifest JSON schema** (llama.cpp grammar / JSON-schema
  mode, or `response_format` on cloud APIs). This is what makes a 2–4B model reliable:
  it cannot produce invalid structure, only questionable content.
- The assistant writes **declarative manifests only** (blocks + sources). It does not
  write JavaScript, so its output is always safe to preview.
- The core validates the result, shows a live preview, and the user accepts or asks for
  changes. Validation errors are fed back once for a self-repair attempt.
- Providers:
  - **Local (default)**: llama.cpp `llama-server` sidecar, downloaded on first use with
    a small GGUF model (Q4, ~2–3 GB). Started on demand, unloaded after 2 minutes idle.
    Runs on CPU, or offloads to a GPU with ≥ 4 GB VRAM. Fits on a 6 GB GPU.
  - **Ollama / LM Studio**: any local OpenAI-compatible endpoint.
  - **Cloud**: any OpenAI-compatible API key (OpenRouter, OpenAI, …).
- Model choice is decided by the benchmark in `tools/agent-bench` (see its README):
  the same prompts, schema-constrained, scored on validity and on whether the widget
  matches the request.
- Packaging: the model ships as an optional download (or a free Steam DLC depot) so the
  base install stays small. Check the model's licence before redistribution.

## 12. Platforms

| | Windows (primary) | macOS | Linux |
|---|---|---|---|
| Webview | WebView2 (on Win10/11) | WKWebView | WebKitGTK |
| Transparent windows | yes | yes | needs a compositor |
| Desktop layer | yes | yes | X11 yes; Wayland depends on compositor |
| Positioning | yes | yes | X11 yes; Wayland: KDE via layer-shell, GNOME not possible |
| Media source | SMTC | restricted; adapter needed | MPRIS |
| Startup | registry Run key | Login item | XDG autostart |

## 13. Steam

- No DRM wrapper; Steamworks is optional at runtime. If Steam is not running, Workshop
  features are disabled and everything else works.
- Workshop items are read from the Steam workshop content folder.
- Builds: GitHub Actions produces Windows (MSI/NSIS), macOS (DMG) and Linux (AppImage)
  artifacts; the Steam depot uses the same binaries.

## 14. Launch widget set (24)

| Area | Widgets |
|---|---|
| Time | Clock & world clock, Weather, Calendar, Agenda, Timer/stopwatch/alarm, Pomodoro |
| Productivity | To-do, Sticky notes, Quick launcher, Folder portal, Clipboard history, Calculator + converters |
| System | System monitor, Network speed + ping, Disk space, Battery, Per-app volume mixer, Power & quick actions |
| Media | Universal media player, Spotify, Audio visualizer, Slideshow / photo frame |
| Other | LLM chat, RSS/news reader |

## 15. Milestones

| | Scope |
|---|---|
| **M1 — Core** (this branch) | Registry, layout, widget windows (tile/drawer, layers), tray, autostart, single instance, Hub, blocks renderer, HTML widget sandbox + SDK, system source, storage, import/export. Widgets: Clock, Sticky Notes, System Monitor. Assistant benchmark. |
| M2 — Media & system | Media source (SMTC/MPRIS), GPU, battery, net fetch with OAuth, Weather, Media Player, Spotify, fullscreen auto-hide, per-widget AppUserModelID. |
| M3 — Create | Visual builder, `forge dev`, guide site, Forge Assistant in the Hub. |
| M4 — Share | GitHub registry + Browse tab, Steam Workshop, desk profiles. |
| M5 — Launch set | Remaining launch widgets, polish, Steam page, installers. |

## 16. Open questions

- Pricing and whether the assistant model ships as DLC or an in-app download.
- Whether to sign widgets from the GitHub registry (publisher keys) in M4.
- Compositor-mode fallback if per-window memory is too high on Windows.
