<script lang="ts">
  import { hub } from "../../lib/api";
  import type { WidgetInfo } from "../../lib/types";
  import Icon from "../components/Icon.svelte";
  import WidgetIcon from "../components/WidgetIcon.svelte";
  import { act, app, permissionList } from "../store.svelte";

  const CATEGORIES = [
    ["all", "All"],
    ["time", "Time"],
    ["productivity", "Productivity"],
    ["system", "System"],
    ["media", "Media"],
    ["info", "Info"],
    ["fun", "Fun"],
    ["other", "Other"],
  ] as const;

  let query = $state("");
  let category = $state<string>("all");

  const widgets = $derived(
    (app.state?.widgets ?? []).filter((w) => {
      const m = w.manifest;
      if (category !== "all" && m.category !== category) return false;
      const q = query.trim().toLowerCase();
      return !q || `${m.name} ${m.description ?? ""} ${m.author ?? ""}`.toLowerCase().includes(q);
    }),
  );
  const present = $derived(new Set((app.state?.widgets ?? []).map((w) => w.manifest.category)));

  function canAdd(w: WidgetInfo) {
    return w.manifest.window.multi_instance || w.instances === 0;
  }

  async function add(w: WidgetInfo) {
    await act(() => hub.addInstance(w.manifest.id), `${w.manifest.name} added to your desktop`);
  }
</script>

<div class="page">
  <div class="page-head">
    <div>
      <h1>Library</h1>
      <p>Every widget you have installed. Add one and it appears on your desktop right away.</p>
    </div>
    <label class="search">
      <Icon name="search" size={16} />
      <input type="search" placeholder="Search widgets" bind:value={query} />
    </label>
  </div>

  <div class="cats">
    {#each CATEGORIES as [key, label] (key)}
      {#if key === "all" || present.has(key)}
        <button class="cat" class:active={category === key} onclick={() => (category = key)}>{label}</button>
      {/if}
    {/each}
  </div>

  {#if widgets.length === 0}
    <p class="muted">No widgets match.</p>
  {/if}

  <div class="grid">
    {#each widgets as w (w.manifest.id)}
      {@const m = w.manifest}
      <article class="card wcard">
        <div class="top">
          <WidgetIcon manifest={m} size={48} />
          <div class="meta">
            <h2>{m.name}</h2>
            <div class="small faint">
              {m.author ?? "Unknown author"} · v{m.version}
            </div>
          </div>
          {#if w.origin === "addon"}<span class="chip accent">Add-on</span>{/if}
        </div>
        <p class="desc muted">{m.description ?? ""}</p>
        <div class="chips">
          <span class="chip">{m.window.mode === "drawer" ? "Drawer" : "Tile"}</span>
          {#if m.ui.entry}<span class="chip">HTML</span>{/if}
          {#each permissionList(m) as p (p.key)}
            <span class="chip" title={p.text}>{p.text}</span>
          {/each}
        </div>
        <div class="foot">
          {#if w.instances > 0}<span class="small faint">{w.instances} on your desktop</span>{:else}<span></span>{/if}
          <button class="btn small" class:primary={canAdd(w)} disabled={!canAdd(w)} onclick={() => add(w)}>
            {#if canAdd(w)}<Icon name="plus" size={14} />{w.instances > 0 ? "Add another" : "Add to desktop"}{:else}Added{/if}
          </button>
        </div>
      </article>
    {/each}
  </div>
</div>

<style>
  .search {
    display: flex;
    align-items: center;
    gap: 8px;
    padding-left: 10px;
    border-radius: 10px;
    border: 1px solid var(--border-strong);
    background: var(--panel-2);
    color: var(--muted);
  }
  .search input {
    border: 0;
    background: transparent;
    width: 200px;
    box-shadow: none;
  }
  .cats {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
    margin-bottom: 18px;
  }
  .cat {
    height: 30px;
    padding: 0 14px;
    border-radius: 999px;
    border: 1px solid var(--border);
    background: var(--panel);
    color: var(--muted);
    font-weight: 600;
    font-size: 12.5px;
    cursor: pointer;
  }
  .cat:hover {
    color: var(--text);
  }
  .cat.active {
    background: var(--text);
    color: var(--bg);
    border-color: var(--text);
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
    gap: 14px;
  }
  .wcard {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 16px;
    transition:
      border-color 0.15s,
      transform 0.15s;
  }
  .wcard:hover {
    border-color: var(--border-strong);
    transform: translateY(-1px);
  }
  .top {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .meta {
    flex: 1;
    min-width: 0;
  }
  .desc {
    flex: 1;
    font-size: 13px;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
  }
  .foot {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
</style>
