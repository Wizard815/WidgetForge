<script>
  // Desk presets: save the whole desktop and switch between saved ones.
  // PLACEHOLDER wording throughout.
  import { onMount } from "svelte";
  import { api } from "../lib/api.js";

  let presets = $state([]);
  let errors = $state([]);
  let name = $state("");
  let message = $state("");
  let confirming = $state(null); // { action, preset }

  async function refresh() {
    const r = await api.listPresets();
    presets = r.presets;
    errors = r.errors;
  }

  async function act(fn, done) {
    try {
      message = "";
      await fn();
      if (done) message = done;
    } catch (e) {
      message = String(e);
    }
    confirming = null;
    await refresh();
  }

  function save(e) {
    e.preventDefault();
    const n = name.trim();
    if (!n) return;
    const exists = presets.some((p) => p.name.toLowerCase() === n.toLowerCase());
    if (exists && confirming?.action !== "overwrite") {
      confirming = { action: "overwrite", preset: { name: n } };
      return;
    }
    act(() => api.savePreset(n), `Saved “${n}”.`).then(() => (name = ""));
  }

  const when = (s) => new Date(s * 1000).toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });

  onMount(refresh);
</script>

<header>
  <h1>Presets</h1>
  <p>Save your desktop — every widget, where it sits, its stack and its settings — and switch between saved desktops.</p>
</header>

{#if message}<div class="message">{message}</div>{/if}

<form class="save" onsubmit={save}>
  <input placeholder="Name, e.g. Work or Gaming" bind:value={name} maxlength="60" />
  <button class="primary" type="submit">Save current desktop</button>
</form>
{#if confirming?.action === "overwrite"}
  <div class="confirm">
    A preset called “{confirming.preset.name}” already exists. Replace it?
    <button class="danger" onclick={() => act(() => api.savePreset(confirming.preset.name), "Replaced.")}>Replace</button>
    <button onclick={() => (confirming = null)}>Cancel</button>
  </div>
{/if}

<div class="list">
  {#if presets.length === 0}<p class="muted">No presets yet.</p>{/if}
  {#each presets as p (p.file)}
    <div class="row">
      <div class="name">
        <strong>{p.name}</strong>
        <small>
          {p.widgets} widget{p.widgets === 1 ? "" : "s"} · {when(p.saved_at)}
          {#if p.auto} · saved automatically before your last switch{/if}
          {#if p.missing.length} · missing: {p.missing.join(", ")}{/if}
        </small>
      </div>
      {#if confirming?.preset === p}
        <span class="ask">{confirming.action === "apply" ? "Replace the current desktop?" : "Delete this preset?"}</span>
        {#if confirming.action === "apply"}
          <button class="primary" onclick={() => act(() => api.applyPreset(p.file), `Switched to “${p.name}”.`)}>Switch</button>
        {:else}
          <button class="danger" onclick={() => act(() => api.deletePreset(p.file), `Deleted “${p.name}”.`)}>Delete</button>
        {/if}
        <button onclick={() => (confirming = null)}>Cancel</button>
      {:else}
        <button class="primary" onclick={() => (confirming = { action: "apply", preset: p })}>Switch to this</button>
        <button class="danger" onclick={() => (confirming = { action: "delete", preset: p })}>Delete</button>
      {/if}
    </div>
  {/each}
</div>

{#if errors.length}
  <h2>Files that could not be read</h2>
  <ul class="errors">{#each errors as e}<li>{e}</li>{/each}</ul>
{/if}

<h2>Share a preset</h2>
<p class="muted">
  Presets are <code>.forgedesk</code> files. Copy one to another computer's presets folder to use it there.
  API keys are never put in a preset; set them again on the other computer. Notes and to-do items stay on this computer.
</p>
<div class="buttons">
  <button onclick={() => act(() => api.openPresetsFolder())}>Open presets folder</button>
  <button onclick={() => act(refresh)}>Reload list</button>
</div>

<style>
  header h1 { margin: 0 0 4px; font-size: 22px; }
  header p { margin: 0 0 18px; opacity: 0.65; }
  h2 { font-size: 15px; margin: 22px 0 8px; }
  .save { display: flex; gap: 8px; margin-bottom: 12px; }
  .save input { flex: 1; padding: 7px 10px; border: 1px solid #2c3548; border-radius: 8px; background: #1b2230; }
  .list { display: flex; flex-direction: column; gap: 8px; }
  .row {
    display: flex; align-items: center; gap: 8px; flex-wrap: wrap;
    padding: 10px 12px; border: 1px solid #222a3a; border-radius: 10px; background: #141925;
  }
  .name { flex: 1; min-width: 200px; display: flex; flex-direction: column; }
  .name small { opacity: 0.6; }
  .ask { font-size: 13px; color: #ffd166; }
  .confirm { display: flex; align-items: center; gap: 8px; margin-bottom: 12px; color: #ffd166; font-size: 13px; }
  .message { padding: 10px 12px; margin-bottom: 14px; border-radius: 8px; background: #1f3a2a; }
  .muted { opacity: 0.6; font-size: 13px; }
  .errors { color: #ffb4b4; }
  .buttons { display: flex; gap: 8px; }
</style>
