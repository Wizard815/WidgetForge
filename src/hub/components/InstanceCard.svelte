<script lang="ts">
  import { hub } from "../../lib/api";
  import type { Edge, InstanceInfo, Layer, Mode } from "../../lib/types";
  import { act, app, manifestFor } from "../store.svelte";
  import Icon from "./Icon.svelte";
  import Segmented from "./Segmented.svelte";
  import SettingsForm from "./SettingsForm.svelte";
  import Toggle from "./Toggle.svelte";
  import WidgetIcon from "./WidgetIcon.svelte";

  let { inst }: { inst: InstanceInfo } = $props();
  const m = $derived(manifestFor(inst.widget_id));
  let expanded = $state(false);
  let el: HTMLElement | undefined = $state();

  // The widget's gear button opens its card here.
  $effect(() => {
    if (app.focus === inst.id) {
      expanded = true;
      el?.scrollIntoView({ behavior: "smooth", block: "center" });
      app.focus = null;
    }
  });

  const subtitle = $derived.by(() => {
    const label = inst.settings.label ?? inst.settings.title;
    return typeof label === "string" && label ? label : (m?.category ?? "");
  });

  const update = (patch: Record<string, unknown>) => act(() => hub.updateInstance(inst.id, patch));

  const LAYERS: { value: Layer; label: string; title: string }[] = [
    { value: "top", label: "On top", title: "Stays above every other window" },
    { value: "normal", label: "Normal", title: "Comes forward when clicked, goes behind other apps" },
    { value: "desktop", label: "Desktop", title: "Sits on the desktop behind all windows" },
  ];
  const MODES: { value: Mode; label: string }[] = [
    { value: "tile", label: "Floating tile" },
    { value: "drawer", label: "Edge drawer" },
  ];
  const EDGES: { value: Edge; label: string }[] = [
    { value: "left", label: "Left" },
    { value: "right", label: "Right" },
    { value: "top", label: "Top" },
    { value: "bottom", label: "Bottom" },
  ];

  async function remove() {
    if (!m) return act(() => hub.removeInstance(inst.id));
    const msg = `Remove this ${m.name}? Its settings${m.permissions.storage ? " and saved content" : ""} will be deleted.`;
    let ok = true;
    try {
      const { ask } = await import("@tauri-apps/plugin-dialog");
      ok = await ask(msg, { title: "Remove widget", kind: "warning" });
    } catch {
      ok = confirm(msg);
    }
    if (ok) await act(() => hub.removeInstance(inst.id), `${m.name} removed`);
  }
</script>

<article class="card inst" class:expanded bind:this={el}>
  <header>
    <button class="main" onclick={() => (expanded = !expanded)} aria-expanded={expanded}>
      {#if m}<WidgetIcon manifest={m} size={40} />{/if}
      <div class="title">
        <div class="name">{m?.name ?? inst.widget_id}</div>
        <div class="sub">{subtitle}</div>
      </div>
    </button>
    <div class="right">
      {#if inst.missing}
        <span class="chip danger">Widget missing</span>
      {:else}
        <span class="chip" class:ok={inst.open}>{inst.open ? "On screen" : "Closed"}</span>
        <span class="chip">{LAYERS.find((l) => l.value === inst.layer)?.label}</span>
        <button class="btn small" onclick={() => act(() => hub.setOpen(inst.id, !inst.open))}>
          <Icon name={inst.open ? "eye-off" : "eye"} size={15} />
          {inst.open ? "Close" : "Show"}
        </button>
      {/if}
      <button class="chev" onclick={() => (expanded = !expanded)} aria-label="Details">
        <Icon name="chevron" size={16} />
      </button>
    </div>
  </header>

  {#if expanded}
    <div class="body">
      {#if m}
        <section>
          <h3>Placement</h3>
          <div class="row">
            <div>
              <div class="label">Layer</div>
              <div class="hint">How it stacks with your other windows</div>
            </div>
            <Segmented value={inst.layer} options={LAYERS} onchange={(v) => update({ layer: v })} />
          </div>
          <div class="row">
            <div>
              <div class="label">Shape</div>
              <div class="hint">A drawer hides at a screen edge and slides out</div>
            </div>
            <div class="inline">
              <Segmented value={inst.mode} options={MODES} onchange={(v) => update({ mode: v })} />
              {#if inst.mode === "drawer"}
                <select value={inst.edge} onchange={(e) => update({ edge: e.currentTarget.value })}>
                  {#each EDGES as e (e.value)}<option value={e.value}>{e.label} edge</option>{/each}
                </select>
              {/if}
            </div>
          </div>
          <div class="row">
            <div>
              <div class="label">Opacity</div>
              <div class="hint">{Math.round(inst.opacity * 100)}%</div>
            </div>
            <input
              type="range"
              min="0.2"
              max="1"
              step="0.05"
              value={inst.opacity}
              onchange={(e) => update({ opacity: e.currentTarget.valueAsNumber })} />
          </div>
        </section>

        <section>
          <h3>Behaviour</h3>
          <Toggle
            label="Start with WidgetForge"
            hint="Opens at this position when WidgetForge starts"
            checked={inst.start_with_app}
            onchange={(v) => update({ start_with_app: v })} />
          <Toggle
            label="Taskbar button"
            hint="Give this widget its own button in the taskbar"
            checked={inst.show_in_taskbar}
            onchange={(v) => update({ show_in_taskbar: v })} />
          <Toggle label="Lock position" hint="Prevent dragging and resizing" checked={inst.locked} onchange={(v) => update({ locked: v })} />
        </section>

        <section>
          <h3>Widget settings</h3>
          <SettingsForm defs={m.settings} values={inst.settings} onchange={(k, v) => update({ settings: { [k]: v } })} />
        </section>
      {/if}

      <footer>
        <span class="faint small mono">{inst.widget_id}</span>
        <button class="btn small danger" onclick={remove}><Icon name="trash" size={14} /> Remove</button>
      </footer>
    </div>
  {/if}
</article>

<style>
  .inst {
    overflow: hidden;
    transition: border-color 0.15s;
  }
  .inst.expanded {
    border-color: var(--border-strong);
  }
  header {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 14px;
  }
  .main {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 12px;
    min-width: 0;
    border: 0;
    background: none;
    padding: 0;
    cursor: pointer;
    text-align: left;
  }
  .title {
    min-width: 0;
  }
  .name {
    font-weight: 650;
  }
  .sub {
    font-size: 12.5px;
    color: var(--muted);
    text-transform: capitalize;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .right {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .chev {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    border: 0;
    border-radius: 8px;
    background: none;
    color: var(--muted);
    cursor: pointer;
    transition: transform 0.15s;
  }
  .chev:hover {
    background: var(--hover);
  }
  .expanded .chev {
    transform: rotate(90deg);
  }
  .body {
    border-top: 1px solid var(--border);
    padding: 4px 18px 14px 66px;
  }
  section {
    padding: 12px 0;
    border-bottom: 1px solid var(--border);
  }
  section h3 {
    font-size: 12px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--faint);
    margin-bottom: 6px;
  }
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 8px 0;
  }
  .label {
    font-weight: 500;
  }
  .hint {
    font-size: 12px;
    color: var(--muted);
  }
  .inline {
    display: flex;
    gap: 8px;
    align-items: center;
  }
  input[type="range"] {
    width: 220px;
  }
  footer {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding-top: 12px;
  }
</style>
