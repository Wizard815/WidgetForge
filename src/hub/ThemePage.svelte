<script>
  // Hub → Theme: one look for every widget. Changes show on the desktop at once.
  // PLACEHOLDER wording throughout.
  import { onMount } from "svelte";
  import { api } from "../lib/api.js";
  import { PRESETS, fullTheme } from "../lib/theme.js";
  import Preview from "./Preview.svelte";

  let theme = $state({});
  let saved = $state("");
  let full = $derived(fullTheme(theme));

  onMount(async () => {
    theme = (await api.getTheme().catch(() => ({}))) ?? {};
  });

  let timer;
  function save() {
    clearTimeout(timer);
    timer = setTimeout(async () => {
      try {
        theme = await api.setTheme(theme);
        saved = "Saved";
        setTimeout(() => (saved = ""), 1500);
      } catch (e) {
        saved = String(e);
      }
    }, 150);
  }
  function pickPreset(id) {
    // A preset replaces the look; font, size and accent choice are kept.
    const { font, scale, accent_mode } = theme;
    theme = { preset: id, font, scale, accent_mode };
    for (const k of Object.keys(theme)) if (theme[k] === undefined) delete theme[k];
    save();
  }
  function set(key, value) {
    theme = { ...theme, [key]: value };
    save();
  }
  function reset() {
    theme = {};
    save();
  }

  // Sample widgets for the examples (made-up numbers).
  const SAMPLE_DATA = {
    pc: { cpu: { usage: 37 }, memory: { usage: 62 } },
    list: [{ name: "Backup", state: "Done", ok: true }, { name: "Updates", state: "3 waiting", ok: false }],
  };
  const SAMPLES = [
    {
      manifest: { style: { accent: "#7cc4ff" } },
      blocks: [
        { type: "text", text: "System", weight: "bold" },
        { type: "grid", columns: 2, children: [{ type: "gauge", value: "{{ pc.cpu.usage }}", label: "CPU" }, { type: "gauge", value: "{{ pc.memory.usage }}", label: "Memory" }] },
      ],
    },
    {
      manifest: { style: { accent: "#ffc861" } },
      blocks: [
        { type: "text", text: "Today", weight: "bold" },
        { type: "stat", label: "London", value: "14°", note: "Partly cloudy" },
        { type: "bar", label: "Day done", value: 64, format: "64%" },
      ],
    },
    {
      manifest: { style: { accent: "#6cf0a0" } },
      blocks: [
        { type: "text", text: "Jobs", weight: "bold" },
        { type: "table", header: false, items: "{{ list }}", columns: [{ value: "{{ row.name }}" }, { value: "{{ row.state }}", align: "right", color: "{{ row.ok | choose('good', 'warn') }}" }] },
        { type: "text", text: "Checked 2 min ago", size: "xs", color: "muted" },
      ],
    },
  ];
</script>

<header>
  <h1>Theme</h1>
  <p>One look for every widget. Each widget can still have its own colour or frame (Desktop → Settings → Look).</p>
</header>

<h2>Start from</h2>
<div class="presets">
  {#each Object.entries(PRESETS) as [id, p]}
    <button class="preset" class:on={full.preset === id} onclick={() => pickPreset(id)}>
      <span class="swatch" style="background: {p.background}; opacity: {Math.max(p.opacity, 0.25)}; color: {p.text}">Aa</span>
      {p.name}
    </button>
  {/each}
</div>

<div class="examples">
  {#each SAMPLES as s}
    <Preview blocks={s.blocks} data={SAMPLE_DATA} manifest={s.manifest} {theme} width={220} height={170} />
  {/each}
</div>

<h2>Adjust {#if saved}<small class="saved">{saved}</small>{/if}</h2>
<div class="grid">
  <label>Background <input type="color" value={full.background} onchange={(e) => set("background", e.currentTarget.value)} /></label>
  <label>
    See-through
    <input type="range" min="0" max="1" step="0.01" value={1 - full.opacity} oninput={(e) => set("opacity", +(1 - e.currentTarget.value).toFixed(2))} />
    <span class="v">{Math.round((1 - full.opacity) * 100)}%</span>
  </label>
  <label>Text <input type="color" value={full.text} onchange={(e) => set("text", e.currentTarget.value)} /></label>
  <label>
    Corners
    <input type="range" min="0" max="28" step="1" value={full.radius} oninput={(e) => set("radius", +e.currentTarget.value)} />
    <span class="v">{full.radius}px</span>
  </label>
  <label class="inline"><input type="checkbox" checked={full.border} onchange={(e) => set("border", e.currentTarget.checked)} /> Thin border</label>
  <label class="inline"><input type="checkbox" checked={full.shadow} onchange={(e) => set("shadow", e.currentTarget.checked)} /> Text shadow (for see-through)</label>
  <label>
    Font
    <select value={full.font} onchange={(e) => set("font", e.currentTarget.value)}>
      <option value="system">System</option>
      <option value="rounded">Rounded</option>
      <option value="serif">Serif</option>
      <option value="mono">Monospace</option>
      <option value="condensed">Condensed</option>
    </select>
  </label>
  <label>
    Size
    <input type="range" min="0.75" max="1.5" step="0.05" value={full.scale} oninput={(e) => set("scale", +e.currentTarget.value)} />
    <span class="v">{Math.round(full.scale * 100)}%</span>
  </label>
  <label>
    Accent colour
    <select value={full.accent_mode} onchange={(e) => set("accent_mode", e.currentTarget.value)}>
      <option value="widget">Each widget's own</option>
      <option value="theme">The same for all</option>
    </select>
  </label>
  {#if full.accent_mode === "theme"}
    <label>Accent <input type="color" value={full.accent} onchange={(e) => set("accent", e.currentTarget.value)} /></label>
  {/if}
</div>
<p><button onclick={reset}>Back to the default look</button></p>
<p class="note">Fonts come from this computer; if a font is not installed, a similar one is used.</p>

<style>
  .presets { display: flex; flex-wrap: wrap; gap: 8px; }
  .preset { display: flex; align-items: center; gap: 8px; padding: 6px 12px 6px 6px; }
  .preset.on { border-color: #7cc4ff; background: #26304a; }
  .swatch { width: 34px; height: 26px; border-radius: 6px; display: grid; place-items: center; font-weight: 700; font-size: 12px; border: 1px solid #3a4560; }
  .examples {
    display: flex; flex-wrap: wrap; gap: 16px; margin: 16px 0; padding: 18px; border-radius: 12px;
    /* A stand-in desktop so see-through looks can be judged. */
    background: linear-gradient(135deg, #2b4a6f, #6d4c7d 50%, #c07a4f);
  }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(240px, 1fr)); gap: 12px 20px; }
  label { display: flex; align-items: center; gap: 8px; font-size: 13px; }
  label input[type="range"] { flex: 1; }
  input[type="color"] { width: 44px; height: 26px; border: 0; background: none; padding: 0; }
  .v { width: 42px; text-align: right; opacity: 0.7; font-variant-numeric: tabular-nums; }
  .saved { opacity: 0.6; font-weight: 400; margin-left: 8px; }
  .note { opacity: 0.55; font-size: 12px; }
</style>
