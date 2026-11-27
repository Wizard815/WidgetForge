<script>
  // Internet radio: search the free Radio Browser directory, keep favourites,
  // play a station. Streams play in this window; nothing is recorded.
  import { getContext, onMount } from "svelte";
  import { api } from "../../lib/api.js";
  let { server = "" } = $props();
  const forge = getContext("forge");

  // Radio Browser asks apps to spread load over its servers and fail over.
  const SERVERS = ["de1.api.radio-browser.info", "de2.api.radio-browser.info", "fi1.api.radio-browser.info"];
  let favourites = $state([]);
  let results = $state([]);
  let query = $state("");
  let searching = $state(false);
  let playing = $state(null);
  let status = $state("");
  let volume = $state(0.8);
  let audio;

  onMount(async () => {
    const saved = await api.storageGet(forge.instanceId, "radio").catch(() => null);
    if (saved?.favourites) favourites = saved.favourites;
    if (typeof saved?.volume === "number") volume = saved.volume;
  });
  const save = () => api.storageSet(forge.instanceId, "radio", { favourites, volume }).catch(() => {});

  async function call(path) {
    const list = server ? [server, ...SERVERS] : [...SERVERS].sort(() => Math.random() - 0.5);
    let last = "";
    for (const host of list) {
      try {
        const r = await api.netFetch(forge.instanceId, { url: `https://${host}${path}` });
        if (r.ok) return r.json;
        last = `HTTP ${r.status}`;
      } catch (e) {
        last = String(e);
      }
    }
    throw last || "no radio directory server answered";
  }

  async function search(e) {
    e?.preventDefault();
    const q = query.trim();
    if (!q) return (results = []);
    searching = true;
    try {
      const list = await call(`/json/stations/search?name=${encodeURIComponent(q)}&limit=25&hidebroken=true&order=votes&reverse=true`);
      results = (list ?? []).map(pick);
      if (!results.length) status = "No stations found.";
    } catch (err) {
      forge.report(String(err));
    } finally {
      searching = false;
    }
  }
  const pick = (s) => ({ id: s.stationuuid, name: s.name?.trim(), url: s.url_resolved || s.url, country: s.countrycode, codec: s.codec, bitrate: s.bitrate });

  function play(st) {
    if (playing?.id === st.id) return stop();
    playing = st;
    status = "Connecting…";
    audio.src = st.url;
    audio.volume = volume;
    audio.play().catch((e) => (status = `Cannot play this stream (${e.name}).`));
    // Counts a play for the directory's popularity list, as its rules ask.
    call(`/json/url/${encodeURIComponent(st.id)}`).catch(() => {});
  }
  function stop() {
    audio.pause();
    audio.removeAttribute("src");
    audio.load();
    playing = null;
    status = "";
  }
  const isFav = (st) => favourites.some((f) => f.id === st.id);
  function toggleFav(st) {
    favourites = isFav(st) ? favourites.filter((f) => f.id !== st.id) : [...favourites, st];
    save();
  }
  function setVolume(v) {
    volume = v;
    if (audio) audio.volume = v;
    save();
  }
  let showing = $derived(query.trim() && results.length ? results : favourites);
</script>

<audio
  bind:this={audio}
  onplaying={() => (status = "Playing")}
  onwaiting={() => (status = "Buffering…")}
  onerror={() => playing && (status = "This stream stopped or cannot be played.")}
></audio>

<div class="radio">
  <form onsubmit={search}>
    <input bind:value={query} placeholder="Find a station (name, e.g. jazz)" oninput={() => !query.trim() && (results = [])} />
  </form>
  {#if playing}
    <div class="now">
      <button class="stop" onclick={stop} title="Stop">■</button>
      <div class="info"><div class="title">{playing.name}</div><div class="sub">{status}</div></div>
      <input class="vol" type="range" min="0" max="1" step="0.01" value={volume} oninput={(e) => setVolume(+e.currentTarget.value)} title="Volume" />
    </div>
  {/if}
  <div class="list">
    {#if searching}<div class="empty">Searching…</div>{/if}
    {#each showing as st (st.id)}
      <div class="st" class:on={playing?.id === st.id}>
        <button class="play" onclick={() => play(st)}>{playing?.id === st.id ? "■" : "▶"}</button>
        <span class="name" title={st.name}>{st.name}</span>
        <span class="meta">{st.country ?? ""} {st.bitrate ? `${st.bitrate}k` : ""}</span>
        <button class="fav" class:yes={isFav(st)} onclick={() => toggleFav(st)} title={isFav(st) ? "Remove from favourites" : "Add to favourites"}>{isFav(st) ? "★" : "☆"}</button>
      </div>
    {:else}
      {#if !searching}<div class="empty">{query.trim() ? status : "Search for a station and star it to keep it here."}</div>{/if}
    {/each}
  </div>
  <div class="credit">Stations from radio-browser.info</div>
</div>

<style>
  .radio { display: flex; flex-direction: column; gap: 6px; min-height: 0; font-size: 12px; }
  input:not(.vol) { width: 100%; font: inherit; color: inherit; padding: 6px 8px; border-radius: 7px; border: 1px solid color-mix(in srgb, currentColor 16%, transparent); background: color-mix(in srgb, currentColor 7%, transparent); }
  .now { display: flex; align-items: center; gap: 8px; padding: 6px; border-radius: 8px; background: color-mix(in srgb, var(--accent) 18%, transparent); }
  .info { flex: 1; min-width: 0; }
  .title { font-weight: 700; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .sub { opacity: 0.7; font-size: 11px; }
  .vol { width: 70px; accent-color: var(--accent); }
  .list { display: flex; flex-direction: column; overflow-y: auto; min-height: 0; flex: 1; }
  .st { display: flex; align-items: center; gap: 6px; padding: 3px 0; border-top: 1px solid color-mix(in srgb, currentColor 6%, transparent); }
  .st.on .name { color: var(--accent); font-weight: 700; }
  .name { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .meta { opacity: 0.5; font-size: 10px; white-space: nowrap; }
  button { border: 0; background: none; color: inherit; cursor: pointer; padding: 0 3px; font-size: 12px; }
  .play, .stop { color: var(--accent); }
  .fav.yes { color: var(--wf-warn); }
  .empty { opacity: 0.55; padding: 6px 0; }
  .credit { font-size: 10px; opacity: 0.4; text-align: right; }
</style>
