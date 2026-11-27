<script>
  // Photo frame: shows pictures one after another with a short fade.
  // `images`: [{ url, name }] — e.g. from a `folder` source.
  import { onMount } from "svelte";

  let { images = [], seconds = 10, fit = "cover", shuffle = false, captions = false, empty = "" } = $props();

  let index = $state(0);
  let order = $derived.by(() => {
    const list = Array.isArray(images) ? [...images] : [];
    if (shuffle) {
      for (let i = list.length - 1; i > 0; i--) {
        const j = Math.floor(Math.random() * (i + 1));
        [list[i], list[j]] = [list[j], list[i]];
      }
    }
    return list;
  });
  let current = $derived(order.length ? order[index % order.length] : null);

  onMount(() => {
    const t = setInterval(() => (index += 1), Math.max(2, seconds) * 1000);
    return () => clearInterval(t);
  });
</script>

<div class="frame">
  {#if current}
    {#key current.url}
      <img src={current.url} alt={current.name} style="object-fit: {fit}" />
    {/key}
    {#if captions}<div class="caption">{current.name}</div>{/if}
    <button class="nav prev" title="Previous" onclick={() => (index = (index - 1 + order.length) % order.length)}>‹</button>
    <button class="nav next" title="Next" onclick={() => (index += 1)}>›</button>
  {:else}
    <div class="empty">{empty}</div>
  {/if}
</div>

<style>
  /* Fill the whole widget, edge to edge. */
  .frame { position: absolute; inset: 0; overflow: hidden; border-radius: inherit; }
  img { width: 100%; height: 100%; display: block; animation: fade 0.8s ease; }
  @keyframes fade { from { opacity: 0; } to { opacity: 1; } }
  .caption {
    position: absolute; left: 0; right: 0; bottom: 0; padding: 18px 12px 8px; font-size: 12px;
    background: linear-gradient(transparent, rgba(0, 0, 0, 0.6)); white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
  }
  .nav {
    position: absolute; top: 50%; margin-top: -16px; width: 28px; height: 32px; border: 0; border-radius: 8px;
    background: rgba(0, 0, 0, 0.4); color: #fff; font-size: 18px; cursor: pointer; opacity: 0; transition: opacity 0.15s;
  }
  .frame:hover .nav { opacity: 1; }
  .prev { left: 6px; }
  .next { right: 6px; }
  .empty { padding: 16px; font-size: 13px; opacity: 0.7; }
</style>
