<script lang="ts">
  let {
    checked,
    label,
    hint = "",
    onchange,
  }: { checked: boolean; label: string; hint?: string; onchange: (v: boolean) => void } = $props();
</script>

<label class="toggle">
  <span class="text">
    <span>{label}</span>
    {#if hint}<span class="hint">{hint}</span>{/if}
  </span>
  <input type="checkbox" {checked} onchange={(e) => onchange(e.currentTarget.checked)} />
  <span class="track"><span class="thumb"></span></span>
</label>

<style>
  .toggle {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 8px 0;
    cursor: pointer;
  }
  .text {
    display: flex;
    flex-direction: column;
  }
  .hint {
    font-size: 12px;
    color: var(--muted);
  }
  input {
    position: absolute;
    opacity: 0;
    pointer-events: none;
  }
  .track {
    flex: none;
    position: relative;
    width: 38px;
    height: 22px;
    border-radius: 999px;
    background: var(--border-strong);
    transition: background 0.15s;
  }
  .thumb {
    position: absolute;
    top: 3px;
    left: 3px;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: #fff;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.3);
    transition: transform 0.15s;
  }
  input:checked + .track {
    background: linear-gradient(135deg, var(--brand-1), var(--brand-2));
  }
  input:checked + .track .thumb {
    transform: translateX(16px);
  }
  input:focus-visible + .track {
    box-shadow: 0 0 0 3px var(--accent-bg);
  }
</style>
