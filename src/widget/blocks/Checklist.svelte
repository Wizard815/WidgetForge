<script>
  // A to-do list saved to the widget's storage.
  import { getContext, onMount } from "svelte";
  import { api } from "../../lib/api.js";

  const forge = getContext("forge");
  let { key, placeholder } = $props();
  let items = $state([]);
  let draft = $state("");

  onMount(async () => {
    const saved = await api.storageGet(forge.instanceId, key).catch(() => null);
    if (Array.isArray(saved)) items = saved;
  });

  const save = () => api.storageSet(forge.instanceId, key, items);

  function add(e) {
    e.preventDefault();
    const text = draft.trim();
    if (!text) return;
    items.push({ text, done: false });
    draft = "";
    save();
  }
  function toggle(i) {
    items[i].done = !items[i].done;
    save();
  }
  function remove(i) {
    items.splice(i, 1);
    save();
  }
  function clearDone() {
    items = items.filter((it) => !it.done);
    save();
  }
  let doneCount = $derived(items.filter((it) => it.done).length);
</script>

<div class="checklist">
  <ul>
    {#each items as item, i}
      <li class:done={item.done}>
        <label><input type="checkbox" checked={item.done} onchange={() => toggle(i)} /> <span>{item.text}</span></label>
        <button class="x" title="Delete" onclick={() => remove(i)}>✕</button>
      </li>
    {/each}
  </ul>
  <form onsubmit={add}>
    <input type="text" bind:value={draft} {placeholder} />
  </form>
  {#if doneCount}
    <button class="clear" onclick={clearDone}>Clear {doneCount} done</button>
  {/if}
</div>

<style>
  .checklist { display: flex; flex-direction: column; gap: 6px; }
  ul { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 2px; }
  li { display: flex; align-items: center; gap: 4px; padding: 3px 2px; border-radius: 6px; }
  li:hover { background: color-mix(in srgb, currentColor 6%, transparent); }
  label { flex: 1; display: flex; align-items: center; gap: 6px; cursor: pointer; }
  input[type="checkbox"] { accent-color: var(--accent); }
  .done span { opacity: 0.5; text-decoration: line-through; }
  .x { opacity: 0; border: 0; background: none; color: inherit; cursor: pointer; font-size: 11px; }
  li:hover .x { opacity: 0.6; }
  input[type="text"] {
    width: 100%;
    box-sizing: border-box;
    padding: 6px 8px;
    border: 1px solid color-mix(in srgb, currentColor 14%, transparent);
    border-radius: 7px;
    background: color-mix(in srgb, currentColor 6%, transparent);
    color: inherit;
    font: inherit;
    outline: none;
    user-select: text;
  }
  input[type="text"]:focus { border-color: var(--accent); }
  .clear {
    align-self: flex-start;
    border: 0;
    background: none;
    color: var(--accent);
    font: inherit;
    font-size: 12px;
    cursor: pointer;
    padding: 0;
  }
</style>
