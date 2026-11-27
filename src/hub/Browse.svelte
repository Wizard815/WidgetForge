<script>
  // Hub → Browse: widgets from the community catalog. Install shows the same
  // screen as importing a file, after the download is checked against the catalog.
  // PLACEHOLDER wording throughout.
  import { onMount } from "svelte";
  import { api } from "../lib/api.js";
  import { compareVersions, permissionChips } from "../lib/versions.js";
  import PackageCard from "./PackageCard.svelte";

  let { widgets = [], oninstalled } = $props();

  let catalog = $state(null);
  let loading = $state(false);
  let error = $state("");
  let query = $state("");
  let category = $state("");
  let onlyUpdates = $state(false);
  let busyId = $state(null);
  let card = $state(null); // install screen for a downloaded package
  let message = $state("");
  let editing = $state(false);
  let address = $state("");

  let installed = $derived(Object.fromEntries(widgets.map((w) => [w.id, w])));
  let categories = $derived([...new Set((catalog?.widgets ?? []).map((w) => w.category))].sort());
  const stateOf = (w) => {
    const mine = installed[w.id];
    if (!mine) return "new";
    return compareVersions(w.version, mine.manifest.version) > 0 ? "update" : "installed";
  };
  let updates = $derived((catalog?.widgets ?? []).filter((w) => stateOf(w) === "update").length);
  let shown = $derived.by(() => {
    const q = query.trim().toLowerCase();
    return (catalog?.widgets ?? []).filter(
      (w) =>
        (!category || w.category === category) &&
        (!onlyUpdates || stateOf(w) === "update") &&
        (!q || `${w.name} ${w.description} ${w.author} ${w.id}`.toLowerCase().includes(q)),
    );
  });

  async function load() {
    loading = true;
    error = "";
    try {
      catalog = await api.registryIndex();
      address = catalog.url;
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }
  onMount(load);

  async function saveAddress() {
    try {
      address = await api.setRegistryUrl(address);
      editing = false;
      await load();
    } catch (e) {
      error = String(e);
    }
  }

  async function get(w) {
    busyId = w.id;
    message = "";
    try {
      const path = await api.registryDownload(w.download, w.sha256, w.size ?? null);
      card = { ...(await api.inspectPackage(path)), _entry: w };
    } catch (e) {
      message = `${w.name}: ${e}`;
    } finally {
      busyId = null;
    }
  }
  const host = (u) => {
    try {
      return new URL(u).host;
    } catch {
      return u;
    }
  };
  const kb = (n) => (n > 1048576 ? `${(n / 1048576).toFixed(1)} MB` : `${Math.max(1, Math.round(n / 1024))} KB`);
</script>

<header class="top">
  <div>
    <h1>Browse</h1>
    <p>Widgets other people made and shared. Each one is checked before it is listed, and again before it installs.</p>
  </div>
  <button onclick={load} disabled={loading}>{loading ? "Loading…" : "Refresh"}</button>
</header>

{#if message}<div class="message">{message}</div>{/if}

{#if card}
  <PackageCard
    info={card}
    source={host(catalog?.url ?? "")}
    oninstalled={() => { message = `Installed ${card.name}. Find it in the Library.`; card = null; oninstalled?.(); }}
    oncancel={() => (card = null)}
  />
{/if}

{#if error}
  <div class="message bad">{error}</div>
{/if}

{#if catalog && !catalog.configured}
  <div class="empty">
    <p><strong>No catalog is set up yet.</strong> When the community catalog is published, its address goes here.</p>
    <p class="muted">You can also use your own: a folder made with the registry template's <code>build_index.py</code>, served over https (or http on this computer).</p>
  </div>
{/if}

{#if catalog?.widgets?.length}
  <div class="filters">
    <input placeholder="Search widgets" bind:value={query} />
    <select bind:value={category}>
      <option value="">All kinds</option>
      {#each categories as c}<option value={c}>{c}</option>{/each}
    </select>
    {#if updates}
      <label class="check"><input type="checkbox" bind:checked={onlyUpdates} /> Updates only ({updates})</label>
    {/if}
  </div>
  <div class="cards">
    {#each shown as w (w.id)}
      {@const st = stateOf(w)}
      <div class="card">
        <div class="card-top">
          {#if w.icon}<img class="icon" src={w.icon} alt="" />{:else}<div class="icon letter">{w.name.charAt(0)}</div>{/if}
          <div>
            <strong>{w.name}</strong>
            <small>{w.author ? `by ${w.author} · ` : ""}v{w.version}{w.size ? ` · ${kb(w.size)}` : ""}</small>
          </div>
        </div>
        <p>{w.description}</p>
        <div class="chips">
          {#each permissionChips(w.permissions) as c}<span class="chip">{c}</span>{/each}
          {#if w.html}<span class="chip">own web page</span>{/if}
        </div>
        <div class="row">
          {#if st === "installed"}
            <span class="have">Installed</span>
          {:else}
            <button class="primary" disabled={busyId === w.id} onclick={() => get(w)}>
              {busyId === w.id ? "Downloading…" : st === "update" ? `Update from v${installed[w.id].manifest.version}` : "Install"}
            </button>
          {/if}
          {#if w.homepage}<button class="link" onclick={() => api.openLink(w.homepage)}>Home page</button>{/if}
        </div>
      </div>
    {:else}
      <p class="muted">Nothing matches.</p>
    {/each}
  </div>
{:else if catalog?.configured && !loading && !error}
  <p class="muted">The catalog is empty.</p>
{/if}

<details class="source" bind:open={editing}>
  <summary>Catalog address</summary>
  <form onsubmit={(e) => { e.preventDefault(); saveAddress(); }}>
    <input bind:value={address} placeholder="https://…/index.json" spellcheck="false" />
    <button>Use this catalog</button>
  </form>
  <p class="muted">Leave empty for the default. Only https addresses (or http on this computer).</p>
</details>

<style>
  .top { display: flex; justify-content: space-between; align-items: flex-start; gap: 16px; }
  .message { padding: 10px 12px; margin-bottom: 12px; border-radius: 8px; background: #1f3350; }
  .message.bad { background: #4a1f28; }
  .empty { padding: 14px; border: 1px dashed #3a4870; border-radius: 10px; margin-bottom: 14px; }
  .empty p { margin: 4px 0; }
  .muted { opacity: 0.6; font-size: 13px; }
  .filters { display: flex; gap: 8px; align-items: center; margin-bottom: 12px; flex-wrap: wrap; }
  .filters input:not([type="checkbox"]) { flex: 1; min-width: 200px; padding: 6px 10px; border: 1px solid #2c3548; border-radius: 8px; background: #0f131c; }
  .check { display: flex; gap: 5px; align-items: center; font-size: 13px; }
  .cards { display: grid; grid-template-columns: repeat(auto-fill, minmax(250px, 1fr)); gap: 12px; }
  .card { display: flex; flex-direction: column; gap: 8px; padding: 14px; border: 1px solid #222a3a; border-radius: 10px; background: #141925; }
  .card-top { display: flex; gap: 10px; align-items: center; }
  .card-top div { display: flex; flex-direction: column; }
  .card small { opacity: 0.6; }
  .card p { margin: 0; flex: 1; opacity: 0.8; font-size: 13px; }
  .icon { width: 40px; height: 40px; border-radius: 8px; flex: none; }
  .letter { display: grid; place-items: center; background: #26304a; font-weight: 700; font-size: 18px; }
  .chips { display: flex; flex-wrap: wrap; gap: 4px; }
  .chip { font-size: 11px; padding: 2px 7px; border-radius: 10px; background: #222a3a; }
  .row { display: flex; gap: 8px; align-items: center; }
  .row .primary { flex: 1; }
  .have { flex: 1; font-size: 13px; color: #6cf0a0; }
  .source { margin-top: 22px; font-size: 13px; }
  .source form { display: flex; gap: 8px; margin-top: 8px; }
  .source input { flex: 1; padding: 6px 8px; border: 1px solid #2c3548; border-radius: 8px; background: #0f131c; font-family: ui-monospace, monospace; font-size: 12px; }
</style>
