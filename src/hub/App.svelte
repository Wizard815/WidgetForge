<script lang="ts">
  import { hub } from "../lib/api";
  import Icon from "./components/Icon.svelte";
  import ImportDialog from "./components/ImportDialog.svelte";
  import Addons from "./pages/Addons.svelte";
  import Create from "./pages/Create.svelte";
  import Desktop from "./pages/Desktop.svelte";
  import Library from "./pages/Library.svelte";
  import Settings from "./pages/Settings.svelte";
  import { act, app, type Page } from "./store.svelte";

  const NAV: { page: Page; label: string; icon: string }[] = [
    { page: "desktop", label: "My desktop", icon: "desktop" },
    { page: "library", label: "Library", icon: "library" },
    { page: "addons", label: "Add-ons", icon: "addons" },
    { page: "create", label: "Create", icon: "create" },
    { page: "settings", label: "Settings", icon: "settings" },
  ];

  let hidden = $state(false);
  const openCount = $derived(app.state?.instances.filter((i) => i.open).length ?? 0);

  async function toggleHidden() {
    hidden = !hidden;
    await act(() => hub.setHidden(hidden));
  }
</script>

<div class="shell">
  <aside class="sidebar">
    <div class="brand">
      <img src="/logo.svg" alt="" width="34" height="34" />
      <div>
        <div class="name">WidgetForge</div>
        <div class="tag">Desktop widgets</div>
      </div>
    </div>

    <nav>
      {#each NAV as n (n.page)}
        <button class:active={app.page === n.page} onclick={() => (app.page = n.page)}>
          <Icon name={n.icon} />
          <span>{n.label}</span>
          {#if n.page === "desktop" && app.state}
            <span class="count">{app.state.instances.length}</span>
          {/if}
        </button>
      {/each}
    </nav>

    <div class="foot">
      <button class="hide" onclick={toggleHidden} title="Hide or show every widget">
        <Icon name={hidden ? "eye" : "eye-off"} size={16} />
        {hidden ? "Show widgets" : "Hide widgets"}
      </button>
      <div class="status">
        <span class="dot" class:on={openCount > 0 && !hidden}></span>
        {openCount} on screen
      </div>
    </div>
  </aside>

  <main>
    {#if !app.state}
      <div class="loading">Loading…</div>
    {:else if app.page === "desktop"}
      <Desktop />
    {:else if app.page === "library"}
      <Library />
    {:else if app.page === "addons"}
      <Addons />
    {:else if app.page === "create"}
      <Create />
    {:else}
      <Settings />
    {/if}
  </main>
</div>

{#if app.importing}
  <ImportDialog />
{/if}

{#if app.toast}
  <div class="toast" class:error={app.toast.kind === "error"} role="status">{app.toast.text}</div>
{/if}

<style>
  .shell {
    display: grid;
    grid-template-columns: 232px 1fr;
    height: 100vh;
  }
  .sidebar {
    display: flex;
    flex-direction: column;
    background: var(--sidebar);
    border-right: 1px solid var(--border);
    padding: 18px 12px 14px;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 11px;
    padding: 4px 8px 22px;
  }
  .brand .name {
    font-weight: 750;
    font-size: 16px;
    letter-spacing: -0.01em;
  }
  .brand .tag {
    font-size: 12px;
    color: var(--faint);
  }
  nav {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  nav button {
    display: flex;
    align-items: center;
    gap: 11px;
    height: 38px;
    padding: 0 10px;
    border: 0;
    border-radius: 10px;
    background: transparent;
    color: var(--muted);
    font-weight: 600;
    cursor: pointer;
    text-align: left;
  }
  nav button:hover {
    background: var(--hover);
    color: var(--text);
  }
  nav button.active {
    background: var(--accent-bg);
    color: var(--accent);
  }
  nav button span:nth-child(2) {
    flex: 1;
  }
  .count {
    min-width: 22px;
    height: 20px;
    padding: 0 6px;
    border-radius: 999px;
    background: var(--panel-2);
    border: 1px solid var(--border);
    font-size: 11px;
    display: grid;
    place-items: center;
    color: var(--muted);
  }
  .foot {
    margin-top: auto;
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 0 4px;
  }
  .hide {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 34px;
    padding: 0 10px;
    border-radius: 10px;
    border: 1px solid var(--border);
    background: var(--panel);
    color: var(--text);
    font-weight: 600;
    font-size: 12.5px;
    cursor: pointer;
  }
  .hide:hover {
    border-color: var(--accent);
  }
  .status {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    color: var(--faint);
    padding: 0 6px;
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--faint);
  }
  .dot.on {
    background: var(--ok);
    box-shadow: 0 0 8px var(--ok);
  }
  main {
    overflow: auto;
  }
  .loading {
    padding: 40px;
    color: var(--muted);
  }
  .toast {
    position: fixed;
    left: 50%;
    bottom: 22px;
    transform: translateX(-50%);
    max-width: min(560px, 90vw);
    padding: 10px 16px;
    border-radius: 12px;
    background: var(--panel);
    border: 1px solid var(--border-strong);
    box-shadow: var(--shadow);
    font-weight: 600;
    white-space: pre-wrap;
    z-index: 100;
  }
  .toast.error {
    border-color: var(--danger);
    color: var(--danger);
  }
</style>
