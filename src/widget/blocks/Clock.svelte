<script>
  // Analog clock face.
  import { onMount } from "svelte";
  import { shiftToZone } from "../../lib/bindings.js";
  let { seconds = true, timezone = "" } = $props();
  const read = () => {
    try {
      return new Date(shiftToZone(Date.now(), timezone));
    } catch {
      return new Date();
    }
  };
  let now = $state(read());

  onMount(() => {
    const t = setInterval(() => (now = read()), 1000);
    return () => clearInterval(t);
  });

  let s = $derived(now.getSeconds());
  let m = $derived(now.getMinutes() + s / 60);
  let h = $derived((now.getHours() % 12) + m / 60);
</script>

<svg class="clock" viewBox="0 0 100 100">
  <circle cx="50" cy="50" r="46" class="face" />
  {#each Array(12) as _, i}
    <line x1="50" y1="8" x2="50" y2={i % 3 === 0 ? 15 : 12} class="mark" transform="rotate({i * 30} 50 50)" />
  {/each}
  <line x1="50" y1="50" x2="50" y2="27" class="hour" transform="rotate({h * 30} 50 50)" />
  <line x1="50" y1="50" x2="50" y2="17" class="minute" transform="rotate({m * 6} 50 50)" />
  {#if seconds}
    <line x1="50" y1="57" x2="50" y2="14" class="second" transform="rotate({s * 6} 50 50)" />
  {/if}
  <circle cx="50" cy="50" r="2.5" class="pin" />
</svg>

<style>
  .clock { width: 100%; max-height: 150px; }
  .face { fill: color-mix(in srgb, currentColor 4%, transparent); stroke: color-mix(in srgb, currentColor 18%, transparent); stroke-width: 1.5; }
  .mark { stroke: color-mix(in srgb, currentColor 50%, transparent); stroke-width: 1.5; stroke-linecap: round; }
  .hour { stroke: currentColor; stroke-width: 4; stroke-linecap: round; }
  .minute { stroke: currentColor; stroke-width: 2.5; stroke-linecap: round; }
  .second { stroke: var(--accent); stroke-width: 1.2; stroke-linecap: round; }
  .pin { fill: var(--accent); }
</style>
