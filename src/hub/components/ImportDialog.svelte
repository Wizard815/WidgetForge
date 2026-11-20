<script lang="ts">
  import Icon from "./Icon.svelte";
  import { app, confirmImport, describePermission, permissionList } from "../store.svelte";

  const imp = $derived(app.importing!);
  const m = $derived(imp.preview.manifest);
  const perms = $derived(permissionList(m));
  const isNew = (key: string) => imp.preview.replaces_version !== null && imp.preview.new_permissions.includes(key);
</script>

<div class="backdrop" role="presentation" onclick={() => (app.importing = null)}>
  <div class="dialog card" role="dialog" aria-modal="true" aria-label="Install widget" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.key === "Escape" && (app.importing = null)} tabindex="-1">
    <h2>{imp.preview.replaces_version ? "Update" : "Install"} {m.name}?</h2>
    <p class="muted small">
      {m.id} · v{m.version}{m.author ? ` · by ${m.author}` : ""}
      {#if imp.preview.replaces_version}
        · replaces v{imp.preview.replaces_version}
      {/if}
    </p>
    {#if m.description}<p class="desc">{m.description}</p>{/if}

    <div class="perms">
      <div class="perm-head"><Icon name="shield" size={16} /> This widget can</div>
      {#if perms.length === 0}
        <p class="muted small">Only show things on screen. It has no access to your system, files or the internet.</p>
      {:else}
        <ul>
          {#each perms as p (p.key)}
            <li class:new={isNew(p.key)}>
              {describePermission(p.key)}
              {#if isNew(p.key)}<span class="chip warn">new</span>{/if}
            </li>
          {/each}
        </ul>
      {/if}
      {#if m.ui.entry}
        <p class="muted small">It runs its own code, isolated from the rest of WidgetForge.</p>
      {/if}
    </div>

    {#if imp.preview.warnings.length}
      <details>
        <summary class="small muted">{imp.preview.warnings.length} warning(s)</summary>
        <ul class="small muted">
          {#each imp.preview.warnings as w}<li>{w.path}: {w.message}</li>{/each}
        </ul>
      </details>
    {/if}

    <div class="buttons">
      <button class="btn" onclick={() => (app.importing = null)}>Cancel</button>
      <button class="btn primary" onclick={confirmImport}>{imp.preview.replaces_version ? "Update" : "Install"}</button>
    </div>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.45);
    display: grid;
    place-items: center;
    z-index: 50;
  }
  .dialog {
    width: min(460px, 92vw);
    padding: 22px;
    box-shadow: var(--shadow);
    outline: none;
  }
  .desc {
    margin-top: 12px;
  }
  .perms {
    margin-top: 16px;
    padding: 12px 14px;
    border-radius: 12px;
    background: var(--panel-2);
    border: 1px solid var(--border);
  }
  .perm-head {
    display: flex;
    align-items: center;
    gap: 8px;
    font-weight: 650;
    margin-bottom: 6px;
  }
  ul {
    margin: 0;
    padding-left: 20px;
  }
  li {
    padding: 2px 0;
  }
  li.new {
    color: var(--warn);
  }
  details {
    margin-top: 12px;
  }
  .buttons {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 20px;
  }
</style>
