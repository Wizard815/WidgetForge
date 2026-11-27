<script>
  // A month grid with today marked and a dot on days that have events.
  // `events`: [{ start (ms), summary }] — e.g. from an ICS calendar source.
  let { events = [], mondayFirst = true } = $props();

  let shown = $state(new Date(new Date().getFullYear(), new Date().getMonth(), 1));
  const dayKey = (d) => `${d.getFullYear()}-${d.getMonth()}-${d.getDate()}`;

  let busy = $derived.by(() => {
    const map = new Map();
    for (const e of Array.isArray(events) ? events : []) {
      const k = dayKey(new Date(e.start));
      map.set(k, [...(map.get(k) ?? []), e.summary]);
    }
    return map;
  });

  let weekdays = $derived.by(() => {
    const base = new Date(2024, 0, mondayFirst ? 1 : 7); // a Monday / a Sunday
    return Array.from({ length: 7 }, (_, i) =>
      new Intl.DateTimeFormat(undefined, { weekday: "narrow" }).format(new Date(base.getFullYear(), 0, base.getDate() + i)),
    );
  });

  let cells = $derived.by(() => {
    const first = shown.getDay();
    const lead = mondayFirst ? (first + 6) % 7 : first;
    const start = new Date(shown.getFullYear(), shown.getMonth(), 1 - lead);
    return Array.from({ length: 42 }, (_, i) => new Date(start.getFullYear(), start.getMonth(), start.getDate() + i));
  });

  const today = dayKey(new Date());
  const move = (n) => (shown = new Date(shown.getFullYear(), shown.getMonth() + n, 1));
  let title = $derived(new Intl.DateTimeFormat(undefined, { month: "long", year: "numeric" }).format(shown));
</script>

<div class="month">
  <div class="head">
    <button onclick={() => move(-1)} title="Previous month">‹</button>
    <button class="title" onclick={() => (shown = new Date(new Date().getFullYear(), new Date().getMonth(), 1))} title="Back to this month">{title}</button>
    <button onclick={() => move(1)} title="Next month">›</button>
  </div>
  <div class="grid">
    {#each weekdays as w}<div class="wd">{w}</div>{/each}
    {#each cells as d}
      {@const k = dayKey(d)}
      <div
        class="day"
        class:other={d.getMonth() !== shown.getMonth()}
        class:today={k === today}
        title={busy.get(k)?.join("\n")}
      >
        {d.getDate()}
        {#if busy.has(k)}<span class="dot"></span>{/if}
      </div>
    {/each}
  </div>
</div>

<style>
  .month { display: flex; flex-direction: column; gap: 6px; }
  .head { display: flex; align-items: center; gap: 4px; }
  .head button {
    border: 0; background: none; color: inherit; cursor: pointer; font: inherit;
    padding: 2px 8px; border-radius: 6px;
  }
  .head button:hover { background: color-mix(in srgb, currentColor 10%, transparent); }
  .head .title { flex: 1; font-weight: 700; text-transform: capitalize; }
  .grid { display: grid; grid-template-columns: repeat(7, 1fr); gap: 2px; text-align: center; }
  .wd { font-size: 10px; opacity: 0.5; padding-bottom: 2px; }
  .day {
    position: relative; font-size: 12px; padding: 4px 0; border-radius: 6px;
    font-variant-numeric: tabular-nums;
  }
  .day.other { opacity: 0.3; }
  .day.today { background: var(--accent); color: var(--wf-on-accent); font-weight: 700; }
  .dot {
    position: absolute; left: 50%; bottom: 1px; width: 4px; height: 4px; margin-left: -2px;
    border-radius: 50%; background: var(--accent);
  }
  .day.today .dot { background: var(--wf-on-accent); }
</style>
