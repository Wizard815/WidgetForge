<script lang="ts">
  import { hub, inTauri } from "../../lib/api";
  import type { WidgetInfo } from "../../lib/types";
  import Icon from "../components/Icon.svelte";
  import WidgetIcon from "../components/WidgetIcon.svelte";
  import { act, app, startImport, toast } from "../store.svelte";

  const addons = $derived((app.state?.widgets ?? []).filter((w) => w.origin === "addon"));
  const builtins = $derived((app.state?.widgets ?? []).filter((w) => w.origin === "builtin"));
  const errors = $derived(app.state?.errors ?? []);

  async function pick(directory: boolean) {
    if (!inTauri) return toast("Importing needs the desktop app.", "error");
    const { open } = await import("@tauri-apps/plugin-dialog");
    const path = await open({
      directory,
      multiple: false,
      title: directory ? "Choose a widget folder" : "Choose a widget package",
      filters: directory ? undefined : [{ name: "WidgetForge widget", extensions: ["forgewidget", "zip"] }],
    });
    if (typeof path === "string") await startImport(path);
  }

  async function exportWidget(w: WidgetInfo) {
    if (!inTauri) return toast("Exporting needs the desktop app.", "error");
    const { save } = await import("@tauri-apps/plugin-dialog");
    const dest = await save({
      title: `Export ${w.manifest.name}`,
      defaultPath: `${w.manifest.id}-${w.manifest.version}.forgewidget`,
      filters: [{ name: "WidgetForge widget", extensions: ["forgewidget"] }],
    });
    if (dest) await act(() => hub.exportPackage(w.manifest.id, dest), "Exported");
  }

  async function uninstall(w: WidgetInfo) {
    const msg = `Uninstall ${w.manifest.name}? It will be removed from your desktop too.`;
    let ok = true;
    try {
      const { ask } = await import("@tauri-apps/plugin-dialog");
      ok = await ask(msg, { title: "Uninstall widget", kind: "warning" });
    } catch {
      ok = confirm(msg);
    }
    if (ok) await act(() => hub.uninstall(w.manifest.id), `${w.manifest.name} uninstalled`);
  }
</script>

<div class="page">
  <div class="page-head">
    <div>
      <h1>Add-ons</h1>
      <p>Install widgets other people made, and share your own.</p>
    </div>
    <div class="actions">
      <button class="btn" onclick={() => act(() => hub.openAddonsFolder())}><Icon name="folder" size={16} /> Open folder</button>
      <button class="btn" onclick={() => act(() => hub.reload(), "Widgets reloaded")}><Icon name="refresh" size={16} /> Reload</button>
    </div>
  </div>

  <div class="drop card">
    <div class="drop-icon"><Icon name="upload" size={26} /></div>
    <div class="drop-text">
      <h2>Install a widget</h2>
      <p class="muted">Drop a <span class="mono">.forgewidget</span> file anywhere on this window, or choose one.</p>
    </div>
    <div class="actions">
      <button class="btn primary" onclick={() => pick(false)}>Choose file…</button>
      <button class="btn" onclick={() => pick(true)}>Choose folder…</button>
    </div>
  </div>

  <div class="sources">
    <div class="card source">
      <h3>Add-ons folder</h3>
      <p class="muted small">Widgets in this folder load automatically.</p>
      <p class="mono path" title={app.state?.addons_dir}>{app.state?.addons_dir}</p>
    </div>
    <div class="card source soon">
      <h3>Community registry <span class="chip">Soon</span></h3>
      <p class="muted small">Browse widgets shared on GitHub and install them in one click.</p>
    </div>
    <div class="card source soon">
      <h3>Steam Workshop <span class="chip">Soon</span></h3>
      <p class="muted small">Subscribe on Steam; widgets keep working even when Steam is closed.</p>
    </div>
  </div>

  <div class="section-title">Installed add-ons ({addons.length})</div>
  {#if addons.length === 0}
    <p class="muted small">None yet.</p>
  {:else}
    <div class="rows card">
      {#each addons as w (w.manifest.id)}
        <div class="wrow">
          <WidgetIcon manifest={w.manifest} size={34} />
          <div class="info">
            <div class="name">{w.manifest.name} <span class="faint small">v{w.manifest.version}</span></div>
            <div class="small faint mono">{w.manifest.id}</div>
          </div>
          {#if w.warnings.length}<span class="chip warn" title={w.warnings.map((x) => x.message).join("\n")}>{w.warnings.length} warning(s)</span>{/if}
          <button class="btn small" onclick={() => exportWidget(w)}><Icon name="download" size={14} /> Export</button>
          <button class="btn small danger" onclick={() => uninstall(w)}><Icon name="trash" size={14} /> Uninstall</button>
        </div>
      {/each}
    </div>
  {/if}

  {#if errors.length}
    <div class="section-title">Couldn't load ({errors.length})</div>
    <div class="rows card">
      {#each errors as e (e.folder)}
        <div class="err">
          <div class="name"><Icon name="warning" size={15} /> {e.folder}</div>
          <ul class="small">
            {#each e.issues as i}<li><span class="mono">{i.path || "widget.yaml"}</span> — {i.message}</li>{/each}
          </ul>
        </div>
      {/each}
    </div>
  {/if}

  <div class="section-title">Built-in widgets ({builtins.length})</div>
  <p class="muted small" style="margin-bottom:10px">Export one to use it as a starting point for your own.</p>
  <div class="rows card">
    {#each builtins as w (w.manifest.id)}
      <div class="wrow">
        <WidgetIcon manifest={w.manifest} size={34} />
        <div class="info">
          <div class="name">{w.manifest.name} <span class="faint small">v{w.manifest.version}</span></div>
          <div class="small faint mono">{w.manifest.id}</div>
        </div>
        <button class="btn small" onclick={() => exportWidget(w)}><Icon name="download" size={14} /> Export</button>
      </div>
    {/each}
  </div>
</div>

<style>
  .drop {
    display: flex;
    align-items: center;
    gap: 18px;
    padding: 22px;
    border-style: dashed;
    border-width: 1.5px;
    border-color: var(--border-strong);
    background: linear-gradient(135deg, var(--accent-bg), transparent 70%), var(--panel);
  }
  .drop-icon {
    display: grid;
    place-items: center;
    width: 54px;
    height: 54px;
    border-radius: 16px;
    color: #fff;
    background: linear-gradient(135deg, var(--brand-1), var(--brand-2));
  }
  .drop-text {
    flex: 1;
  }
  .sources {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 12px;
    margin-top: 14px;
  }
  .source {
    padding: 14px 16px;
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }
  .source h3 {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .source.soon {
    opacity: 0.75;
  }
  .path {
    margin-top: 4px;
    font-size: 11.5px;
    color: var(--muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .rows {
    display: flex;
    flex-direction: column;
  }
  .wrow {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 14px;
    border-bottom: 1px solid var(--border);
  }
  .wrow:last-child,
  .err:last-child {
    border-bottom: 0;
  }
  .info {
    flex: 1;
    min-width: 0;
  }
  .name {
    font-weight: 600;
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .err {
    padding: 12px 16px;
    border-bottom: 1px solid var(--border);
  }
  .err .name {
    color: var(--danger);
  }
  .err ul {
    margin: 6px 0 0;
    padding-left: 20px;
    color: var(--muted);
  }
</style>
