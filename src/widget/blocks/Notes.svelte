<script>
  // Editable text saved to the widget's storage.
  import { getContext, onMount } from "svelte";
  import { api } from "../../lib/api.js";

  const forge = getContext("forge");
  let { key, placeholder, paper } = $props();
  const PAPERS = {
    yellow: "#fff3a8", pink: "#ffc9de", green: "#c9f2c1",
    blue: "#c4e3ff", purple: "#e0ccff", graphite: "#3b4150",
  };
  let text = $state("");
  let loaded = false;
  let timer;

  onMount(async () => {
    text = (await api.storageGet(forge.instanceId, key).catch(() => "")) ?? "";
    loaded = true;
  });

  function save() {
    if (!loaded) return;
    clearTimeout(timer);
    timer = setTimeout(() => api.storageSet(forge.instanceId, key, text), 400);
  }

  let bg = $derived(PAPERS[paper] ?? paper ?? "transparent");
  let dark = $derived(paper === "graphite");
</script>

<textarea
  class="notes"
  class:paper={!!paper}
  style="background: {paper ? bg : 'transparent'}; color: {paper && !dark ? '#2a2a2a' : 'inherit'}"
  {placeholder}
  bind:value={text}
  oninput={save}
></textarea>

<style>
  .notes {
    flex: 1;
    width: 100%;
    min-height: 60px;
    box-sizing: border-box;
    border: 0;
    outline: none;
    resize: none;
    font: 14px/1.45 system-ui, "Segoe UI", sans-serif;
    user-select: text;
  }
  .paper {
    /* Fill the whole widget, like a sheet of paper. */
    position: absolute;
    inset: 0;
    padding: 30px 14px 14px;
  }
</style>
