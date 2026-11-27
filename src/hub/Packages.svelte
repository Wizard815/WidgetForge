<script>
  // Import a .forgewidget: see what it is and what it may do, then install.
  // Packages arrive by dropping them on the Hub, double-clicking one, or typing a path.
  // PLACEHOLDER wording throughout.
  import { api } from "../lib/api.js";
  import PackageCard from "./PackageCard.svelte";

  let { pending = $bindable(null), oninstalled } = $props();
  let path = $state("");
  let info = $state(null);
  let message = $state("");

  $effect(() => {
    if (pending) {
      const p = pending;
      pending = null;
      inspect(p);
    }
  });

  async function inspect(p) {
    message = "";
    info = null;
    path = p;
    try {
      info = await api.inspectPackage(p);
    } catch (e) {
      message = `Cannot use this package: ${e}`;
    }
  }
</script>

<h2>Import a widget</h2>
<div class="drop">
  <span>Drop a <code>.forgewidget</code> file anywhere on this window, double-click one, or type its path:</span>
  <form onsubmit={(e) => { e.preventDefault(); inspect(path); }}>
    <input bind:value={path} placeholder="/home/you/Downloads/my.widget-1.0.0.forgewidget" spellcheck="false" />
    <button>Open</button>
  </form>
</div>
{#if message}<p class="msg">{message}</p>{/if}

{#if info}
  <PackageCard
    {info}
    oninstalled={() => { message = `Installed ${info.name}. Find it in the Library.`; info = null; oninstalled?.(); }}
    oncancel={() => (info = null)}
  />
{/if}

<style>
  .drop { border: 1px dashed #3a4870; border-radius: 10px; padding: 12px 14px; font-size: 13px; display: flex; flex-direction: column; gap: 8px; }
  form { display: flex; gap: 8px; }
  input { flex: 1; padding: 6px 8px; border: 1px solid #2c3548; border-radius: 8px; background: #0f131c; font-family: ui-monospace, monospace; font-size: 12px; }
  .msg { padding: 8px 12px; border-radius: 8px; background: #1f3350; }
</style>
