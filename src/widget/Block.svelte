<script>
  // Renders one declarative block (DESIGN.md §5.3) and, for layout blocks, its children.
  import { getContext } from "svelte";
  import { bind, bindDeep, isVisible } from "../lib/bindings.js";
  import { api, widgetFile } from "../lib/api.js";
  import Block from "./Block.svelte";
  import Graph from "./blocks/Graph.svelte";
  import Clock from "./blocks/Clock.svelte";
  import Notes from "./blocks/Notes.svelte";
  import Checklist from "./blocks/Checklist.svelte";
  import Timer from "./blocks/Timer.svelte";
  import Calculator from "./blocks/Calculator.svelte";
  import Month from "./blocks/Month.svelte";
  import Slideshow from "./blocks/Slideshow.svelte";
  import Mixer from "./blocks/Mixer.svelte";
  import Habits from "./blocks/Habits.svelte";
  import Radio from "./blocks/Radio.svelte";
  import Converter from "./blocks/Converter.svelte";
  import Chart from "./blocks/Chart.svelte";
  import Reminders from "./blocks/Reminders.svelte";
  import Input from "./blocks/Input.svelte";

  let { block, ctx } = $props();
  const forge = getContext("forge");

  const SIZES = { xs: 11, sm: 12, md: 14, lg: 17, xl: 22, xxl: 34 };
  const bindIn = (value, scope, fallback) => {
    const v = bind(value, scope);
    return v === undefined || v === null || v === "" ? fallback : v;
  };
  const b = (key, fallback) => bindIn(block[key], ctx, fallback);
  const colorOf = (c) =>
    c === "muted" ? "var(--wf-muted)" : c === "accent" ? "var(--accent)"
    : c === "good" ? "var(--wf-good)" : c === "bad" ? "var(--wf-bad)" : c === "warn" ? "var(--wf-warn)" : c;
  const pct = (value, max) => Math.max(0, Math.min(100, (Number(value) / (Number(max) || 1)) * 100 || 0));

  /** Runs a button or row action: open a link, copy text, or a named action. */
  async function run(action, scope) {
    try {
      const url = action.open && bind(action.open, scope);
      if (url) return await api.openUrl(String(url));
      const copy = action.copy !== undefined ? bind(action.copy, scope) : undefined;
      if (copy !== undefined && copy !== null) return await api.clipboardCopy(forge.instanceId, String(copy));
      const name = action.action && bind(action.action, scope);
      if (!name) return;
      const arg = action.arg !== undefined ? bind(action.arg, scope) : undefined;
      if (bind(action.confirm, scope) && !(await confirmClick(action, scope))) return;
      if (name.startsWith("media.")) await api.mediaControl(forge.instanceId, name.slice(6));
      else if (name.startsWith("docker.")) await api.dockerAction(forge.instanceId, String(arg), name.slice(7));
      else if (name === "clipboard.clear") await api.clipboardClear(forge.instanceId);
      else if (name.startsWith("fetch.")) {
        // Runs a `manual: true` data source (a toggle, a pause, a translation).
        await forge.runSource(name.slice(6), arg);
        flash(action, scope);
        return;
      } else if (name.startsWith("run.")) {
        const r = await api.runCommand(forge.instanceId, name.slice(4));
        if (!r.configured) throw "Set this button's command in the widget's settings.";
        if (!r.ok) throw (r.stderr || r.stdout || `exit code ${r.code}`).split("\n")[0];
      } else if (name.startsWith("wol.")) await api.wakeOnLan(forge.instanceId, name.slice(4));
      else if (name === "screentime.clear") await api.screenTimeClear(forge.instanceId);
      else if (name === "open_item") await api.openFolderItem(forge.instanceId, bind(action.setting ?? "folder", scope), String(arg));
      flash(action, scope);
      // Every action refreshes the data, so the widget shows the result at once.
      forge.refresh();
    } catch (e) {
      forge.report(String(e));
    }
  }

  // Buttons marked `confirm: true` need a second click within 3 seconds.
  let armed = $state(null);
  function confirmClick(action, scope) {
    const key = JSON.stringify([action.action, action.arg !== undefined ? bind(action.arg, scope) : null]);
    if (armed === key) {
      armed = null;
      return true;
    }
    armed = key;
    setTimeout(() => armed === key && (armed = null), 3000);
    return false;
  }
  // A short "✓" on the button that just worked.
  let done = $state(null);
  function flash(action, scope) {
    const key = JSON.stringify([action.action, action.arg !== undefined ? bind(action.arg, scope) : null]);
    done = key;
    setTimeout(() => done === key && (done = null), 1200);
  }
  const isDone = (action, scope) =>
    done === JSON.stringify([action.action, action.arg !== undefined ? bind(action.arg, scope) : null]);
  const isArmed = (action, scope) =>
    armed === JSON.stringify([action.action, action.arg !== undefined ? bind(action.arg, scope) : null]);

  let visible = $derived(isVisible(block, ctx));
  let kids = $derived(block.children ?? []);
</script>

{#if visible}
  {#if block.type === "column" || block.type === "row"}
    <div
      class="flex"
      class:row={block.type === "row"}
      style="flex-direction: {block.type}; gap: {b('gap', 8)}px; align-items: {block.type === 'row' ? 'center' : 'stretch'}"
    >
      {#each kids as child}<Block block={child} {ctx} />{/each}
    </div>
  {:else if block.type === "grid"}
    <div class="grid" style="grid-template-columns: repeat({b('columns', 2)}, minmax(0, 1fr)); gap: {b('gap', 8)}px">
      {#each kids as child}<Block block={child} {ctx} />{/each}
    </div>
  {:else if block.type === "each"}
    <!-- Repeats `child` (or `children`) for every item; read it as {{ item.* }}. -->
    {@const items = b("items", [])}
    {@const list = (Array.isArray(items) ? items : []).slice(0, Number(b("limit", 100)))}
    {#if list.length === 0 && block.empty}
      <div class="empty">{b("empty", "")}</div>
    {:else}
      <div class="grid" style="grid-template-columns: repeat({b('columns', 1)}, minmax(0, 1fr)); gap: {b('gap', 6)}px">
        {#each list as item, index}
          {@const scope = { ...ctx, item, index }}
          {#if block.child}
            <Block block={block.child} ctx={scope} />
          {:else}
            <div class="flex" style="flex-direction: column; gap: 4px">
              {#each kids as child}<Block block={child} ctx={scope} />{/each}
            </div>
          {/if}
        {/each}
      </div>
    {/if}
  {:else if block.type === "input"}
    <Input
      name={block.name ?? "text"}
      placeholder={b("placeholder", "")}
      submit={block.submit}
      button={b("button", "")}
      options={b("options", null)}
      initial={b("value", "")}
      multiline={b("multiline", false)}
    />
  {:else if block.type === "habits"}
    <Habits key={b("key", "habits")} days={Number(b("days", 7))} />
  {:else if block.type === "radio"}
    <Radio server={b("server", "")} />
  {:else if block.type === "converter"}
    <Converter rates={b("rates", null)} base={b("base", "EUR")} />
  {:else if block.type === "chart"}
    <Chart
      items={b("items", [])}
      value={block.value ?? "value"}
      second={block.second}
      label={b("label", "")}
      format={b("format", "")}
      color={colorOf(b("color", "accent"))}
      secondColor={colorOf(b("second_color", "muted"))}
      kind={b("kind", "bars")}
    />
  {:else if block.type === "reminders"}
    <Reminders list={bindDeep(block.list ?? [], ctx)} notify={b("notify", true)} sound={b("sound", true)} />
  {:else if block.type === "spacer"}
    <div style="flex: 1; min-height: {b('size', 0)}px"></div>
  {:else if block.type === "divider"}
    <hr class="divider" />
  {:else if block.type === "text"}
    <div
      class="text"
      class:mono={b("mono", false)}
      class:clip={b("clip", false)}
      style="font-size: {SIZES[b('size', 'md')] ?? 14}px; font-weight: {b('weight', 'normal')};
             text-align: {b('align', 'left')}; color: {colorOf(b('color', 'inherit'))}"
    >
      {b("text", "")}
    </div>
  {:else if block.type === "stat"}
    <div class="stat" style="text-align: {b('align', 'left')}">
      <div class="label">{b("label", "")}</div>
      <div class="stat-value" style="color: {colorOf(b('color', 'inherit'))}; font-size: {SIZES[b('size', 'xl')] ?? 22}px">
        {b("value", "–")}
      </div>
      {#if block.note}<div class="note">{b("note", "")}</div>{/if}
    </div>
  {:else if block.type === "gauge"}
    {@const p = pct(b("value", 0), b("max", 100))}
    {@const r = 34}
    {@const c = 2 * Math.PI * r}
    <div class="gauge">
      <svg viewBox="0 0 84 84">
        <circle cx="42" cy="42" {r} class="track" />
        <circle
          cx="42" cy="42" {r} class="fill"
          stroke={colorOf(b("color", "accent"))}
          stroke-dasharray="{(p / 100) * c} {c}"
          transform="rotate(-90 42 42)"
        />
        <text x="42" y="46" text-anchor="middle">{b("format", Math.round(p) + "%")}</text>
      </svg>
      <div class="label">{b("label", "")}</div>
    </div>
  {:else if block.type === "bar"}
    {@const p = pct(b("value", 0), b("max", 100))}
    <div class="bar">
      <div class="top"><span class="label">{b("label", "")}</span><span>{b("format", Math.round(p) + "%")}</span></div>
      <div class="track"><div class="fill" style="width: {p}%; background: {colorOf(b('color', 'accent'))}"></div></div>
    </div>
  {:else if block.type === "graph"}
    <Graph
      value={Number(b("value", 0))}
      max={b("max", 100) === "auto" ? "auto" : Number(b("max", 100))}
      label={b("label", "")}
      format={b("format", "")}
      color={colorOf(b("color", "accent"))}
      tick={ctx._tick}
    />
  {:else if block.type === "list"}
    {@const items = b("items", [])}
    <ul class="list">
      {#each Array.isArray(items) ? items : [] as item}
        <li>{typeof item === "object" ? (item.text ?? JSON.stringify(item)) : item}</li>
      {/each}
    </ul>
  {:else if block.type === "table"}
    {@const rows = b("items", [])}
    {@const cols = block.columns ?? []}
    {@const list = (Array.isArray(rows) ? rows : []).slice(0, Number(b("limit", 50)))}
    {#if list.length === 0 && block.empty}
      <div class="empty">{b("empty", "")}</div>
    {:else}
      <table class="table" class:headless={block.header === false}>
        {#if block.header !== false}
          <thead>
            <tr>{#each cols as col}<th style="text-align: {col.align ?? 'left'}; width: {col.width ?? 'auto'}">{bind(col.label ?? "", ctx)}</th>{/each}</tr>
          </thead>
        {/if}
        <tbody>
          {#each list as row, index}
            {@const scope = { ...ctx, row, index }}
            <tr
              class:clickable={!!(block.row_open || block.row_copy)}
              onclick={() => (block.row_open || block.row_copy) && run({ open: block.row_open, copy: block.row_copy }, scope)}
              title={block.row_title ? bind(block.row_title, scope) : undefined}
            >
              {#each cols as col}
                <td
                  style="text-align: {col.align ?? 'left'}; width: {col.width ?? 'auto'}; color: {colorOf(bindIn(col.color, scope, 'inherit'))}"
                  class:wrap={col.wrap}
                >
                  {#if col.bar !== undefined}
                    <div class="mini-bar" title={bindIn(col.value ?? "", scope, "")}>
                      <div style="width: {pct(bindIn(col.bar, scope, 0), bindIn(col.max ?? 100, scope, 100))}%; background: {colorOf(bindIn(col.bar_color, scope, 'accent'))}"></div>
                    </div>
                  {:else if col.actions}
                    <span class="row-actions">
                      {#each col.actions as a}
                        {#if bindIn(a.visible, scope, true)}
                          <button
                            class="mini"
                            class:armed={isArmed(a, scope)}
                            title={bindIn(a.title, scope, "")}
                            onclick={(e) => { e.stopPropagation(); run(a, scope); }}
                          >{isArmed(a, scope) ? "Sure?" : bindIn(a.label, scope, "")}</button>
                        {/if}
                      {/each}
                    </span>
                  {:else}
                    {bindIn(col.value ?? "", scope, "")}
                  {/if}
                </td>
              {/each}
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  {:else if block.type === "image"}
    {@const src = String(b("src", ""))}
    <img
      class="image"
      alt={b("alt", "")}
      src={/^(https?:|data:)/.test(src) ? src : widgetFile(forge.widgetId, src)}
      style="object-fit: {b('fit', 'contain')}"
    />
  {:else if block.type === "clock"}
    <Clock seconds={b("seconds", true)} timezone={b("timezone", "")} />
  {:else if block.type === "button"}
    <button
      class="button"
      class:big={b("size", "") === "lg"}
      class:tile={b("style", "") === "tile"}
      class:on={b("on", false)}
      class:armed={isArmed(block, ctx)}
      class:done={isDone(block, ctx)}
      title={b("title", "")}
      onclick={() => run(block, ctx)}
    >
      {#if block.icon}<span class="icon">{b("icon", "")}</span>{/if}
      <span class="button-label">{isArmed(block, ctx) ? "Sure?" : isDone(block, ctx) ? "✓" : b("label", "Button")}</span>
      {#if block.note}<span class="button-note">{b("note", "")}</span>{/if}
    </button>
  {:else if block.type === "notes"}
    <Notes key={b("key", "notes")} placeholder={b("placeholder", "")} paper={b("paper", "")} />
  {:else if block.type === "checklist"}
    <Checklist key={b("key", "items")} placeholder={b("placeholder", "")} />
  {:else if block.type === "timer"}
    <Timer
      mode={b("mode", "countdown")}
      minutes={Number(b("minutes", 5))}
      work={Number(b("work", 25))}
      rest={Number(b("rest", 5))}
      longRest={Number(b("long_rest", 15))}
      labels={{ work: b("work_label", "Focus"), rest: b("rest_label", "Break") }}
    />
  {:else if block.type === "calculator"}
    <Calculator />
  {:else if block.type === "month"}
    <Month events={b("events", [])} mondayFirst={b("week_starts_monday", true)} />
  {:else if block.type === "slideshow"}
    <Slideshow
      images={b("images", [])}
      seconds={Number(b("seconds", 10))}
      fit={b("fit", "cover")}
      shuffle={b("shuffle", false)}
      captions={b("captions", false)}
      empty={b("empty", "No pictures yet.")}
    />
  {:else if block.type === "mixer"}
    <Mixer audio={b("audio", null)} showApps={b("apps", true)} showDevices={b("devices", true)} />
  {:else}
    <div class="unknown">Unknown block “{block.type}”</div>
  {/if}
{/if}

<style>
  .flex { display: flex; min-width: 0; }
  .flex.row > :global(*) { flex: 1 1 0; min-width: 0; }
  .grid { display: grid; }
  .text { white-space: pre-wrap; overflow-wrap: anywhere; }
  .text.clip { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .mono { font-family: ui-monospace, "Cascadia Mono", monospace; font-variant-numeric: tabular-nums; }
  .label { opacity: 0.75; font-size: 12px; }
  .note { opacity: 0.55; font-size: 11px; }
  .stat-value { font-weight: 700; font-variant-numeric: tabular-nums; line-height: 1.2; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .divider { border: 0; border-top: 1px solid color-mix(in srgb, currentColor 10%, transparent); margin: 2px 0; width: 100%; }
  .gauge { display: flex; flex-direction: column; align-items: center; gap: 2px; }
  .gauge svg { width: 100%; max-width: 110px; }
  .gauge .track { fill: none; stroke: color-mix(in srgb, currentColor 12%, transparent); stroke-width: 8; }
  .gauge .fill { fill: none; stroke-width: 8; stroke-linecap: round; transition: stroke-dasharray 0.5s; }
  .gauge text { fill: currentColor; font-size: 15px; font-weight: 600; }
  .bar .top { display: flex; justify-content: space-between; margin-bottom: 4px; font-size: 12px; gap: 8px; }
  .bar .top span:last-child { font-variant-numeric: tabular-nums; white-space: nowrap; }
  .bar .track { height: 7px; border-radius: 4px; background: color-mix(in srgb, currentColor 12%, transparent); overflow: hidden; }
  .bar .fill { height: 100%; border-radius: 4px; transition: width 0.5s; }
  .list { margin: 0; padding-left: 18px; }
  .empty { opacity: 0.55; font-size: 12px; padding: 6px 0; }
  .table { width: 100%; border-collapse: collapse; font-size: 12px; table-layout: fixed; }
  .table th { font-weight: 600; opacity: 0.6; padding: 0 4px 4px; }
  .table td {
    padding: 4px;
    border-top: 1px solid color-mix(in srgb, currentColor 6%, transparent);
    font-variant-numeric: tabular-nums;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .table.headless tr:first-child td { border-top: 0; }
  .table td.wrap { white-space: normal; overflow-wrap: anywhere; }
  .table tr.clickable { cursor: pointer; }
  .table tr.clickable:hover td { background: color-mix(in srgb, currentColor 6%, transparent); }
  .image { width: 100%; height: 100%; min-height: 0; }
  .mini-bar { height: 6px; border-radius: 3px; background: color-mix(in srgb, currentColor 12%, transparent); overflow: hidden; }
  .mini-bar > div { height: 100%; border-radius: 3px; }
  .row-actions { display: inline-flex; gap: 3px; }
  .mini {
    padding: 1px 6px; font: inherit; font-size: 11px; color: inherit; cursor: pointer;
    border: 1px solid color-mix(in srgb, currentColor 16%, transparent); border-radius: 5px; background: color-mix(in srgb, currentColor 6%, transparent);
  }
  .mini:hover { border-color: var(--accent); }
  .armed { border-color: var(--wf-warn) !important; color: var(--wf-warn); }
  .button {
    padding: 8px 6px;
    border: 1px solid color-mix(in srgb, currentColor 16%, transparent);
    border-radius: 8px;
    background: color-mix(in srgb, currentColor 8%, transparent);
    color: inherit;
    font: inherit;
    cursor: pointer;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .button.big { font-size: 18px; padding: 10px 6px; }
  .button { display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 2px; min-width: 0; }
  .button.tile { min-height: 64px; }
  .button.on { background: color-mix(in srgb, var(--accent) 30%, transparent); border-color: var(--accent); }
  .button.done { border-color: var(--wf-good); }
  .button .icon { font-size: 22px; line-height: 1; }
  .button-label { max-width: 100%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .button-note { font-size: 10px; opacity: 0.65; max-width: 100%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .button:hover { background: color-mix(in srgb, currentColor 16%, transparent); border-color: var(--accent); }
  .unknown { color: var(--wf-bad); font-size: 12px; }
</style>
