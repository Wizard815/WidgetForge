<script>
  // Break reminders: each comes round every N minutes. When one is due the
  // widget lights up, can play a soft tone and send a desktop notification.
  // `list`: [{ name, minutes, message }]
  import { getContext, onMount } from "svelte";
  import { api } from "../../lib/api.js";
  let { list = [], notify = true, sound = true } = $props();
  const forge = getContext("forge");

  let now = $state(Date.now());
  let due = $state({}); // name → when it is next due (ms)
  let paused = $state(false);
  let items = $derived((Array.isArray(list) ? list : []).filter((r) => r && r.name && Number(r.minutes) > 0));

  function reset(r) {
    due[r.name] = Date.now() + Number(r.minutes) * 60000;
  }
  function tone() {
    try {
      const ctx = new AudioContext();
      const o = ctx.createOscillator();
      const g = ctx.createGain();
      o.frequency.value = 660;
      g.gain.setValueAtTime(0.0001, ctx.currentTime);
      g.gain.exponentialRampToValueAtTime(0.15, ctx.currentTime + 0.05);
      g.gain.exponentialRampToValueAtTime(0.0001, ctx.currentTime + 0.9);
      o.connect(g).connect(ctx.destination);
      o.start();
      o.stop(ctx.currentTime + 1);
      setTimeout(() => ctx.close(), 1200);
    } catch {}
  }
  let alerted = {};
  onMount(() => {
    const t = setInterval(() => {
      now = Date.now();
      for (const r of items) {
        if (due[r.name] === undefined) reset(r);
        if (paused) due[r.name] += 1000;
        else if (now >= due[r.name] && !alerted[r.name]) {
          alerted[r.name] = true;
          if (sound) tone();
          if (notify) api.notify(forge.instanceId, r.name, r.message || r.name).catch(() => {});
        }
      }
    }, 1000);
    return () => clearInterval(t);
  });
  function done(r) {
    alerted[r.name] = false;
    reset(r);
  }
  const left = (r) => {
    const s = Math.max(0, Math.round(((due[r.name] ?? now) - now) / 1000));
    return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
  };
  const isDue = (r) => due[r.name] !== undefined && now >= due[r.name];
</script>

<div class="rem">
  {#each items as r (r.name)}
    <div class="r" class:due={isDue(r) && !paused}>
      <div class="text">
        <div class="name">{r.name}</div>
        <div class="msg">{isDue(r) ? r.message || "Time for a break" : `every ${r.minutes} min`}</div>
      </div>
      {#if isDue(r)}
        <button onclick={() => done(r)}>Done</button>
      {:else}
        <span class="left">{left(r)}</span>
      {/if}
    </div>
  {:else}
    <div class="msg">Turn on a reminder in this widget's settings.</div>
  {/each}
  {#if items.length}
    <button class="pause" onclick={() => (paused = !paused)}>{paused ? "Resume" : "Pause"}</button>
  {/if}
</div>

<style>
  .rem { display: flex; flex-direction: column; gap: 6px; }
  .r { display: flex; align-items: center; gap: 8px; padding: 6px 8px; border-radius: 8px; background: color-mix(in srgb, currentColor 5%, transparent); transition: background 0.3s; }
  .r.due { background: color-mix(in srgb, var(--accent) 35%, transparent); animation: pulse 1.6s ease-in-out infinite; }
  @keyframes pulse { 50% { background: color-mix(in srgb, var(--accent) 15%, transparent); } }
  .text { flex: 1; min-width: 0; }
  .name { font-weight: 700; font-size: 13px; }
  .msg { font-size: 11px; opacity: 0.7; }
  .left { font-variant-numeric: tabular-nums; font-size: 15px; opacity: 0.85; }
  button { font: inherit; font-size: 12px; color: inherit; cursor: pointer; padding: 4px 10px; border-radius: 7px; border: 1px solid color-mix(in srgb, currentColor 20%, transparent); background: color-mix(in srgb, currentColor 10%, transparent); }
  .pause { align-self: flex-end; border: 0; background: none; opacity: 0.5; padding: 0; font-size: 11px; }
  .pause:hover { opacity: 1; }
</style>
