<script>
  // Hub → Builder: make a widget without writing YAML. Pick blocks, connect
  // them to data, see it live, save it as an add-on.
  // PLACEHOLDER wording throughout.
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { api } from "../lib/api.js";
  import { bind } from "../lib/bindings.js";
  import { parseFeed, parseIcs } from "../lib/parsers.js";
  import {
    BLOCK_TYPES, COMMON_FIELDS, COLOR_NAMES, SOURCE_KINDS, SYSTEM_CATEGORIES, FILTER_CHOICES,
    newDraft, newSource, buildManifest, draftFromManifest, problems, getAt, insertBlock, removeBlock,
    moveBlock, updateBlock, outline, valuePaths, rowPaths, binding, slug,
  } from "../lib/builder.js";
  import Preview from "./Preview.svelte";

  let { widgets = [], onsaved } = $props();

  const STORE = "widgetforge.builder.draft";
  let draft = $state(newDraft());
  let editingId = $state(null); // the add-on being edited, if any
  let selected = $state([0]);
  let tab = $state("block");
  let samples = $state({});
  let sampleErrors = $state({});
  let tick = $state(0);
  let message = $state("");
  let confirmReplace = $state(false);
  let theme = $state({});
  let picker = $state(null); // { field, rows } while the value list is open
  let pickFilter = $state("");
  let pickSearch = $state("");

  let manifest = $derived(buildManifest(draft));
  let issues = $derived(problems(draft));
  let tree = $derived(outline(draft.blocks));
  let block = $derived(selected ? getAt(draft.blocks, selected) : null);
  let type = $derived(block ? BLOCK_TYPES[block.type] : null);
  let ctx = $derived({ ...samples, settings: {} });
  let allPaths = $derived(valuePaths(samples));
  let addons = $derived(widgets.filter((w) => w.source !== "built-in"));
  let builtins = $derived(widgets.filter((w) => w.source === "built-in" && !w.manifest.ui?.stack && !w.manifest.ui?.entry));
  const GROUPS = ["Show", "Do", "Layout"];

  // ---------- keep the draft between visits ----------
  onMount(() => {
    try {
      const saved = JSON.parse(localStorage.getItem(STORE) ?? "null");
      if (saved?.draft) {
        draft = saved.draft;
        editingId = saved.editingId ?? null;
      }
    } catch {}
    api.getTheme().then((t) => (theme = t ?? {})).catch(() => {});
    const off = listen("theme-changed", (e) => (theme = e.payload ?? {}));
    refreshSamples(true);
    const t = setInterval(() => refreshSamples(false), 2000);
    return () => {
      clearInterval(t);
      off.then((f) => f());
    };
  });
  $effect(() => {
    localStorage.setItem(STORE, JSON.stringify({ draft, editingId }));
  });

  // ---------- sample data for the preview ----------
  let systemCache = null;
  async function refreshSamples(all) {
    const next = { ...samples };
    const errs = { ...sampleErrors };
    let sys = null;
    for (const s of draft.sources) {
      try {
        if (s.kind === "clock") next[s.name] = { now: Date.now(), real: Date.now(), timezone: Intl.DateTimeFormat().resolvedOptions().timeZone };
        else if (s.kind === "system") {
          sys = sys ?? (await api.builderSampleSystem());
          systemCache = sys;
          const out = { uptime: sys.uptime };
          for (const c of s.categories ?? []) out[c] = sys[c];
          next[s.name] = out;
        } else if (s.kind === "net" && all) {
          if (!/^https?:\/\//.test(s.url ?? "")) continue;
          const started = performance.now();
          const r = await api.builderFetch(s.url, { Accept: "application/json" });
          const out = { ok: r.ok, status: r.status, ms: Math.round(performance.now() - started), json: r.json, text: r.text };
          if (r.ok && s.format === "rss") Object.assign(out, parseFeed(r.text ?? ""));
          if (r.ok && s.format === "ics") Object.assign(out, parseIcs(r.text ?? "", { days: 60 }));
          next[s.name] = out;
          errs[s.name] = r.ok ? "" : `HTTP ${r.status}${s.use_key ? " (the sample is fetched without your key)" : ""}`;
        }
        if (s.kind !== "net" && s.kind !== "command") errs[s.name] = "";
      } catch (e) {
        errs[s.name] = String(e);
      }
    }
    for (const k of Object.keys(next)) if (!draft.sources.some((s) => s.name === k)) delete next[k];
    samples = next;
    sampleErrors = errs;
    tick++;
  }
  async function runCommandSample(s) {
    try {
      const r = await api.builderRun(s.command ?? "");
      samples = { ...samples, [s.name]: r };
      sampleErrors = { ...sampleErrors, [s.name]: r.ok ? "" : (r.stderr || `exit code ${r.code}`) };
    } catch (e) {
      sampleErrors = { ...sampleErrors, [s.name]: String(e) };
    }
  }

  // ---------- editing ----------
  function addBlock(kind) {
    const [blocks, path] = insertBlock(draft.blocks, selected, BLOCK_TYPES[kind].make());
    draft.blocks = blocks;
    selected = path;
    tab = "block";
  }
  function setField(key, value) {
    draft.blocks = updateBlock(draft.blocks, selected, { [key]: value });
  }
  function move(dir) {
    const [blocks, path] = moveBlock(draft.blocks, selected, dir);
    draft.blocks = blocks;
    selected = path;
  }
  function remove() {
    draft.blocks = removeBlock(draft.blocks, selected);
    selected = draft.blocks.length ? [Math.max(0, selected[0] - (selected.length === 1 ? 1 : 0))] : null;
  }
  function setColumns(cols) {
    setField("columns", cols);
  }
  function addSource(kind) {
    draft.sources = [...draft.sources, newSource(kind, draft.sources.map((s) => s.name))];
    refreshSamples(true);
  }
  function setSource(i, patch, refetch = false) {
    const old = draft.sources[i].name;
    draft.sources[i] = { ...draft.sources[i], ...patch };
    if (patch.name && patch.name !== old) {
      const { [old]: v, ...rest } = samples;
      samples = { ...rest, [patch.name]: v };
    }
    if (refetch) refreshSamples(true);
  }
  function removeSource(i) {
    draft.sources = draft.sources.filter((_, j) => j !== i);
  }
  function toggleCategory(i, cat, on) {
    const s = draft.sources[i];
    const cats = new Set(s.categories ?? []);
    on ? cats.add(cat) : cats.delete(cat);
    setSource(i, { categories: SYSTEM_CATEGORIES.map(([c]) => c).filter((c) => cats.has(c)) });
    refreshSamples(false);
  }

  // ---------- value picker ----------
  function openPicker(field, rows = null) {
    picker = picker?.field === field ? null : { field, rows };
    pickSearch = "";
    pickFilter = "";
  }
  function insertValue(path) {
    const f = picker.field;
    const text = binding(path, pickFilter);
    if (f.startsWith("col:")) {
      const [, i, key] = f.split(":");
      const cols = JSON.parse(JSON.stringify(block.columns ?? []));
      cols[+i][key] = (cols[+i][key] ?? "") + text;
      setColumns(cols);
    } else if (f === "items") setField("items", `{{ ${path} }}`);
    else setField(f, (block[f] ? String(block[f]) : "") + text);
    picker = null;
  }
  let pickList = $derived.by(() => {
    if (!picker) return [];
    let list = picker.rows ?? allPaths;
    if (picker.field === "items") list = list.filter((p) => p.kind === "list");
    const q = pickSearch.trim().toLowerCase();
    return q ? list.filter((p) => p.path.toLowerCase().includes(q)) : list;
  });
  let tableRows = $derived(block?.type === "table" ? rowPaths(bind(block.items ?? "", ctx)) : []);

  // ---------- files ----------
  function startNew() {
    draft = newDraft();
    editingId = null;
    selected = [0];
    message = "";
    refreshSamples(true);
  }
  function open(value) {
    if (!value) return;
    const [how, id] = value.split(":");
    const w = widgets.find((x) => x.id === id);
    if (!w) return;
    draft = draftFromManifest(w.manifest, how === "copy");
    editingId = how === "edit" ? id : null;
    selected = draft.blocks.length ? [0] : null;
    message = how === "copy" ? `A copy of ${w.manifest.name}. Give it your own id before saving.` : "";
    refreshSamples(true);
  }
  async function save(addToDesktop) {
    message = "";
    if (issues.length) {
      message = issues[0];
      return;
    }
    try {
      await api.builderSave(manifest, editingId === draft.id || confirmReplace);
      confirmReplace = false;
      editingId = draft.id;
      let text = `Saved as an add-on (${draft.id}).`;
      if (addToDesktop) {
        const inst = await api.addInstance(draft.id);
        // Commands go into the user's own settings, never into the widget file.
        const commands = {};
        for (const s of draft.sources.filter((x) => x.kind === "command" && x.command)) {
          commands[s.name === "cmd" ? "command" : `${s.name}_command`] = s.command;
        }
        if (Object.keys(commands).length) await api.updateInstance(inst.id, { settings: commands });
        text += " It is on your desktop.";
        if (manifest.settings?.some((s) => s.type === "secret")) text += " Enter its key in Desktop → Settings.";
      }
      message = text;
      onsaved?.();
    } catch (e) {
      const err = String(e);
      if (err.includes("already exists")) {
        confirmReplace = true;
        message = `${err}. Press Save again to replace it.`;
      } else message = err;
    }
  }
  function nameChanged(v) {
    const autoId = !editingId && (draft.id === "my.new-widget" || draft.id === `my.${slug(draft.name)}`);
    draft.name = v;
    if (autoId) draft.id = `my.${slug(v)}`;
  }
  const isBind = (f) => f.kind === "bind";
</script>

<header class="top">
  <div>
    <h1>Builder</h1>
    <p>Make your own widget: add blocks, connect them to data, and watch it change as you go.</p>
  </div>
  <div class="actions">
    <button onclick={startNew}>New</button>
    <select value="" onchange={(e) => { open(e.currentTarget.value); e.currentTarget.value = ""; }}>
      <option value="">Open…</option>
      {#if addons.length}
        <optgroup label="Your add-ons">
          {#each addons as w}<option value="edit:{w.id}">{w.manifest.name}</option>{/each}
        </optgroup>
      {/if}
      <optgroup label="Copy a built-in widget">
        {#each builtins as w}<option value="copy:{w.id}">{w.manifest.name}</option>{/each}
      </optgroup>
    </select>
    <button onclick={() => save(false)}>{confirmReplace ? "Replace" : "Save"}</button>
    <button class="primary" onclick={() => save(true)}>Save and add to desktop</button>
  </div>
</header>
{#if message}<div class="message">{message}</div>{/if}

<div class="builder">
  <!-- Outline and palette -->
  <aside class="left">
    <h3>Blocks</h3>
    <div class="outline">
      {#each tree as row (row.path.join("."))}
        <button
          class="node"
          class:sel={selected && selected.join(".") === row.path.join(".")}
          style="padding-left: {8 + row.depth * 14}px"
          onclick={() => { selected = row.path; tab = "block"; }}
        >
          <span class="kind">{BLOCK_TYPES[row.block.type]?.name ?? row.block.type}</span>
          <span class="peek">{row.block.text ?? row.block.label ?? row.block.value ?? ""}</span>
        </button>
      {:else}
        <div class="muted">No blocks yet. Add one below.</div>
      {/each}
    </div>
    {#if block}
      <div class="tools">
        <button title="Move up" onclick={() => move(-1)}>↑</button>
        <button title="Move down" onclick={() => move(1)}>↓</button>
        <button class="danger" title="Remove" onclick={remove}>Remove</button>
      </div>
    {/if}
    <h3>Add</h3>
    <p class="muted small">New blocks go after the selected one, or inside it if it is a layout block.</p>
    {#each GROUPS as g}
      <div class="group">{g}</div>
      <div class="palette">
        {#each Object.entries(BLOCK_TYPES).filter(([, t]) => t.group === g) as [k, t]}
          <button title={t.hint} onclick={() => addBlock(k)}>{t.name}</button>
        {/each}
      </div>
    {/each}
  </aside>

  <!-- Live preview -->
  <section class="middle">
    <div class="stage">
      <Preview
        blocks={draft.blocks}
        data={samples}
        {manifest}
        {theme}
        width={draft.width}
        height={draft.height}
        {tick}
        {selected}
        onpick={(p) => { selected = p; tab = "block"; }}
      />
    </div>
    <div class="size">
      <label>Width <input type="number" min="80" max="1600" value={draft.width} onchange={(e) => (draft.width = +e.currentTarget.value)} /></label>
      <label>Height <input type="number" min="40" max="1600" value={draft.height} onchange={(e) => (draft.height = +e.currentTarget.value)} /></label>
      <span class="muted small">Live, with real data from this computer. Buttons do nothing here.</span>
    </div>
    {#if issues.length}
      <ul class="issues">{#each issues as p}<li>{p}</li>{/each}</ul>
    {/if}
  </section>

  <!-- Inspector -->
  <aside class="right">
    <div class="tabs">
      <button class:on={tab === "block"} onclick={() => (tab = "block")}>Block</button>
      <button class:on={tab === "data"} onclick={() => (tab = "data")}>Data</button>
      <button class:on={tab === "widget"} onclick={() => (tab = "widget")}>Widget</button>
    </div>

    {#if tab === "block"}
      {#if !block}
        <p class="muted">Select a block in the list or the preview.</p>
      {:else if !type}
        <p class="muted">This block ({block.type}) can only be changed in the widget file. It is kept as it is.</p>
      {:else}
        <h3>{type.name}</h3>
        <p class="muted small">{type.hint}</p>
        {#each [...type.fields, ...COMMON_FIELDS] as f}
          <div class="field">
            <span class="label">{f.label}</span>
            {#if f.kind === "bind"}
              <div class="bindrow">
                <input value={block[f.key] ?? ""} placeholder={f.default ?? ""} oninput={(e) => setField(f.key, e.currentTarget.value)} />
                <button class="value" title="Insert a value from your data" onclick={() => openPicker(f.key)}>＋ value</button>
              </div>
            {:else if f.kind === "text"}
              <input value={block[f.key] ?? ""} oninput={(e) => setField(f.key, e.currentTarget.value)} />
            {:else if f.kind === "number"}
              <input type="number" value={block[f.key] ?? f.default ?? ""} oninput={(e) => setField(f.key, e.currentTarget.value === "" ? undefined : +e.currentTarget.value)} />
            {:else if f.kind === "bool"}
              <input type="checkbox" checked={block[f.key] ?? f.default ?? false} onchange={(e) => setField(f.key, e.currentTarget.checked)} />
            {:else if f.kind === "choice"}
              <select value={block[f.key] ?? f.default} onchange={(e) => setField(f.key, e.currentTarget.value === f.default ? undefined : e.currentTarget.value)}>
                {#each f.options as o}<option value={o}>{o || "normal"}</option>{/each}
              </select>
            {:else if f.kind === "color"}
              <div class="bindrow">
                <select value={COLOR_NAMES.includes(block[f.key] ?? f.default) ? (block[f.key] ?? f.default) : "custom"} onchange={(e) => setField(f.key, e.currentTarget.value === "custom" ? "#ffffff" : e.currentTarget.value === f.default ? undefined : e.currentTarget.value)}>
                  {#each COLOR_NAMES as c}<option value={c}>{c === "inherit" ? "normal" : c}</option>{/each}
                  <option value="custom">custom…</option>
                </select>
                {#if block[f.key] && !COLOR_NAMES.includes(block[f.key])}
                  <input type="color" value={block[f.key]} onchange={(e) => setField(f.key, e.currentTarget.value)} />
                {/if}
              </div>
            {/if}
            {#if picker?.field === f.key}
              {@render pickerPanel()}
            {/if}
          </div>
        {/each}

        {#if type.columns}
          <h3>Columns</h3>
          <p class="muted small">Each column shows something from one item of the list: {"{{ row.… }}"}.</p>
          {#each block.columns ?? [] as col, i}
            <div class="col">
              <div class="bindrow">
                <input placeholder="Name" value={col.label ?? ""} oninput={(e) => { const c = JSON.parse(JSON.stringify(block.columns)); c[i].label = e.currentTarget.value; setColumns(c); }} />
                <select value={col.align ?? "left"} onchange={(e) => { const c = JSON.parse(JSON.stringify(block.columns)); c[i].align = e.currentTarget.value; setColumns(c); }}>
                  <option>left</option><option>center</option><option>right</option>
                </select>
                <button class="danger" title="Remove column" onclick={() => setColumns(block.columns.filter((_, j) => j !== i))}>✕</button>
              </div>
              <div class="bindrow">
                <input placeholder="{'{{ row.name }}'}" value={col.value ?? ""} oninput={(e) => { const c = JSON.parse(JSON.stringify(block.columns)); c[i].value = e.currentTarget.value; setColumns(c); }} />
                <button class="value" onclick={() => openPicker(`col:${i}:value`, tableRows)}>＋ value</button>
              </div>
              {#if picker?.field === `col:${i}:value`}{@render pickerPanel()}{/if}
            </div>
          {/each}
          <button onclick={() => setColumns([...(block.columns ?? []), { label: "", value: "" }])}>Add column</button>
        {/if}
      {/if}

    {:else if tab === "data"}
      <p class="muted small">Where the widget's values come from. Name each one; use the name in blocks, e.g. {"{{ pc.cpu.usage }}"}.</p>
      {#each draft.sources as s, i}
        <div class="source">
          <div class="bindrow">
            <input class="name" value={s.name} onchange={(e) => setSource(i, { name: e.currentTarget.value.trim() })} title="Name used in blocks" />
            <span class="kindname">{SOURCE_KINDS[s.kind]?.name ?? "Kept from the widget file"}</span>
            <button class="danger" title="Remove" onclick={() => removeSource(i)}>✕</button>
          </div>
          {#if s.kind === "system"}
            <div class="cats">
              {#each SYSTEM_CATEGORIES as [c, label]}
                <label><input type="checkbox" checked={s.categories?.includes(c)} onchange={(e) => toggleCategory(i, c, e.currentTarget.checked)} /> {label}</label>
              {/each}
            </div>
          {:else if s.kind === "net"}
            <input placeholder="https://… or http://192.168.1.10:8123/api/…" value={s.url ?? ""} onchange={(e) => setSource(i, { url: e.currentTarget.value.trim() }, true)} />
            <div class="bindrow">
              <select value={s.format ?? "json"} onchange={(e) => setSource(i, { format: e.currentTarget.value }, true)}>
                <option value="json">JSON</option><option value="rss">RSS / Atom feed</option><option value="ics">Calendar (ICS)</option>
              </select>
              <label class="small"><input type="checkbox" checked={s.use_key} onchange={(e) => setSource(i, { use_key: e.currentTarget.checked })} /> Needs a key</label>
            </div>
            {#if s.use_key}
              <div class="bindrow">
                <input value={s.key_header} title="Header name" onchange={(e) => setSource(i, { key_header: e.currentTarget.value })} />
                <input value={s.key_prefix} title="Text before the key" onchange={(e) => setSource(i, { key_prefix: e.currentTarget.value })} />
              </div>
              <p class="muted small">The key is typed in the widget's settings after you add it, and kept in the system keychain.</p>
            {/if}
            <button onclick={() => refreshSamples(true)}>Fetch a sample</button>
          {:else if s.kind === "command"}
            <input class="mono" placeholder="e.g. uptime" value={s.command ?? ""} onchange={(e) => setSource(i, { command: e.currentTarget.value })} />
            <p class="warn small">Runs on your computer as you. It is saved in this widget's settings on your desktop, never in the widget file.</p>
            <button onclick={() => runCommandSample(s)}>Run once for a sample</button>
          {:else if s.kind === "raw"}
            <p class="muted small">Set up in the widget file; the builder keeps it as it is.</p>
          {/if}
          {#if s.kind !== "raw" && s.kind !== "clock"}
            <label class="small">Every <input type="number" class="tiny" min="1" value={s.every} onchange={(e) => setSource(i, { every: +e.currentTarget.value })} /> seconds</label>
          {/if}
          {#if sampleErrors[s.name]}<p class="err small">{sampleErrors[s.name]}</p>{/if}
        </div>
      {/each}
      <h3>Add data</h3>
      {#each Object.entries(SOURCE_KINDS) as [k, info]}
        <button class="addsrc" title={info.hint} onclick={() => addSource(k)}><strong>{info.name}</strong><span>{info.hint}</span></button>
      {/each}
      <h3>Values you can use</h3>
      <div class="paths">
        {#each allPaths.slice(0, 200) as p}
          <div class="path"><code>{p.path}</code><span>{p.sample}</span></div>
        {:else}
          <p class="muted small">No sample yet.</p>
        {/each}
      </div>

    {:else}
      <div class="field"><span class="label">Name</span><input value={draft.name} oninput={(e) => nameChanged(e.currentTarget.value)} /></div>
      <div class="field">
        <span class="label">Id (unique, like my.cpu-card)</span>
        <input class="mono" value={draft.id} oninput={(e) => (draft.id = e.currentTarget.value.trim())} />
      </div>
      <div class="field"><span class="label">Description</span><textarea rows="3" value={draft.description} oninput={(e) => (draft.description = e.currentTarget.value)}></textarea></div>
      <div class="field"><span class="label">Category</span><input value={draft.category} oninput={(e) => (draft.category = e.currentTarget.value)} /></div>
      <div class="field"><span class="label">Accent colour</span><input type="color" value={draft.accent} onchange={(e) => (draft.accent = e.currentTarget.value)} /></div>
      <label class="small"><input type="checkbox" checked={draft.multi} onchange={(e) => (draft.multi = e.currentTarget.checked)} /> Can be added more than once</label>
      <h3>Permissions it will ask for</h3>
      <ul class="small">
        {#each manifest.permissions?.system ?? [] as c}<li>Read this computer: {c}</li>{/each}
        {#each manifest.permissions?.network ?? [] as h}<li>Reach {h}</li>{/each}
        {#if manifest.permissions?.run}<li>Run the command you type</li>{/if}
        {#if manifest.permissions?.storage}<li>Save its own notes or lists</li>{/if}
        {#if !manifest.permissions}<li>None</li>{/if}
      </ul>
      <details>
        <summary>The widget file it makes</summary>
        <pre>{JSON.stringify(manifest, null, 2)}</pre>
      </details>
    {/if}
  </aside>
</div>

{#snippet pickerPanel()}
  <div class="picker">
    <div class="bindrow">
      <input placeholder="Search values" bind:value={pickSearch} />
      {#if picker.field !== "items"}
        <select bind:value={pickFilter} title="Show it as">
          {#each FILTER_CHOICES as fc}<option value={fc.id}>{fc.name}</option>{/each}
        </select>
      {/if}
    </div>
    <div class="paths">
      {#each pickList.slice(0, 150) as p}
        <button class="path" onclick={() => insertValue(p.path)}><code>{p.path}</code><span>{p.sample}</span></button>
      {:else}
        <p class="muted small">{picker.rows ? "Pick the table's list first, then fetch a sample." : "No values yet: add data in the Data tab."}</p>
      {/each}
    </div>
  </div>
{/snippet}

<style>
  .top { display: flex; justify-content: space-between; align-items: flex-start; gap: 16px; flex-wrap: wrap; }
  .actions { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; }
  .builder { display: grid; grid-template-columns: 220px minmax(300px, 1fr) 320px; gap: 16px; align-items: start; }
  /* A narrow Hub window: the settings go under the preview. */
  @media (max-width: 1180px) {
    .builder { grid-template-columns: 200px minmax(0, 1fr); }
    aside.right { grid-column: 2; max-height: none; }
  }
  @media (max-width: 760px) {
    .builder { grid-template-columns: minmax(0, 1fr); }
    aside.right { grid-column: 1; }
  }
  aside { background: #141925; border: 1px solid #222a3a; border-radius: 10px; padding: 12px; }
  aside.right { max-height: calc(100vh - 170px); overflow: auto; }
  h3 { font-size: 13px; margin: 12px 0 6px; opacity: 0.85; }
  h3:first-child { margin-top: 0; }
  .muted { opacity: 0.6; }
  .small { font-size: 12px; }
  .warn { color: #ffd166; }
  .err { color: #ff9a9a; }
  .outline { display: flex; flex-direction: column; gap: 2px; max-height: 300px; overflow: auto; }
  .node { display: flex; gap: 6px; text-align: left; border: 0; background: none; padding: 4px 8px; border-radius: 6px; font-size: 12px; }
  .node.sel { background: #26304a; }
  .node .kind { font-weight: 600; white-space: nowrap; }
  .node .peek { opacity: 0.5; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .tools { display: flex; gap: 4px; margin-top: 6px; }
  .tools button { padding: 3px 9px; }
  .group { font-size: 11px; text-transform: uppercase; letter-spacing: 0.06em; opacity: 0.5; margin: 8px 0 4px; }
  .palette { display: flex; flex-wrap: wrap; gap: 4px; }
  .palette button { padding: 4px 8px; font-size: 12px; }
  .middle { display: flex; flex-direction: column; gap: 10px; min-width: 0; }
  .stage {
    display: grid; place-items: center; min-height: 320px; padding: 24px; border-radius: 12px; overflow: auto;
    background: linear-gradient(135deg, #2b4a6f, #6d4c7d 50%, #c07a4f);
  }
  .size { display: flex; gap: 12px; align-items: center; flex-wrap: wrap; font-size: 12px; }
  .size input { width: 70px; }
  .issues { color: #ffd166; font-size: 12px; margin: 0; padding-left: 18px; }
  .tabs { display: flex; gap: 4px; margin-bottom: 10px; }
  .tabs button { flex: 1; padding: 5px; }
  .tabs button.on { background: #26304a; border-color: #3a4870; }
  .field { display: flex; flex-direction: column; gap: 3px; margin-bottom: 8px; }
  .label { font-size: 12px; opacity: 0.75; }
  input:not([type="checkbox"]):not([type="color"]), textarea, select {
    font: inherit; font-size: 13px; padding: 5px 7px; border: 1px solid #2c3548; border-radius: 6px; background: #0f131c; min-width: 0;
  }
  textarea { resize: vertical; }
  .mono { font-family: ui-monospace, monospace; }
  .bindrow { display: flex; gap: 4px; align-items: center; }
  .bindrow input:not([type="checkbox"]) { flex: 1; }
  .value { white-space: nowrap; padding: 4px 7px; font-size: 12px; }
  .picker { border: 1px solid #3a4870; border-radius: 8px; padding: 6px; margin-top: 4px; background: #10141f; }
  .paths { display: flex; flex-direction: column; max-height: 220px; overflow: auto; margin-top: 4px; }
  .path { display: flex; justify-content: space-between; gap: 8px; font-size: 11px; padding: 3px 4px; border: 0; background: none; text-align: left; border-radius: 4px; }
  button.path:hover { background: #26304a; }
  .path code { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .path span { opacity: 0.55; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; max-width: 40%; }
  .col { border-top: 1px solid #222a3a; padding: 6px 0; display: flex; flex-direction: column; gap: 4px; }
  .source { border: 1px solid #222a3a; border-radius: 8px; padding: 8px; margin-bottom: 8px; display: flex; flex-direction: column; gap: 6px; }
  .source .name { max-width: 90px; font-family: ui-monospace, monospace; }
  .kindname { flex: 1; font-size: 12px; opacity: 0.7; }
  .cats { display: grid; grid-template-columns: 1fr 1fr; gap: 2px 8px; font-size: 12px; }
  .cats label { display: flex; gap: 4px; align-items: center; }
  .tiny { width: 60px; }
  .addsrc { display: flex; flex-direction: column; align-items: flex-start; width: 100%; text-align: left; margin-bottom: 4px; }
  .addsrc span { font-size: 11px; opacity: 0.6; }
  pre { font-size: 11px; max-height: 300px; overflow: auto; background: #0f131c; padding: 8px; border-radius: 6px; }
  .message { padding: 10px 12px; margin-bottom: 12px; border-radius: 8px; background: #1f3350; }
</style>
