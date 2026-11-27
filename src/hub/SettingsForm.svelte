<script>
  // A form built from a manifest's `settings` list. Saves each change at once.
  import { effectiveSettings } from "../lib/bindings.js";

  let { manifest, instance, onchange, onsecret } = $props();
  let values = $derived(effectiveSettings(manifest, instance));
  const folderHint = navigator.userAgent.includes("Windows") ? "e.g. C:\\Users\\you\\Pictures" : "e.g. ~/Pictures";
  // Time zones for the suggestions list (Intl knows them all).
  const zones = typeof Intl.supportedValuesOf === "function" ? Intl.supportedValuesOf("timeZone") : [];

  function set(key, value) {
    onchange({ [key]: value });
  }
</script>

<datalist id="wf-timezones">{#each zones as z}<option value={z}></option>{/each}</datalist>

{#if (manifest.settings ?? []).length === 0}
  <p class="muted">This widget has no settings.</p>
{:else}
  <div class="form">
    {#each manifest.settings as s (s.key)}
      <label class="field" class:inline={s.type === "bool"}>
        {#if s.type === "bool"}
          <input type="checkbox" checked={!!values[s.key]} onchange={(e) => set(s.key, e.currentTarget.checked)} />
          <span>{s.label ?? s.key}</span>
        {:else}
          <span>{s.label ?? s.key}</span>
          {#if s.type === "choice"}
            <select value={values[s.key]} onchange={(e) => set(s.key, e.currentTarget.value)}>
              {#each s.options ?? [] as opt}<option value={opt}>{opt}</option>{/each}
            </select>
          {:else if s.type === "number"}
            <input
              type="number"
              value={values[s.key]}
              min={s.min}
              max={s.max}
              step={s.step ?? "any"}
              onchange={(e) => set(s.key, Number(e.currentTarget.value))}
            />
          {:else if s.type === "timezone"}
            <input
              type="text"
              list="wf-timezones"
              placeholder="e.g. America/New_York — empty for this computer"
              value={values[s.key] ?? ""}
              onchange={(e) => set(s.key, e.currentTarget.value.trim())}
            />
          {:else if s.type === "datetime" || s.type === "date"}
            <input
              type={s.type === "date" ? "date" : "datetime-local"}
              value={values[s.key] ?? ""}
              onchange={(e) => set(s.key, e.currentTarget.value)}
            />
          {:else if s.type === "folder"}
            <input
              type="text"
              placeholder={folderHint}
              value={values[s.key] ?? ""}
              onchange={(e) => set(s.key, e.currentTarget.value.trim())}
            />
          {:else if s.type === "lines"}
            <!-- A list, one entry per line. -->
            <textarea
              rows="5"
              spellcheck="false"
              placeholder={s.placeholder ?? ""}
              value={values[s.key] ?? ""}
              onchange={(e) => set(s.key, e.currentTarget.value)}
            ></textarea>
          {:else if s.type === "file"}
            <input
              type="text"
              placeholder={s.placeholder ?? "Full path to the file"}
              value={values[s.key] ?? ""}
              onchange={(e) => set(s.key, e.currentTarget.value.trim())}
            />
          {:else if s.type === "command"}
            <!-- Runs on this computer as you. Only what is typed here is ever run. -->
            <input
              class="mono"
              type="text"
              spellcheck="false"
              placeholder={s.placeholder ?? "e.g. uptime"}
              value={values[s.key] ?? ""}
              onchange={(e) => set(s.key, e.currentTarget.value)}
            />
            {#if !s.quiet}<small class="warn">This runs on your computer as you. Only use commands you trust.</small>{/if}
          {:else if s.type === "secret"}
            <!-- Saved by the core outside the settings; the widget's own code never sees it. -->
            <input
              type="password"
              autocomplete="off"
              placeholder={values[s.key] ? "Saved — type to replace" : "Not set"}
              onchange={(e) => {
                onsecret(s.key, e.currentTarget.value);
                e.currentTarget.value = "";
              }}
            />
            {#if values[s.key]}
              <button class="link" type="button" onclick={() => onsecret(s.key, "")}>Remove saved key</button>
            {/if}
          {:else}
            <input type="text" value={values[s.key] ?? ""} onchange={(e) => set(s.key, e.currentTarget.value)} />
          {/if}
        {/if}
      </label>
    {/each}
  </div>
{/if}

<style>
  .form { display: grid; grid-template-columns: repeat(auto-fill, minmax(220px, 1fr)); gap: 10px 16px; }
  .field { display: flex; flex-direction: column; gap: 4px; font-size: 13px; }
  .field.inline { flex-direction: row; align-items: center; gap: 8px; }
  .muted { opacity: 0.6; margin: 0; }
  .mono { font-family: ui-monospace, monospace; }
  .warn { color: #ffd166; font-size: 11px; }
  .field:has(textarea) { grid-column: 1 / -1; }
  textarea {
    font: 12px ui-monospace, monospace; color: inherit; resize: vertical; padding: 6px 8px; border-radius: 6px;
    border: 1px solid rgba(255, 255, 255, 0.16); background: rgba(0, 0, 0, 0.25);
  }
  .link { align-self: flex-start; border: 0; background: none; color: #7cc4ff; padding: 0; cursor: pointer; font-size: 12px; }
</style>
