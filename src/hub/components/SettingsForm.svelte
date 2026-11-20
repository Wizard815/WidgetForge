<script lang="ts">
  import type { SettingDef } from "../../lib/types";
  import Toggle from "./Toggle.svelte";

  let {
    defs,
    values,
    onchange,
  }: { defs: SettingDef[]; values: Record<string, unknown>; onchange: (key: string, value: unknown) => void } = $props();

  const val = (d: SettingDef) => (d.key in values ? values[d.key] : d.default);
  const labelOf = (d: SettingDef) => d.label ?? d.key;

  let timers: Record<string, number> = {};
  function debounced(key: string, value: unknown) {
    clearTimeout(timers[key]);
    timers[key] = window.setTimeout(() => onchange(key, value), 350);
  }
</script>

{#if defs.length === 0}
  <p class="muted small">This widget has no settings.</p>
{:else}
  <div class="form">
    {#each defs as d (d.key)}
      {#if d.type === "bool"}
        <Toggle label={labelOf(d)} checked={Boolean(val(d))} onchange={(v) => onchange(d.key, v)} />
      {:else}
        <label class="field">
          <span>{labelOf(d)}</span>
          {#if d.type === "choice"}
            <select value={String(val(d))} onchange={(e) => onchange(d.key, e.currentTarget.value)}>
              {#each d.options ?? [] as o}
                <option value={String(o)}>{String(o)}</option>
              {/each}
            </select>
          {:else if d.type === "number"}
            <input
              type="number"
              value={Number(val(d))}
              min={d.min}
              max={d.max}
              oninput={(e) => debounced(d.key, e.currentTarget.valueAsNumber)} />
          {:else if d.type === "color"}
            <input type="color" value={String(val(d) || "#7cc4ff")} oninput={(e) => debounced(d.key, e.currentTarget.value)} />
          {:else}
            <input type="text" value={String(val(d) ?? "")} oninput={(e) => debounced(d.key, e.currentTarget.value)} />
          {/if}
        </label>
      {/if}
    {/each}
  </div>
{/if}

<style>
  .form {
    display: flex;
    flex-direction: column;
  }
  .field {
    display: grid;
    grid-template-columns: 1fr minmax(160px, 260px);
    align-items: center;
    gap: 16px;
    padding: 6px 0;
  }
  .field input,
  .field select {
    width: 100%;
  }
  .field input[type="color"] {
    width: 44px;
    justify-self: end;
  }
</style>
