<script>
  // One widget window: loads its manifest and renders it (or a stack of
  // widgets), and handles dragging, resizing and the hover toolbar.
  import { onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { listen } from "@tauri-apps/api/event";
  import { api } from "../lib/api.js";
  import WidgetView from "./WidgetView.svelte";
  import StackView from "./StackView.svelte";
  import { look, lookVars } from "../lib/theme.js";

  const win = getCurrentWindow();
  const LAYERS = ["desktop", "normal", "top"];
  const LAYER_NAMES = { desktop: "On desktop", normal: "Normal", top: "Always on top" };

  let manifest = $state(null);
  let instance = $state(null);
  let children = $state([]);
  let error = $state("");

  // The app theme (Hub → Theme), this widget's colours and this widget's own overrides.
  let appTheme = $state({});
  let vars = $derived(manifest ? lookVars(look(appTheme, manifest, instance)) : "");

  // ---- edge drawer: closed shows a handle; hover or click slides it open ----
  let isDrawer = $derived(instance?.mode === "drawer");
  let edge = $derived(instance?.edge ?? "right");
  let drawerOpen = $state(false);
  let hoverTimer, leaveTimer;
  let drag = null;

  async function setDrawer(open) {
    clearTimeout(hoverTimer);
    clearTimeout(leaveTimer);
    if (open === drawerOpen) return;
    // Show the content as it slides in; hide it after it slides out.
    if (open) drawerOpen = true;
    try {
      await api.drawerSet(open);
    } catch (e) {
      error = String(e);
    }
    if (!open) drawerOpen = false;
  }
  function handleEnter() {
    clearTimeout(hoverTimer);
    hoverTimer = setTimeout(() => !drag && setDrawer(true), 350);
  }
  function handleLeave() {
    clearTimeout(hoverTimer);
  }
  // Dragging the handle moves the drawer along its edge; a click opens it.
  function handleDown(e) {
    if (e.button !== 0) return;
    try {
      e.currentTarget.setPointerCapture(e.pointerId);
    } catch {}
    drag = { x: e.screenX, y: e.screenY, moved: false, sent: 0 };
  }
  function handleMove(e) {
    if (!drag) return;
    const along = edge === "left" || edge === "right" ? e.screenY - drag.y : e.screenX - drag.x;
    if (!drag.moved && Math.abs(along) < 5) return;
    drag.moved = true;
    clearTimeout(hoverTimer);
    const step = along - drag.sent;
    if (Math.abs(step) >= 2) {
      drag.sent = along;
      api.drawerSlide(step).catch(() => {});
    }
  }
  function handleUp() {
    const wasClick = drag && !drag.moved;
    drag = null;
    if (wasClick) setDrawer(true);
  }
  // Closes when the pointer leaves, unless pinned or you are typing in it.
  function frameLeave() {
    if (!isDrawer || !drawerOpen || instance?.pinned) return;
    clearTimeout(leaveTimer);
    leaveTimer = setTimeout(() => {
      const typing = document.activeElement?.matches?.("input, textarea, select, [contenteditable]");
      if (!typing && !instance?.pinned) setDrawer(false);
    }, 700);
  }
  function frameEnter() {
    clearTimeout(leaveTimer);
  }
  async function togglePin() {
    // Use the answer at once; the instance-changed event follows anyway.
    const updated = await api.updateInstance(instance.id, { pinned: !instance.pinned }).catch((e) => (error = String(e)));
    if (updated?.id) instance = updated;
  }

  async function load() {
    try {
      const c = await api.widgetContext();
      manifest = c.manifest;
      instance = c.instance;
      children = c.children ?? [];
      appTheme = c.theme ?? {};
    } catch (e) {
      error = String(e);
    }
  }

  onMount(() => {
    load();
    const stops = [
      win.listen("instance-changed", (e) => {
        if (e.payload.id === instance?.id) instance = e.payload;
        else load(); // a widget inside this stack changed
      }),
      win.listen("stack-changed", load),
      // Clicking another app closes an open drawer (unless pinned).
      win.onFocusChanged(({ payload: focused }) => {
        if (!focused && isDrawer && drawerOpen && !instance?.pinned) setDrawer(false);
      }),
      listen("theme-changed", (e) => (appTheme = e.payload ?? {})),
    ];
    return () => stops.forEach((p) => p.then((f) => f()));
  });

  const INTERACTIVE = "button, input, textarea, select, a, label, iframe, tr.clickable, [data-nodrag]";
  function onPointerDown(e) {
    if (e.button !== 0 || instance?.locked || isDrawer) return;
    if (e.target.closest(INTERACTIVE)) return;
    win.startDragging();
  }

  function resize(e) {
    e.preventDefault();
    e.stopPropagation();
    win.startResizeDragging("SouthEast");
  }

  function close() {
    api.showInstance(instance.id, false);
  }
  function nextLayer() {
    const i = LAYERS.indexOf(instance.layer);
    api.updateInstance(instance.id, { layer: LAYERS[(i + 1) % LAYERS.length] });
  }
  function toggleLock() {
    api.updateInstance(instance.id, { locked: !instance.locked });
  }
</script>

{#if isDrawer && !drawerOpen && manifest && instance}
  <!-- A closed drawer: only the handle at the screen edge. -->
  <button
    class="handle edge-{edge}"
    style={vars}
    title="{manifest.name} — hover or click to open, drag to move along the edge"
    onpointerenter={handleEnter}
    onpointerleave={handleLeave}
    onpointerdown={handleDown}
    onpointermove={handleMove}
    onpointerup={handleUp}
  >
    <span class="grip-lines"></span>
  </button>
{:else}
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="frame"
  class:drawer={isDrawer}
  class:locked={instance?.locked}
  style={vars}
  onpointerdown={onPointerDown}
  onpointerleave={frameLeave}
  onpointerenter={frameEnter}
>
  {#if error}
    <div class="error">{error}</div>
  {:else if manifest && instance}
    <div class="content">
      {#if manifest.ui?.stack}
        <StackView {manifest} {instance} {children} />
      {:else}
        {#key instance.id}
          <WidgetView {manifest} {instance} />
        {/key}
      {/if}
    </div>

    <div class="toolbar" data-nodrag>
      {#if isDrawer}
        <button title={instance.pinned ? "Unpin (closes when you move away)" : "Pin open"} class:on={instance.pinned} onclick={togglePin}>📌</button>
        <button title="Slide closed" onclick={() => setDrawer(false)}>{({ left: "◀", right: "▶", top: "▲", bottom: "▼" })[edge]}</button>
      {/if}
      <button title="Close" onclick={close}>✕</button>
      <button title="Settings" onclick={() => api.openHub(instance.id)}>⚙</button>
      {#if !isDrawer}<button title="Layer: {LAYER_NAMES[instance.layer]}" onclick={nextLayer}>
        {instance.layer === "top" ? "▲" : instance.layer === "desktop" ? "▼" : "◆"}
      </button>{/if}
      <button title={instance.locked ? "Unlock position" : "Lock position"} onclick={toggleLock}>
        {instance.locked ? "🔒" : "🔓"}
      </button>
    </div>

    {#if !instance.locked && manifest.window?.resizable !== false}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="grip" data-nodrag onpointerdown={resize}></div>
    {/if}
  {/if}
</div>
{/if}

<style>
  :global(html, body) {
    height: 100%;
  }
  :global(body) {
    font-size: 13px;
    line-height: 1.35;
    user-select: none;
  }
  .frame {
    position: fixed;
    inset: 0;
    box-sizing: border-box;
    border-radius: var(--wf-radius, 14px);
    overflow: hidden;
    cursor: default;
    background: var(--wf-bg, rgba(16, 21, 32, 0.78));
    border: 1px solid var(--wf-border, transparent);
    color: var(--wf-text, #eef2f8);
    font-family: var(--wf-font, system-ui, sans-serif);
    text-shadow: var(--wf-shadow, none);
  }
  .content {
    zoom: var(--wf-scale, 1);
    position: absolute;
    inset: 0;
    padding: 14px;
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .toolbar {
    position: absolute;
    top: 6px;
    right: 6px;
    display: flex;
    gap: 2px;
    padding: 2px;
    border-radius: 8px;
    background: rgba(0, 0, 0, 0.55);
    opacity: 0;
    transition: opacity 0.15s;
  }
  .frame:hover .toolbar {
    opacity: 1;
  }
  .toolbar button {
    width: 22px;
    height: 22px;
    padding: 0;
    border: 0;
    border-radius: 6px;
    background: transparent;
    color: #fff;
    font-size: 11px;
    cursor: pointer;
  }
  .toolbar button:hover {
    background: rgba(255, 255, 255, 0.18);
  }
  .grip {
    position: absolute;
    right: 0;
    bottom: 0;
    width: 14px;
    height: 14px;
    cursor: nwse-resize;
    background: linear-gradient(135deg, transparent 50%, rgba(255, 255, 255, 0.25) 50%);
    opacity: 0;
  }
  .frame:hover .grip {
    opacity: 1;
  }
  .handle {
    position: fixed;
    inset: 0;
    padding: 0;
    border: 1px solid var(--wf-border, transparent);
    background: var(--wf-bg, rgba(16, 21, 32, 0.78));
    cursor: pointer;
    display: grid;
    place-items: center;
    touch-action: none;
  }
  .handle.edge-right { border-radius: 9px 0 0 9px; border-right: 0; }
  .handle.edge-left { border-radius: 0 9px 9px 0; border-left: 0; }
  .handle.edge-top { border-radius: 0 0 9px 9px; border-top: 0; }
  .handle.edge-bottom { border-radius: 9px 9px 0 0; border-bottom: 0; }
  .handle:hover { background: color-mix(in srgb, var(--accent) 35%, var(--wf-bg, #101520)); }
  .grip-lines { background: var(--accent); border-radius: 2px; opacity: 0.9; }
  .edge-left .grip-lines, .edge-right .grip-lines { width: 4px; height: 46px; }
  .edge-top .grip-lines, .edge-bottom .grip-lines { width: 46px; height: 4px; }
  .toolbar button.on { background: color-mix(in srgb, var(--accent) 45%, transparent); }
  .error {
    padding: 12px;
    color: var(--wf-bad, #ffb4b4);
    font-size: 12px;
  }
</style>
