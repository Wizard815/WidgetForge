<script>
  // Bars (or a line) for a list of past values, e.g. speed-test history.
  // `value` / `second` are paths inside each item.
  import { getPath } from "../../lib/bindings.js";
  let { items = [], value = "value", second, label = "", format = "", color, secondColor, kind = "bars" } = $props();
  let list = $derived(Array.isArray(items) ? items.slice(-48) : []);
  let a = $derived(list.map((x) => Number(getPath(x, value)) || 0));
  let b2 = $derived(second ? list.map((x) => Number(getPath(x, second)) || 0) : []);
  let top = $derived(Math.max(1, ...a, ...b2) * 1.1);
  const W = 100, H = 40;
  let step = $derived(list.length ? W / list.length : W);
  const line = (vals) => vals.map((v, i) => `${(i + 0.5) * step},${H - (v / top) * (H - 2)}`).join(" ");
</script>

<div class="chart">
  {#if label || format}<div class="top"><span class="label">{label}</span><span>{format}</span></div>{/if}
  {#if list.length === 0}
    <div class="empty">No history yet.</div>
  {:else}
    <svg viewBox="0 0 {W} {H}" preserveAspectRatio="none">
      {#if kind === "line"}
        <polyline points={line(a)} fill="none" stroke={color} stroke-width="1.5" vector-effect="non-scaling-stroke" />
      {:else}
        {#each a as v, i}
          <rect x={i * step + step * 0.15} width={step * (second ? 0.35 : 0.7)} y={H - (v / top) * (H - 2)} height={(v / top) * (H - 2)} fill={color} rx="0.6" />
        {/each}
      {/if}
      {#if second}
        {#if kind === "line"}
          <polyline points={line(b2)} fill="none" stroke={secondColor} stroke-width="1.5" vector-effect="non-scaling-stroke" />
        {:else}
          {#each b2 as v, i}
            <rect x={i * step + step * 0.5} width={step * 0.35} y={H - (v / top) * (H - 2)} height={(v / top) * (H - 2)} fill={secondColor} rx="0.6" />
          {/each}
        {/if}
      {/if}
    </svg>
  {/if}
</div>

<style>
  .chart { display: flex; flex-direction: column; gap: 4px; }
  .top { display: flex; justify-content: space-between; font-size: 12px; gap: 8px; }
  .label { opacity: 0.75; }
  svg { width: 100%; height: 64px; display: block; background: color-mix(in srgb, currentColor 3%, transparent); border-radius: 6px; }
  .empty { font-size: 12px; opacity: 0.55; }
</style>
