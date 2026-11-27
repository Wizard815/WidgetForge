<script>
  // Sparkline with a short history. A new point is added on every data update.
  import { untrack } from "svelte";
  let { value, max, label, format, color, tick } = $props();
  const POINTS = 60;
  let history = $state([]);
  let lastTick = -1;

  $effect(() => {
    if (tick === lastTick) return;
    lastTick = tick;
    const v = value;
    untrack(() => (history = [...history.slice(-(POINTS - 1)), v]));
  });

  // max: "auto" scales to the largest recent value (for network speeds and latency).
  let top = $derived(max === "auto" ? Math.max(1, ...history) * 1.15 : max);

  let path = $derived.by(() => {
    if (history.length < 2) return "";
    const step = 100 / (POINTS - 1);
    const offset = (POINTS - history.length) * step;
    return history
      .map((v, i) => `${(offset + i * step).toFixed(2)},${(40 - (Math.min(v, top) / (top || 1)) * 38).toFixed(2)}`)
      .join(" ");
  });
</script>

<div class="graph">
  <div class="top"><span class="label">{label}</span><span>{format}</span></div>
  <svg viewBox="0 0 100 40" preserveAspectRatio="none">
    {#if path}
      <polygon points="{path} 100,40 {path.split(' ')[0].split(',')[0]},40" fill={color} opacity="0.18" />
      <polyline points={path} fill="none" stroke={color} stroke-width="1.5" vector-effect="non-scaling-stroke" />
    {/if}
  </svg>
</div>

<style>
  .top { display: flex; justify-content: space-between; font-size: 12px; margin-bottom: 4px; }
  .label { opacity: 0.75; }
  svg {
    width: 100%;
    height: 48px;
    display: block;
    border-radius: 6px;
    background: color-mix(in srgb, currentColor 5%, transparent);
  }
</style>
