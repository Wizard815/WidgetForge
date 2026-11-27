<script>
  // A widget drawn in the Hub with sample data: the builder's live preview
  // and the theme page's examples. Uses the same blocks as real widgets.
  import { setContext } from "svelte";
  import Block from "../widget/Block.svelte";
  import { look, lookVars } from "../lib/theme.js";

  let { blocks = [], data = {}, settings = {}, manifest = {}, theme = {}, instance = {}, width = 260, height = 160, tick = 0, selected = null, onpick = null } = $props();

  // Buttons and saved blocks do nothing here; it is only a picture.
  setContext("forge", {
    instanceId: "builder-preview",
    widgetId: "builder-preview",
    refresh() {},
    report() {},
    runSource: async () => {},
    setInput() {},
  });

  let vars = $derived(lookVars(look(theme, manifest, instance)));
  let ctx = $derived({ ...data, settings, input: {}, lookups: manifest.lookups ?? {}, _tick: tick, _now: Date.now(), _errors: {} });
</script>

<div class="frame" style="{vars}; width: {width}px; height: {height}px">
  <div class="content">
    {#each blocks as block, i}
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="pick" class:sel={selected && selected.length === 1 && selected[0] === i} onclick={() => onpick?.([i])}>
        <Block {block} {ctx} />
      </div>
    {/each}
  </div>
</div>

<style>
  .frame {
    position: relative;
    box-sizing: border-box;
    border-radius: var(--wf-radius);
    overflow: hidden;
    background: var(--wf-bg);
    border: 1px solid var(--wf-border);
    color: var(--wf-text);
    font-family: var(--wf-font);
    font-size: 13px;
    line-height: 1.35;
    text-shadow: var(--wf-shadow);
    flex: none;
  }
  .content {
    position: absolute;
    inset: 0;
    padding: 14px;
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: 8px;
    zoom: var(--wf-scale);
  }
  .pick { display: contents; }
  .pick.sel > :global(*) { outline: 1px dashed var(--accent); outline-offset: 2px; }
</style>
