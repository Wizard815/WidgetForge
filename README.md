# WidgetForge

Desktop widgets for everyone, in the spirit of Windows 7 gadgets. Clocks, system stats,
sticky notes, to-dos and anything else you can describe in a small YAML file. Built for
Windows first, with macOS and Linux from the same code.

> Status: **milestone 1 (core)**. The runtime, the Hub (main menu), the widget format and
> six built-in widgets work. See [docs/DESIGN.md](docs/DESIGN.md) for the full plan.

## What works now

- **Hub** (main menu): your desktop, the library, add-ons, a create guide, settings.
- **Widgets as their own windows**: frameless and transparent, draggable and resizable, with
  a hover toolbar (close, settings, layer, lock).
- **Layers**: always on top, normal, or pinned to the desktop behind everything.
- **Shapes**: a floating tile, or an edge drawer that hides at a screen edge and slides out.
- **Startup**: optional sign-in launch (Windows registry, macOS login item, XDG autostart).
  It starts in the tray and opens only the widgets marked "Start with WidgetForge", at
  their saved positions. Steam is not needed.
- **Tray icon** with show/hide all, and single-instance behaviour.
- **Widget format**: `widget.yaml` with declarative blocks and live bindings, or an HTML/JS
  entry point running in a sandboxed iframe with a small SDK.
- **Permissions**: system stats, network hosts and storage are declared in the manifest
  and enforced in Rust.
- **Packages**: import and export `.forgewidget` files (zip), or drop folders into the
  add-ons folder. Updates that ask for new permissions are flagged before installing.
- **Built-in widgets**: Clock, System Monitor, Sticky Notes, To-do, Quick Links, and Day
  Progress (an HTML/JS example).

## Run it

You need Node 22+, Rust (stable) and the
[Tauri prerequisites](https://tauri.app/start/prerequisites/) for your OS. On Windows
that is WebView2, which ships with Windows 10/11, and the MSVC build tools.

```bash
npm install
```

```bash
npm run tauri dev
```

Build installers:

```bash
npm run tauri build
```

The GitHub Actions workflow `.github/workflows/build.yml` builds Windows (MSI/NSIS),
macOS (DMG) and Linux (AppImage/deb) installers on demand, for pull requests, and on
pushes to `main`.

Tests:

```bash
npm test && npm run check && (cd src-tauri && cargo test)
```

### UI without the desktop app

`npm run dev` and open <http://localhost:1420/hub.html> in a browser. The Hub runs against
an in-memory mock backend. To preview a single widget, open
`/widget.html?mock=forge.system-monitor`.

## Make a widget

Create a folder in the add-ons folder (Hub → Add-ons → Open folder) with a `widget.yaml`:

```yaml
schema: 1
id: me.cpu-ring
name: CPU Ring
version: 1.0.0
permissions:
  system: [cpu]
data:
  sys: { source: system, interval: 1000 }
ui:
  blocks:
    - type: gauge
      label: CPU
      value: "{{ sys.cpu.usage }}"
      format: "{{ sys.cpu.usage | fixed(0) }}%"
```

Press **Reload** on the Add-ons page. The full format is in
[docs/DESIGN.md §5](docs/DESIGN.md), and every built-in widget in `widgets/` is a
working example.

## Repository layout

| Path | What |
|---|---|
| `src-tauri/src/` | Rust core: manifest validation, registry, layout, windows, tray, system stats, storage, packages, the `widget://` protocol |
| `src/hub/` | Hub UI (Svelte 5) |
| `src/widget/` | Widget host: block renderer, HTML-widget sandbox bridge |
| `src/lib/` | Shared types, IPC wrappers, template bindings, browser mock |
| `src/sdk/forge-sdk.js` | SDK injected into HTML widgets |
| `widgets/` | Built-in widgets |
| `tools/agent-bench/` | Benchmark for choosing the Forge Assistant model |
| `docs/DESIGN.md` | Design document |
