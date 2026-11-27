<script>
  // The install screen for one package: what it is, what it may do, what it replaces.
  // PLACEHOLDER wording throughout.
  import { api } from "../lib/api.js";

  let { info, oninstalled, oncancel, source = "" } = $props();
  let busy = $state(false);
  let message = $state("");

  async function install() {
    busy = true;
    message = "";
    try {
      const r = await api.installPackage(info.path, info.installed);
      oninstalled?.(r.id, info);
    } catch (e) {
      message = String(e);
    } finally {
      busy = false;
    }
  }
  const kb = (n) => (n > 1048576 ? `${(n / 1048576).toFixed(1)} MB` : `${Math.max(1, Math.round(n / 1024))} KB`);
</script>

  <div class="card">
    <div class="head">
      <div>
        <strong>{info.name}</strong> <small>v{info.version}{info.author ? ` · by ${info.author}` : ""}</small>
        <div class="id"><code>{info.id}</code> · {info.files} files · {kb(info.bytes)}{info.html ? " · runs its own web page (in a sandbox)" : ""}</div>
      </div>
    </div>
    {#if info.description}<p>{info.description}</p>{/if}
    <h3>It will be able to</h3>
    <ul>
      {#each info.permissions as p}<li class:new={info.new_permissions.includes(p)}>{p}{#if info.new_permissions.includes(p)} <span class="tag">new</span>{/if}</li>{/each}
      {#if !info.permissions.length}<li>Nothing beyond showing itself</li>{/if}
    </ul>
    {#if info.installed}
      <p class="warn">
        You have version {info.installed_version} installed{info.installed_source === "built-in" ? " (built-in)" : ""}.
        Installing replaces it; the old copy is kept in the add-ons folder under <code>.replaced</code>.
        {#if info.new_permissions.length}This version asks for <strong>{info.new_permissions.length} new permission{info.new_permissions.length > 1 ? "s" : ""}</strong> (marked above).{/if}
      </p>
    {/if}
    {#if source}<p class="note">From {source}. The download matched the catalog's checksum.</p>{/if}
    <p class="note">Only install widgets from people you trust. Keys you enter stay in your system keychain and are never part of a package.</p>
    {#if message}<p class="warn">{message}</p>{/if}
    <div class="buttons">
      <button class="primary" disabled={busy} onclick={install}>{info.installed ? "Replace and install" : "Install"}</button>
      <button onclick={() => oncancel?.()}>Cancel</button>
    </div>
  </div>

<style>
  .card { margin-top: 12px; padding: 14px; border: 1px solid #2c3548; border-radius: 10px; background: #141925; }
  .id { font-size: 12px; opacity: 0.6; margin-top: 2px; }
  h3 { font-size: 13px; margin: 10px 0 4px; }
  ul { margin: 0; padding-left: 20px; font-size: 13px; }
  li.new { color: #ffd166; }
  .tag { font-size: 10px; padding: 1px 6px; border-radius: 8px; background: #5a4410; }
  .warn { color: #ffd166; font-size: 13px; }
  .note { opacity: 0.6; font-size: 12px; }
  .buttons { display: flex; gap: 8px; }
</style>
