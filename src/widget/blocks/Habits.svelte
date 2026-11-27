<script>
  // Habit tracker: tick each habit once a day; shows the last days and the streak.
  // Saved in the widget's storage as { habits: [{ name, days: ["2026-11-26", ...] }] }.
  import { getContext, onMount } from "svelte";
  import { api } from "../../lib/api.js";
  let { key = "habits", days = 7 } = $props();
  const forge = getContext("forge");
  let habits = $state([]);
  let draft = $state("");
  let editing = $state(false);

  const dayKey = (d) => `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
  const back = (n) => {
    const d = new Date();
    d.setHours(12, 0, 0, 0);
    d.setDate(d.getDate() - n);
    return d;
  };
  let columns = $derived(Array.from({ length: days }, (_, i) => back(days - 1 - i)));
  const today = dayKey(new Date());

  onMount(async () => {
    const saved = await api.storageGet(forge.instanceId, key).catch(() => null);
    if (Array.isArray(saved)) habits = saved;
    else editing = true;
  });
  const save = () => api.storageSet(forge.instanceId, key, habits).catch((e) => forge.report(String(e)));

  function toggle(h, k) {
    h.days = h.days.includes(k) ? h.days.filter((x) => x !== k) : [...h.days, k].sort().slice(-400);
    save();
  }
  function streak(h) {
    const set = new Set(h.days);
    // A streak still counts today before you tick it.
    let n = 0, i = set.has(today) ? 0 : 1;
    while (set.has(dayKey(back(i)))) { n++; i++; }
    return n;
  }
  function add(e) {
    e.preventDefault();
    const name = draft.trim();
    if (!name) return;
    habits.push({ name, days: [] });
    draft = "";
    save();
  }
  function remove(i) {
    habits.splice(i, 1);
    save();
  }
  const weekday = (d) => new Intl.DateTimeFormat(undefined, { weekday: "narrow" }).format(d);
</script>

<div class="habits">
  <div class="row head">
    <span class="name"></span>
    {#each columns as d}<span class="cell wd" class:today={dayKey(d) === today}>{weekday(d)}</span>{/each}
    <span class="streak" title="Days in a row">🔥</span>
  </div>
  {#each habits as h, i}
    <div class="row">
      <span class="name" title={h.name}>
        {#if editing}<button class="x" onclick={() => remove(i)} title="Remove">×</button>{/if}{h.name}
      </span>
      {#each columns as d}
        {@const k = dayKey(d)}
        <button class="cell dot" class:on={h.days.includes(k)} class:today={k === today} onclick={() => toggle(h, k)} title={k}></button>
      {/each}
      <span class="streak">{streak(h)}</span>
    </div>
  {/each}
  {#if editing}
    <form onsubmit={add}><input bind:value={draft} placeholder="New habit (e.g. Read 20 minutes)" /></form>
  {/if}
  <button class="edit" onclick={() => (editing = !editing)}>{editing ? "Done" : "Edit"}</button>
</div>

<style>
  .habits { display: flex; flex-direction: column; gap: 4px; font-size: 12px; }
  .row { display: flex; align-items: center; gap: 3px; }
  .name { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .cell { width: 18px; flex: none; text-align: center; }
  .wd { font-size: 10px; opacity: 0.5; }
  .wd.today { opacity: 1; color: var(--accent); font-weight: 700; }
  .dot { height: 18px; border-radius: 5px; border: 1px solid color-mix(in srgb, currentColor 18%, transparent); background: color-mix(in srgb, currentColor 4%, transparent); cursor: pointer; padding: 0; }
  .dot.today { border-color: color-mix(in srgb, currentColor 45%, transparent); }
  .dot.on { background: var(--accent); border-color: var(--accent); }
  .streak { width: 24px; text-align: right; font-variant-numeric: tabular-nums; font-weight: 700; }
  .x { border: 0; background: none; color: var(--wf-bad); cursor: pointer; padding: 0 4px 0 0; font-size: 13px; }
  input { width: 100%; font: inherit; color: inherit; padding: 5px 7px; border-radius: 6px; border: 1px solid color-mix(in srgb, currentColor 16%, transparent); background: color-mix(in srgb, currentColor 7%, transparent); }
  .edit { align-self: flex-end; border: 0; background: none; color: inherit; opacity: 0.5; cursor: pointer; font-size: 11px; }
  .edit:hover { opacity: 1; }
</style>
