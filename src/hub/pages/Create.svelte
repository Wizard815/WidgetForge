<script lang="ts">
  import { hub } from "../../lib/api";
  import Icon from "../components/Icon.svelte";
  import { act, toast } from "../store.svelte";

  const SAMPLE = `schema: 1
id: me.cpu-ring              # yourname.widget-name
name: CPU Ring
version: 1.0.0
author: Me
description: A single ring showing CPU usage.
category: system

window:
  size: [180, 200]
  layer: desktop             # top | normal | desktop

style:
  theme: glass
  accent: "#ff6b6b"

permissions:
  system: [cpu]

data:
  sys: { source: system, interval: 1000 }

ui:
  blocks:
    - type: gauge
      label: CPU
      value: "{{ sys.cpu.usage }}"
      format: "{{ sys.cpu.usage | fixed(0) }}%"
`;

  async function copy() {
    try {
      await navigator.clipboard.writeText(SAMPLE);
      toast("Copied");
    } catch {
      toast("Couldn't copy", "error");
    }
  }

  const BLOCKS = [
    ["text", "Any text, with live values"],
    ["gauge", "Ring gauge"],
    ["bar", "Progress bar"],
    ["graph", "Line chart with history"],
    ["list", "A list from data"],
    ["clock", "Analog clock face"],
    ["button", "Opens a link"],
    ["notes", "Editable text, saved"],
    ["checklist", "To-do list, saved"],
    ["image", "Picture from the widget folder"],
    ["row · column · grid", "Layout"],
  ];
</script>

<div class="page">
  <div class="page-head">
    <div>
      <h1>Create</h1>
      <p>Make your own widget. Simple ones are a single YAML file — no programming needed.</p>
    </div>
  </div>

  <div class="hero card">
    <div class="hero-icon"><Icon name="sparkle" size={28} /></div>
    <div>
      <h2>Forge Assistant <span class="chip accent">Coming soon</span></h2>
      <p class="muted">
        Describe the widget you want — “a red gauge for my CPU on the right edge” — and a small AI that runs on your own
        PC builds it for you, with a live preview. Works offline; also works with Ollama, LM Studio or a cloud key.
      </p>
    </div>
  </div>

  <div class="cols">
    <section class="card steps">
      <h2>Make one by hand</h2>
      <ol>
        <li>
          <button class="link" onclick={() => act(() => hub.openAddonsFolder())}>Open your add-ons folder</button> and make
          a new folder, e.g. <span class="mono">me.cpu-ring</span>.
        </li>
        <li>Save the example on the right inside it as <span class="mono">widget.yaml</span>.</li>
        <li>
          Go to <b>Add-ons</b> and press <b>Reload</b>. Your widget shows up in the Library.
        </li>
        <li>Edit, reload, repeat. Errors show on the Add-ons page with the exact line to fix.</li>
      </ol>

      <h3>Building blocks</h3>
      <ul class="blocks">
        {#each BLOCKS as [name, desc]}
          <li><span class="mono">{name}</span><span class="muted">{desc}</span></li>
        {/each}
      </ul>
      <p class="muted small">
        Values update live with <span class="mono">{"{{ sys.cpu.usage }}"}</span>, formatted with filters like
        <span class="mono">| fixed(0)</span>, <span class="mono">| bytes</span>, <span class="mono">| time('HH:mm')</span>. Need
        more? Point <span class="mono">ui.entry</span> at an HTML file and use JavaScript.
      </p>
    </section>

    <section class="card code">
      <div class="code-head">
        <span class="mono">widget.yaml</span>
        <button class="btn small" onclick={copy}><Icon name="copy" size={14} /> Copy</button>
      </div>
      <pre>{SAMPLE}</pre>
    </section>
  </div>
</div>

<style>
  .hero {
    display: flex;
    gap: 18px;
    align-items: center;
    padding: 22px;
    background:
      radial-gradient(120% 140% at 0% 0%, rgba(138, 61, 255, 0.22), transparent 55%),
      radial-gradient(120% 140% at 100% 100%, rgba(63, 77, 255, 0.18), transparent 55%),
      var(--panel);
  }
  .hero h2 {
    display: flex;
    gap: 10px;
    align-items: center;
    margin-bottom: 4px;
  }
  .hero-icon {
    flex: none;
    display: grid;
    place-items: center;
    width: 58px;
    height: 58px;
    border-radius: 18px;
    color: #fff;
    background: linear-gradient(135deg, var(--brand-1), var(--brand-2) 60%, var(--hot));
  }
  .cols {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 14px;
    margin-top: 14px;
  }
  .steps {
    padding: 18px 20px;
  }
  ol {
    padding-left: 20px;
    margin: 10px 0 18px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .link {
    border: 0;
    background: none;
    padding: 0;
    color: var(--accent);
    font-weight: 600;
    cursor: pointer;
  }
  .blocks {
    list-style: none;
    padding: 0;
    margin: 8px 0 12px;
    display: grid;
    grid-template-columns: 1fr;
    gap: 4px;
    font-size: 13px;
  }
  .blocks li {
    display: grid;
    grid-template-columns: 150px 1fr;
  }
  .code {
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .code-head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 10px 14px;
    border-bottom: 1px solid var(--border);
  }
  pre {
    margin: 0;
    padding: 14px 16px;
    overflow: auto;
    font: 12.5px/1.55 "Cascadia Mono", ui-monospace, Menlo, Consolas, monospace;
    background: var(--panel-2);
    flex: 1;
    user-select: text;
  }
</style>
