<script>
  // A calculator with a keypad and typed input. Uses the safe parser in lib/calc.js.
  import { calculate } from "../../lib/calc.js";

  let input = $state("");
  let result = $state("");
  let error = $state("");
  let history = $state([]);

  const KEYS = ["C", "(", ")", "÷", "7", "8", "9", "×", "4", "5", "6", "-", "1", "2", "3", "+", "0", ".", "⌫", "="];

  function preview() {
    try {
      result = input ? String(calculate(input)) : "";
      error = "";
    } catch {
      result = "";
    }
  }
  function equals() {
    try {
      const v = String(calculate(input));
      history = [`${input} = ${v}`, ...history].slice(0, 4);
      input = v;
      result = "";
      error = "";
    } catch (e) {
      error = e.message;
    }
  }
  function press(k) {
    if (k === "C") { input = ""; result = ""; error = ""; return; }
    if (k === "⌫") input = input.slice(0, -1);
    else if (k === "=") return equals();
    else input += k;
    preview();
  }
  function onkeydown(e) {
    if (e.key === "Enter") { e.preventDefault(); equals(); }
    if (e.key === "Escape") press("C");
  }
</script>

<div class="calc">
  {#each history as h}<div class="history">{h}</div>{/each}
  <input class="display" bind:value={input} oninput={preview} {onkeydown} placeholder="0" spellcheck="false" />
  <div class="result" class:error={!!error}>{error || (result ? `= ${result}` : " ")}</div>
  <div class="keys">
    {#each KEYS as k}
      <button class:op={"÷×-+=".includes(k)} class:eq={k === "="} onclick={() => press(k)}>{k}</button>
    {/each}
  </div>
</div>

<style>
  .calc { display: flex; flex-direction: column; gap: 6px; }
  .history { font-size: 11px; opacity: 0.45; text-align: right; font-variant-numeric: tabular-nums; }
  .display {
    width: 100%; box-sizing: border-box; padding: 6px 8px; text-align: right;
    font: 600 22px/1.2 ui-monospace, "Cascadia Mono", monospace; color: inherit;
    background: color-mix(in srgb, currentColor 6%, transparent); border: 1px solid color-mix(in srgb, currentColor 12%, transparent); border-radius: 8px;
    outline: none; user-select: text;
  }
  .display:focus { border-color: var(--accent); }
  .result { text-align: right; font-size: 13px; opacity: 0.7; min-height: 17px; font-variant-numeric: tabular-nums; }
  .result.error { color: #ff9a9a; opacity: 1; }
  .keys { display: grid; grid-template-columns: repeat(4, 1fr); gap: 5px; }
  button {
    height: 34px; border-radius: 8px; cursor: pointer; font: inherit; font-size: 15px; color: inherit;
    border: 1px solid color-mix(in srgb, currentColor 12%, transparent); background: color-mix(in srgb, currentColor 7%, transparent);
  }
  button:hover { background: color-mix(in srgb, currentColor 14%, transparent); }
  button.op { color: var(--accent); }
  button.eq { background: var(--accent); color: var(--wf-on-accent); border-color: transparent; }
</style>
