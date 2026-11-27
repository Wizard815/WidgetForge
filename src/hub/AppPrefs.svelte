<script>
  // Hub → Settings: start at sign-in and global keyboard shortcuts.
  // PLACEHOLDER wording.
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";

  let prefs = $state(null);
  let toggle = $state("");
  let hub = $state("");
  let message = $state("");
  let error = $state("");

  async function load() {
    prefs = await invoke("get_app_settings");
    toggle = prefs.shortcut_toggle;
    hub = prefs.shortcut_hub;
  }

  async function setAutostart(on) {
    error = message = "";
    try {
      const now = await invoke("set_autostart", { enabled: on });
      message = now ? "WidgetForge will start (in the tray) when you sign in." : "WidgetForge will not start at sign-in.";
    } catch (e) {
      error = String(e);
    }
    await load();
  }

  async function saveShortcuts() {
    error = message = "";
    try {
      prefs = await invoke("set_shortcuts", { toggle, hub });
      message = prefs.shortcut_error ? "" : "Shortcuts saved.";
    } catch (e) {
      error = String(e);
    }
  }

  // Turns a key press into text like "Ctrl+Alt+W".
  const NAMES = { " ": "Space", ArrowUp: "Up", ArrowDown: "Down", ArrowLeft: "Left", ArrowRight: "Right" };
  function capture(e, set) {
    e.preventDefault();
    if (e.key === "Backspace" || e.key === "Delete") return set("");
    if (["Control", "Alt", "Shift", "Meta"].includes(e.key)) return;
    const mods = [];
    if (e.ctrlKey) mods.push("Ctrl");
    if (e.altKey) mods.push("Alt");
    if (e.shiftKey) mods.push("Shift");
    if (e.metaKey) mods.push("Super");
    if (mods.length === 0 && !/^F\d+$/.test(e.key)) {
      error = "Use at least one of Ctrl, Alt, Shift or Super with a letter or number.";
      return;
    }
    error = "";
    let key = NAMES[e.key] ?? e.key;
    if (/^Key[A-Z]$/.test(e.code)) key = e.code.slice(3);
    else if (/^Digit\d$/.test(e.code)) key = e.code.slice(5);
    set([...mods, key.length === 1 ? key.toUpperCase() : key].join("+"));
  }

  onMount(load);
</script>

<h2>Startup</h2>
<div class="box">
  <label class="check">
    <input type="checkbox" checked={prefs?.autostart} disabled={!prefs} onchange={(e) => setAutostart(e.currentTarget.checked)} />
    Start WidgetForge when I sign in
  </label>
  <p class="muted">It starts quietly in the tray and opens only the widgets marked “Start with app”. Steam is not needed.</p>
</div>

<h2>Keyboard shortcuts</h2>
<div class="box">
  <p class="muted">Click a box and press the keys. Backspace clears it. These work from any program.</p>
  <div class="grid">
    <span>Show or hide all widgets</span>
    <input readonly value={toggle} placeholder="None" onkeydown={(e) => capture(e, (v) => (toggle = v))} />
    <span>Open WidgetForge</span>
    <input readonly value={hub} placeholder="None" onkeydown={(e) => capture(e, (v) => (hub = v))} />
  </div>
  <button onclick={saveShortcuts} disabled={!prefs}>Save shortcuts</button>
  {#if prefs?.shortcut_error}<p class="error">{prefs.shortcut_error}</p>{/if}
</div>

{#if error}<p class="error">{error}</p>{/if}
{#if message}<p class="note">{message}</p>{/if}

<style>
  h2 { font-size: 15px; margin: 22px 0 8px; }
  .box { padding: 12px 14px; border: 1px solid #222a3a; border-radius: 10px; background: #141925; }
  .check { display: flex; gap: 8px; align-items: center; }
  .muted { opacity: 0.65; font-size: 13px; margin: 6px 0 10px; }
  .grid { display: grid; grid-template-columns: 220px 220px; gap: 8px 12px; align-items: center; margin-bottom: 10px; }
  .grid input {
    padding: 6px 10px; border: 1px solid #2c3548; border-radius: 8px; background: #1b2230;
    font-family: ui-monospace, monospace; cursor: pointer;
  }
  .grid input:focus { border-color: #7cc4ff; outline: none; }
  .error { color: #ffb4b4; }
  .note { color: #9fe6b8; }
</style>
