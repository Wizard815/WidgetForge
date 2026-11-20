<script lang="ts" generics="T extends string">
  let {
    value,
    options,
    onchange,
  }: { value: T; options: { value: T; label: string; title?: string }[]; onchange: (v: T) => void } = $props();
</script>

<div class="seg" role="radiogroup">
  {#each options as o (o.value)}
    <button
      type="button"
      role="radio"
      aria-checked={value === o.value}
      class:active={value === o.value}
      title={o.title ?? ""}
      onclick={() => value !== o.value && onchange(o.value)}>{o.label}</button>
  {/each}
</div>

<style>
  .seg {
    display: inline-flex;
    padding: 3px;
    gap: 2px;
    border-radius: 10px;
    background: var(--panel-2);
    border: 1px solid var(--border);
  }
  button {
    height: 28px;
    padding: 0 12px;
    border: 0;
    border-radius: 7px;
    background: transparent;
    color: var(--muted);
    font-size: 12.5px;
    font-weight: 600;
    cursor: pointer;
  }
  button:hover {
    color: var(--text);
  }
  button.active {
    background: var(--panel);
    color: var(--text);
    box-shadow:
      0 1px 2px rgba(0, 0, 0, 0.2),
      0 0 0 1px var(--border);
  }
</style>
