<script>
  // A text box (or a drop-down with `options`). What is typed is readable in
  // bindings as {{ input.<name> }}; Enter or the button runs the `submit` source.
  import { getContext, onMount } from "svelte";
  let { name, placeholder = "", submit, button = "", options = null, initial = "", multiline = false } = $props();
  const forge = getContext("forge");
  let value = $state("");
  let busy = $state(false);

  onMount(() => {
    value = initial ?? "";
    forge.setInput(name, value);
  });
  const opts = $derived(
    Array.isArray(options) ? options.map((o) => (typeof o === "object" ? { value: o.value ?? o.code, label: o.label ?? o.name ?? o.value } : { value: o, label: o })) : null,
  );
  function changed(v) {
    value = v;
    forge.setInput(name, v);
    if (opts && submit) go();
  }
  async function go() {
    if (!submit || busy) return;
    busy = true;
    try {
      await forge.runSource(submit);
    } catch (e) {
      forge.report(String(e));
    } finally {
      busy = false;
    }
  }
  function key(e) {
    if (e.key === "Enter" && !(multiline && e.shiftKey)) {
      e.preventDefault();
      go();
    }
  }
</script>

<div class="input">
  {#if opts}
    <select {value} onchange={(e) => changed(e.currentTarget.value)}>
      {#each opts as o}<option value={o.value}>{o.label}</option>{/each}
    </select>
  {:else if multiline}
    <textarea {placeholder} {value} rows="3" oninput={(e) => changed(e.currentTarget.value)} onkeydown={key}></textarea>
  {:else}
    <input {placeholder} {value} oninput={(e) => changed(e.currentTarget.value)} onkeydown={key} />
  {/if}
  {#if button && submit}<button onclick={go} disabled={busy}>{busy ? "…" : button}</button>{/if}
</div>

<style>
  .input { display: flex; gap: 6px; align-items: stretch; }
  input, textarea, select {
    flex: 1; min-width: 0; font: inherit; font-size: 13px; color: inherit; padding: 6px 8px; resize: none;
    border: 1px solid color-mix(in srgb, currentColor 16%, transparent); border-radius: 7px; background: color-mix(in srgb, currentColor 7%, transparent);
  }
  input:focus, textarea:focus, select:focus { outline: none; border-color: var(--accent); }
  button {
    font: inherit; font-size: 12px; color: inherit; cursor: pointer; padding: 0 10px;
    border: 1px solid color-mix(in srgb, currentColor 16%, transparent); border-radius: 7px; background: color-mix(in srgb, currentColor 8%, transparent);
  }
  button:hover { border-color: var(--accent); }
</style>
