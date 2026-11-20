<script lang="ts">
  import type { Manifest } from "../../lib/types";
  import { iconUrl } from "../store.svelte";

  let { manifest, size = 40 }: { manifest: Manifest; size?: number } = $props();
  let failed = $state(false);
  const url = $derived(iconUrl(manifest));
  const letter = $derived(manifest.name.trim().charAt(0).toUpperCase());
</script>

<div class="wicon" style:width="{size}px" style:height="{size}px" style:--accent-c={manifest.style.accent ?? "var(--accent)"}>
  {#if url && !failed}
    <img src={url} alt="" onerror={() => (failed = true)} draggable="false" />
  {:else}
    <span style:font-size="{size * 0.45}px">{letter}</span>
  {/if}
</div>

<style>
  .wicon {
    flex: none;
    display: grid;
    place-items: center;
    border-radius: 28%;
    background: color-mix(in srgb, var(--accent-c) 14%, var(--panel-2));
    border: 1px solid var(--border);
    overflow: hidden;
  }
  img {
    width: 74%;
    height: 74%;
    object-fit: contain;
  }
  span {
    font-weight: 700;
    color: var(--accent-c);
  }
</style>
