<script>
  // Volume mixer: output devices (with "use this one") and each app's volume.
  // `audio`: { devices: [{ id, name, volume, muted, default }], apps: [{ id, name, volume, muted }] }
  import { getContext } from "svelte";
  import { api } from "../../lib/api.js";

  let { audio = null, showApps = true, showDevices = true } = $props();
  const forge = getContext("forge");

  // Values being dragged are shown at once, then sent a moment later.
  let pending = $state({});
  let timers = {};

  function setVolume(kind, id, value) {
    pending[`${kind}:${id}`] = value;
    clearTimeout(timers[`${kind}:${id}`]);
    timers[`${kind}:${id}`] = setTimeout(async () => {
      await send({ kind, id, volume: value / 100 });
      delete pending[`${kind}:${id}`];
    }, 120);
  }
  async function send(change) {
    try {
      await api.audioSet(forge.instanceId, change);
    } catch (e) {
      forge.report(String(e));
    }
    forge.refresh();
  }
  const vol = (kind, item) => pending[`${kind}:${item.id}`] ?? Math.round((item.volume ?? 0) * 100);
</script>

{#if !audio}
  <div class="empty">Reading audio…</div>
{:else if audio.available === false}
  <div class="empty">{audio.reason ?? "Audio control is not available on this computer yet."}</div>
{:else}
  {#if showDevices}
    <div class="label">Output</div>
    {#each audio.devices ?? [] as d (d.id)}
      <div class="row" class:current={d.default}>
        <button class="pick" title={d.default ? "In use" : "Use this output"} onclick={() => !d.default && send({ kind: "device", id: d.id, default: true })}>
          {d.default ? "●" : "○"}
        </button>
        <span class="name" title={d.name}>{d.name}</span>
        <button class="mute" class:on={d.muted} title={d.muted ? "Unmute" : "Mute"} onclick={() => send({ kind: "device", id: d.id, muted: !d.muted })}>
          {d.muted ? "🔇" : "🔊"}
        </button>
        <input type="range" min="0" max="100" value={vol("device", d)} oninput={(e) => setVolume("device", d.id, +e.currentTarget.value)} />
        <span class="pct">{vol("device", d)}</span>
      </div>
    {/each}
  {/if}
  {#if showApps}
    <div class="label">Apps</div>
    {#each audio.apps ?? [] as a (a.id)}
      <div class="row">
        <span class="name" title={a.name}>{a.name}</span>
        <button class="mute" class:on={a.muted} title={a.muted ? "Unmute" : "Mute"} onclick={() => send({ kind: "app", id: a.id, muted: !a.muted })}>
          {a.muted ? "🔇" : "🔊"}
        </button>
        <input type="range" min="0" max="100" value={vol("app", a)} oninput={(e) => setVolume("app", a.id, +e.currentTarget.value)} />
        <span class="pct">{vol("app", a)}</span>
      </div>
    {:else}
      <div class="empty">No app is playing sound.</div>
    {/each}
  {/if}
{/if}

<style>
  .label { font-size: 11px; text-transform: uppercase; letter-spacing: 0.08em; opacity: 0.55; margin-top: 4px; }
  .row { display: grid; grid-template-columns: auto 1fr auto 90px 28px; align-items: center; gap: 6px; padding: 2px 0; }
  .row:not(:has(.pick)) { grid-template-columns: 1fr auto 90px 28px; }
  .name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 12px; }
  .current .name { font-weight: 700; }
  button {
    border: 0; background: none; color: inherit; cursor: pointer; padding: 0 2px; font-size: 12px; opacity: 0.8;
  }
  button:hover { opacity: 1; }
  .pick { color: var(--accent); }
  .mute.on { opacity: 1; }
  input[type="range"] { width: 100%; accent-color: var(--accent); }
  .pct { font-size: 11px; text-align: right; opacity: 0.7; font-variant-numeric: tabular-nums; }
  .empty { font-size: 12px; opacity: 0.6; }
</style>
