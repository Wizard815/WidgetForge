<script>
  // A stack: several widgets shown one under another in a single window.
  // Each keeps its own settings, data and storage; the Hub adds and removes them.
  import { api } from "../lib/api.js";
  import { effectiveSettings } from "../lib/bindings.js";
  import WidgetView from "./WidgetView.svelte";

  let { manifest, instance, children } = $props();
  let settings = $derived(effectiveSettings(manifest, instance));

  function titleOf(c) {
    const s = c.instance.settings ?? {};
    return s.label || s.title || c.manifest.name;
  }
</script>

{#if settings.title}
  <div class="stack-title">{settings.title}</div>
{/if}

{#if children.length === 0}
  <div class="empty">
    This stack is empty. Open WidgetForge → Desktop and choose this stack in a widget's “Stack” menu.
    <button onclick={() => api.openHub(instance.id)}>Open WidgetForge</button>
  </div>
{/if}

{#each children as c, i (c.instance.id)}
  {#if i > 0}<hr />{/if}
  <section style="--accent: {c.manifest.style?.accent ?? '#7cc4ff'}">
    {#if settings.show_names !== false && c.manifest.ui?.stack_title !== false}
      <div class="child-title">{titleOf(c)}</div>
    {/if}
    <WidgetView manifest={c.manifest} instance={c.instance} inStack />
  </section>
{/each}

<style>
  .stack-title { font-weight: 700; font-size: 15px; }
  .child-title {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    opacity: 0.55;
    margin-bottom: 6px;
  }
  section { display: flex; flex-direction: column; }
  hr { border: 0; border-top: 1px solid color-mix(in srgb, currentColor 10%, transparent); margin: 4px 0; width: 100%; }
  .empty { font-size: 12px; opacity: 0.75; display: flex; flex-direction: column; gap: 8px; align-items: flex-start; }
  .empty button {
    border: 1px solid color-mix(in srgb, currentColor 20%, transparent); background: color-mix(in srgb, currentColor 8%, transparent);
    color: inherit; border-radius: 7px; padding: 5px 10px; cursor: pointer; font: inherit;
  }
</style>
