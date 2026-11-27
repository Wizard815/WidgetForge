<script>
  // The Hub (main menu): your desktop, the library, add-ons.
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { api, widgetFile } from "../lib/api.js";
  import SettingsForm from "./SettingsForm.svelte";
  import Credits from "./Credits.svelte";
  import Presets from "./Presets.svelte";
  import Discord from "./Discord.svelte";
  import AppPrefs from "./AppPrefs.svelte";
  import Builder from "./Builder.svelte";
  import ThemePage from "./ThemePage.svelte";
  import Packages from "./Packages.svelte";
  import Browse from "./Browse.svelte";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { PRESETS } from "../lib/theme.js";

  const SECTIONS = [
    { id: "desktop", name: "Desktop" },
    { id: "library", name: "Library" },
    { id: "browse", name: "Browse" },
    { id: "presets", name: "Presets" },
    { id: "builder", name: "Builder" },
    { id: "theme", name: "Theme" },
    { id: "addons", name: "Add-ons" },
    { id: "settings", name: "Settings" },
    { id: "credits", name: "Credits" },
  ];
  const LAYERS = [
    { id: "desktop", name: "On desktop" },
    { id: "normal", name: "Normal" },
    { id: "top", name: "Always on top" },
  ];

  let section = $state("desktop");
  let widgets = $state([]);
  let errors = $state([]);
  let instances = $state([]);
  let expanded = $state(null);
  let paths = $state({});
  let message = $state("");
  let pendingImport = $state(null);
  let exported = $state(null); // { id, path }

  function offerImport(path) {
    section = "addons";
    pendingImport = path;
  }
  async function exportWidget(w) {
    try {
      message = "";
      exported = { id: w.id, path: await api.exportWidget(w.id) };
    } catch (e) {
      message = String(e);
    }
  }

  let byId = $derived(Object.fromEntries(widgets.map((w) => [w.id, w])));
  let stacks = $derived(instances.filter((i) => byId[i.widget_id]?.manifest.ui?.stack));
  // Stacks first with their members under them, then the rest.
  let ordered = $derived.by(() => {
    const out = [];
    for (const i of instances) {
      if (i.parent) continue;
      out.push(i);
      for (const c of instances) if (c.parent === i.id) out.push(c);
    }
    for (const c of instances) if (c.parent && !instances.some((i) => i.id === c.parent)) out.push(c);
    return out;
  });

  async function refresh() {
    const r = await api.listWidgets();
    widgets = r.widgets;
    errors = r.errors;
    instances = await api.listInstances();
  }

  async function act(fn) {
    try {
      message = "";
      await fn();
    } catch (e) {
      message = String(e);
    }
    await refresh();
  }

  onMount(() => {
    refresh();
    api.pendingImports().then((list) => list?.length && offerImport(list[list.length - 1])).catch(() => {});
    api.appPaths().then((p) => {
      paths = p;
      if (p.start_section && SECTIONS.some((s) => s.id === p.start_section)) section = p.start_section;
    });
    const stops = [
      listen("layout-changed", refresh),
      // A .forgewidget double-clicked while WidgetForge runs.
      listen("import-package", (e) => offerImport(e.payload)),
      // A .forgewidget dropped onto the Hub.
      getCurrentWebview().onDragDropEvent((e) => {
        if (e.payload.type !== "drop") return;
        const pkg = (e.payload.paths ?? []).find((p) => p.toLowerCase().endsWith(".forgewidget"));
        if (pkg) offerImport(pkg);
        else if (e.payload.paths?.length) message = "Only .forgewidget files can be dropped here.";
      }),
      listen("focus-instance", (e) => {
        section = "desktop";
        expanded = e.payload;
      }),
    ];
    return () => stops.forEach((p) => p.then((f) => f()));
  });

  function permissionsOf(m) {
    const p = m.permissions ?? {};
    const out = [];
    for (const s of p.system ?? []) out.push(`system: ${s}`);
    for (const h of p.network ?? []) out.push(`network: ${h}`);
    if (p.storage) out.push("storage");
    return out;
  }

  function titleOf(inst) {
    const w = byId[inst.widget_id];
    const label = inst.settings?.label || inst.settings?.title;
    return label ? `${w?.manifest.name ?? inst.widget_id} · ${label}` : (w?.manifest.name ?? inst.widget_id);
  }
</script>

<div class="app">
  <nav>
    <div class="brand">WidgetForge</div>
    {#each SECTIONS as s}
      <button class:active={section === s.id} onclick={() => (section = s.id)}>{s.name}</button>
    {/each}
  </nav>

  <main>
    {#if message}<div class="message">{message}</div>{/if}

    {#if section === "desktop"}
      <header>
        <h1>Desktop</h1>
        <p>The widgets you have placed. Hover a widget for its toolbar; drag it anywhere.</p>
      </header>
      {#if instances.length === 0}
        <div class="empty">
          No widgets yet. <button class="link" onclick={() => (section = "library")}>Open the library</button>
        </div>
      {/if}
      <div class="list">
        {#each ordered as inst (inst.id)}
          {@const w = byId[inst.widget_id]}
          {@const isStack = !!w?.manifest.ui?.stack}
          <div class="row" class:hidden={!inst.open && !inst.parent} class:child={!!inst.parent}>
            <div class="line">
              {#if w?.manifest.icon}<img class="icon" src={widgetFile(w.id, w.manifest.icon)} alt="" />{/if}
              <div class="name">
                <strong>{titleOf(inst)}</strong>
                <small>
                  {inst.parent ? "In a stack" : inst.open ? "Showing" : "Hidden"}{w ? "" : " · widget missing"}
                </small>
              </div>
              {#if !isStack && stacks.length}
                <select
                  title="Show this widget inside a stack"
                  value={inst.parent ?? ""}
                  onchange={(e) => act(() => api.moveToStack(inst.id, e.currentTarget.value))}
                >
                  <option value="">Own window</option>
                  {#each stacks as st}<option value={st.id}>Stack: {titleOf(st)}</option>{/each}
                </select>
              {/if}
              {#if !inst.parent && !isStack}
                <!-- Shape: a floating tile, or a drawer on a screen edge. PLACEHOLDER wording. -->
                <select
                  title="Shape"
                  value={inst.mode === "drawer" ? `drawer:${inst.edge ?? "right"}` : "tile"}
                  onchange={(e) => {
                    const v = e.currentTarget.value;
                    const patch = v === "tile" ? { mode: "tile" } : { mode: "drawer", edge: v.split(":")[1], layer: "top" };
                    act(() => api.updateInstance(inst.id, patch));
                  }}
                >
                  <option value="tile">Floating</option>
                  <option value="drawer:left">Drawer, left edge</option>
                  <option value="drawer:right">Drawer, right edge</option>
                  <option value="drawer:top">Drawer, top edge</option>
                  <option value="drawer:bottom">Drawer, bottom edge</option>
                </select>
              {/if}
              {#if !inst.parent}
              <select
                value={inst.layer}
                onchange={(e) => act(() => api.updateInstance(inst.id, { layer: e.currentTarget.value }))}
              >
                {#each LAYERS as l}<option value={l.id}>{l.name}</option>{/each}
              </select>
              <label class="check" title="Open this widget when WidgetForge starts">
                <input
                  type="checkbox"
                  checked={inst.start_with_app}
                  onchange={(e) => act(() => api.updateInstance(inst.id, { start_with_app: e.currentTarget.checked }))}
                />
                Start with app
              </label>
              <label class="check">
                <input
                  type="checkbox"
                  checked={inst.locked}
                  onchange={(e) => act(() => api.updateInstance(inst.id, { locked: e.currentTarget.checked }))}
                />
                Lock
              </label>
              <button onclick={() => act(() => api.showInstance(inst.id, !inst.open))}>
                {inst.open ? "Hide" : "Show"}
              </button>
              {/if}
              <button onclick={() => (expanded = expanded === inst.id ? null : inst.id)}>Settings</button>
              <button class="danger" onclick={() => act(() => api.removeInstance(inst.id))}>Remove</button>
            </div>
            {#if expanded === inst.id && w}
              <div class="settings">
                <!-- This widget's own look, over the app theme. PLACEHOLDER wording. -->
                <div class="look">
                  <strong>Look</strong>
                  <label>
                    Frame
                    <select value={inst.style?.frame ?? "theme"} onchange={(e) => act(() => api.updateInstance(inst.id, { style: { frame: e.currentTarget.value === "theme" ? null : e.currentTarget.value } }))}>
                      <option value="theme">Same as the theme</option>
                      {#each Object.entries(PRESETS) as [id, p]}<option value={id}>{p.name}</option>{/each}
                    </select>
                  </label>
                  <label>
                    See-through
                    <input type="range" min="0" max="1" step="0.05" value={1 - (inst.style?.opacity ?? 0.78)} onchange={(e) => act(() => api.updateInstance(inst.id, { style: { opacity: +(1 - e.currentTarget.value).toFixed(2) } }))} />
                  </label>
                  <label>
                    Accent
                    <input type="color" value={inst.style?.accent ?? w.manifest.style?.accent ?? "#7cc4ff"} onchange={(e) => act(() => api.updateInstance(inst.id, { style: { accent: e.currentTarget.value } }))} />
                  </label>
                  {#if inst.style && Object.keys(inst.style).length}
                    <button class="link" onclick={() => act(() => api.updateInstance(inst.id, { style: { frame: null, opacity: null, accent: null } }))}>Use the theme</button>
                  {/if}
                </div>
                <SettingsForm
                  manifest={w.manifest}
                  instance={inst}
                  onchange={(patch) => act(() => api.updateInstance(inst.id, { settings: patch }))}
                  onsecret={(key, value) => act(() => api.setSecret(inst.id, key, value))}
                />
              </div>
            {/if}
          </div>
        {/each}
      </div>
    {:else if section === "library"}
      <header>
        <h1>Library</h1>
        <p>Every installed widget. Add one to place it on your desktop.</p>
      </header>
      <div class="cards">
        {#each widgets as w (w.id)}
          <div class="card" style="--accent: {w.manifest.style?.accent ?? '#7cc4ff'}">
            <div class="card-top">
              {#if w.manifest.icon}<img class="icon big" src={widgetFile(w.id, w.manifest.icon)} alt="" />{/if}
              <div>
                <strong>{w.manifest.name}</strong>
                <small>{w.manifest.category ?? "other"} · v{w.manifest.version}</small>
              </div>
            </div>
            <p>{w.manifest.description ?? ""}</p>
            <div class="chips">
              {#each permissionsOf(w.manifest) as p}<span class="chip">{p}</span>{/each}
            </div>
            <div class="card-buttons">
              <button class="primary" onclick={() => act(() => api.addInstance(w.id))}>Add to desktop</button>
              <button title="Save as a .forgewidget file to share" onclick={() => exportWidget(w)}>Export</button>
            </div>
            {#if exported?.id === w.id}
              <small class="exported">Saved to {exported.path} <button class="link" onclick={() => api.revealFile(exported.path)}>Show</button></small>
            {/if}
          </div>
        {/each}
      </div>
    {:else if section === "addons"}
      <header>
        <h1>Add-ons</h1>
        <p>Drop a widget folder (with a <code>widget.yaml</code>) into the add-ons folder, then reload.</p>
      </header>
      <div class="buttons">
        <button onclick={() => act(() => api.openAddonsFolder())}>Open add-ons folder</button>
        <button onclick={() => act(() => api.reloadWidgets())}>Reload</button>
      </div>
      <p class="path"><code>{paths.addons}</code></p>
      <Packages bind:pending={pendingImport} oninstalled={() => refresh()} />
      {#if errors.length}
        <h2>Problems</h2>
        <ul class="errors">{#each errors as e}<li>{e}</li>{/each}</ul>
      {/if}
      <h2>Installed</h2>
      <table>
        <thead><tr><th>Widget</th><th>Id</th><th>Version</th><th>Source</th><th></th></tr></thead>
        <tbody>
          {#each widgets as w (w.id)}
            <tr>
              <td>{w.manifest.name}</td><td><code>{w.id}</code></td><td>{w.manifest.version}</td><td>{w.source}</td>
              <td><button onclick={() => exportWidget(w)}>Export</button></td>
            </tr>
            {#if exported?.id === w.id}
              <tr><td colspan="5" class="exported">Saved to <code>{exported.path}</code> <button class="link" onclick={() => api.revealFile(exported.path)}>Show in folder</button></td></tr>
            {/if}
          {/each}
        </tbody>
      </table>
    {:else if section === "presets"}
      <Presets />
    {:else if section === "browse"}
      <Browse {widgets} oninstalled={refresh} />
    {:else if section === "builder"}
      <Builder {widgets} onsaved={refresh} />
    {:else if section === "theme"}
      <ThemePage />
    {:else if section === "settings"}
      <header>
        <h1>Settings</h1>
        <p>How WidgetForge starts, its shortcuts, and where it keeps your things.</p>
      </header>
      <AppPrefs />
      <h2>Where things are kept</h2>
      <table>
        <tbody>
          <tr><td>API keys and tokens</td><td>Saved in {paths.key_storage}. Widget code never sees them.</td></tr>
          <tr><td>Desktop layout</td><td><code>{paths.layout}</code></td></tr>
          <tr><td>Notes, to-dos, timers</td><td><code>{paths.storage}</code></td></tr>
          <tr><td>Presets</td><td><code>{paths.presets}</code></td></tr>
          <tr><td>Add-on widgets</td><td><code>{paths.addons}</code></td></tr>
        </tbody>
      </table>
      <Discord clientId={paths.discord_client_id} onsaved={() => api.appPaths().then((p) => (paths = p))} />
    {:else if section === "credits"}
      <Credits />
    {/if}
  </main>
</div>

<style>
  :global(html, body) { margin: 0; height: 100%; }
  :global(body) {
    color-scheme: dark;
    background: #0f131c;
    color: #e8ecf3;
    font: 14px/1.45 system-ui, "Segoe UI", sans-serif;
  }
  :global(button), :global(select), :global(input) { font: inherit; color: inherit; }
  .app { display: flex; height: 100vh; }
  nav {
    width: 190px;
    flex: none;
    padding: 18px 10px;
    background: #151a25;
    border-right: 1px solid #222a3a;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .brand { font-weight: 700; font-size: 16px; padding: 0 10px 14px; }
  nav button {
    text-align: left;
    padding: 8px 10px;
    border: 0;
    border-radius: 8px;
    background: none;
    cursor: pointer;
  }
  nav button:hover { background: #1e2533; }
  nav button.active { background: #26304a; }
  main { flex: 1; overflow: auto; padding: 24px 28px; }
  header h1 { margin: 0 0 4px; font-size: 22px; }
  header p { margin: 0 0 18px; opacity: 0.65; }
  h2 { font-size: 15px; margin: 22px 0 8px; }
  button {
    padding: 6px 12px;
    border: 1px solid #2c3548;
    border-radius: 8px;
    background: #1b2230;
    cursor: pointer;
  }
  button:hover { background: #242d3f; }
  button.primary { background: #2a4f8a; border-color: #3765ad; }
  button.danger:hover { background: #5a2430; border-color: #7a3040; }
  button.link { border: 0; background: none; color: #7cc4ff; padding: 0; text-decoration: underline; }
  select { padding: 5px 8px; border: 1px solid #2c3548; border-radius: 8px; background: #1b2230; }
  .message { padding: 10px 12px; margin-bottom: 14px; border-radius: 8px; background: #4a1f28; }
  .empty { opacity: 0.8; padding: 20px 0; }
  .list { display: flex; flex-direction: column; gap: 8px; }
  .row { border: 1px solid #222a3a; border-radius: 10px; background: #141925; }
  .row.hidden { opacity: 0.6; }
  .row.child { margin-left: 28px; border-left: 3px solid #3a4870; }
  .line { display: flex; align-items: center; gap: 8px; padding: 10px 12px; flex-wrap: wrap; }
  .name { flex: 1; min-width: 120px; display: flex; flex-direction: column; }
  .name small, .card small { opacity: 0.6; }
  .icon { width: 28px; height: 28px; }
  .icon.big { width: 40px; height: 40px; }
  .check { display: flex; align-items: center; gap: 5px; font-size: 13px; }
  .settings { padding: 12px 14px 14px; border-top: 1px solid #222a3a; }
  .look { display: flex; gap: 16px; align-items: center; flex-wrap: wrap; font-size: 13px; padding-bottom: 12px; margin-bottom: 12px; border-bottom: 1px solid #222a3a; }
  .look label { display: flex; gap: 6px; align-items: center; }
  .look input[type="color"] { width: 40px; height: 24px; border: 0; background: none; padding: 0; }
  .cards { display: grid; grid-template-columns: repeat(auto-fill, minmax(240px, 1fr)); gap: 12px; }
  .card {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 14px;
    border: 1px solid #222a3a;
    border-top: 3px solid var(--accent);
    border-radius: 10px;
    background: #141925;
  }
  .card-top { display: flex; gap: 10px; align-items: center; }
  .card-top div { display: flex; flex-direction: column; }
  .card p { margin: 0; flex: 1; opacity: 0.8; font-size: 13px; }
  .chips { display: flex; flex-wrap: wrap; gap: 4px; }
  .card-buttons { display: flex; gap: 6px; }
  .card-buttons .primary { flex: 1; }
  .exported { font-size: 12px; opacity: 0.8; overflow-wrap: anywhere; }
  .chip { font-size: 11px; padding: 2px 7px; border-radius: 10px; background: #222a3a; }
  .buttons { display: flex; gap: 8px; }
  .path { opacity: 0.6; font-size: 12px; }
  .errors { color: #ffb4b4; }
  table { border-collapse: collapse; width: 100%; font-size: 13px; }
  th, td { text-align: left; padding: 6px 10px; border-bottom: 1px solid #222a3a; }
  th { opacity: 0.6; font-weight: 600; }
</style>
