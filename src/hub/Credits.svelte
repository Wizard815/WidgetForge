<script>
  // Credits: data services, art, and every third-party package the app ships.
  // The package list and license texts come from tools/gen_credits.py.
  import { onMount } from "svelte";
  import { api } from "../lib/api.js";

  // Open links in the browser, not inside the app window.
  function link(e) {
    const a = e.target.closest("a[href]");
    if (!a) return;
    e.preventDefault();
    api.openLink(a.href);
  }

  // PLACEHOLDER wording throughout; the facts (names, licenses, links) are real.
  const SERVICES = [
    {
      name: "Open-Meteo",
      what: "Weather data and place search (Weather widget)",
      license: "Data: CC BY 4.0",
      url: "https://open-meteo.com/",
    },
    {
      name: "TheSportsDB",
      what: "Team results and fixtures (Team Scores widget)",
      license: "See TheSportsDB terms",
      url: "https://www.thesportsdb.com/",
    },
    { name: "Frankfurter", what: "Currency rates (Converter widget)", license: "Free API", url: "https://frankfurter.dev/" },
    { name: "Radio Browser", what: "Radio station directory (Internet Radio widget)", license: "Free and open", url: "https://www.radio-browser.info/" },
    { name: "NASA APOD", what: "Astronomy Picture of the Day (Picture of the Day widget)", license: "Per-picture credit shown", url: "https://apod.nasa.gov/" },
    { name: "Wikipedia / Wikimedia Commons", what: "Featured picture (Picture of the Day widget)", license: "Per-picture license shown", url: "https://commons.wikimedia.org/" },
    { name: "CoinGecko", what: "Crypto prices (Prices widget). Powered by CoinGecko", license: "CoinGecko API terms", url: "https://www.coingecko.com/" },
    { name: "Finnhub", what: "Stock prices with your own key (Prices widget)", license: "Finnhub terms", url: "https://finnhub.io/" },
    { name: "Ship24", what: "Parcel tracking with your own key (Parcels widget)", license: "Ship24 terms", url: "https://www.ship24.com/" },
    { name: "Steam Web API", what: "Friends online with your own key (Steam Friends widget). Not affiliated with Valve", license: "Steam Web API terms", url: "https://steamcommunity.com/dev" },
    { name: "ipify and icanhazip", what: "Your public IP address (IP & VPN widget)", license: "Their terms", url: "https://www.ipify.org/" },
  ];

  let packages = $state([]);
  let missing = $state([]);
  let query = $state("");
  let notices = $state("");
  let showNotices = $state(false);
  let error = $state("");

  let shown = $derived(
    packages.filter((p) => !query || `${p.name} ${p.license}`.toLowerCase().includes(query.toLowerCase())),
  );

  onMount(async () => {
    try {
      const r = await fetch("credits/third-party.json");
      const j = await r.json();
      packages = j.packages;
      missing = j.missing_license_files ?? [];
    } catch (e) {
      error = "The credits list is missing. Run tools/gen_credits.py before building.";
    }
  });

  async function openNotices() {
    showNotices = !showNotices;
    if (showNotices && !notices) notices = await (await fetch("credits/THIRD-PARTY-NOTICES.txt")).text();
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div onclick={link}>
<header>
  <h1>Credits</h1>
  <p>WidgetForge is built on the work of others. Thank you.</p>
</header>

<h2>Data services</h2>
<table>
  <thead><tr><th>Service</th><th>Used for</th><th>License</th></tr></thead>
  <tbody>
    {#each SERVICES as s}
      <tr><td><a href={s.url} target="_blank" rel="noreferrer">{s.name}</a></td><td>{s.what}</td><td>{s.license}</td></tr>
    {/each}
  </tbody>
</table>
<p class="muted">Feeds, calendars and services you add yourself belong to their owners.</p>

<h2>Art</h2>
<p class="muted">The app icon is a temporary stand-in. Widget icons: to be credited once their source is confirmed.</p>

<h2>Third-party software ({packages.length})</h2>
{#if error}<p class="error">{error}</p>{/if}
<div class="tools">
  <input placeholder="Search packages or licenses" bind:value={query} />
  <button onclick={openNotices}>{showNotices ? "Hide" : "Show"} full license texts</button>
</div>
{#if showNotices}
  <pre class="notices">{notices || "Loading…"}</pre>
{:else}
  <table>
    <thead><tr><th>Package</th><th>Version</th><th>License</th></tr></thead>
    <tbody>
      {#each shown as p (p.kind + p.name + p.version)}
        <tr>
          <td><a href={p.url} target="_blank" rel="noreferrer">{p.name}</a></td>
          <td>{p.version}</td>
          <td>{p.license}</td>
        </tr>
      {/each}
    </tbody>
  </table>
{/if}
{#if missing.length}
  <p class="muted">No license file inside: {missing.join(", ")}. Their license is listed above.</p>
{/if}
</div>

<style>
  header h1 { margin: 0 0 4px; font-size: 22px; }
  header p { margin: 0 0 18px; opacity: 0.65; }
  h2 { font-size: 15px; margin: 22px 0 8px; }
  table { border-collapse: collapse; width: 100%; font-size: 13px; }
  th, td { text-align: left; padding: 5px 10px; border-bottom: 1px solid #222a3a; }
  th { opacity: 0.6; font-weight: 600; }
  a { color: #7cc4ff; text-decoration: none; }
  a:hover { text-decoration: underline; }
  .muted { opacity: 0.6; font-size: 13px; }
  .error { color: #ffb4b4; }
  .tools { display: flex; gap: 8px; margin-bottom: 10px; }
  .tools input {
    flex: 1; padding: 6px 10px; border: 1px solid #2c3548; border-radius: 8px; background: #1b2230;
  }
  .notices {
    max-height: 60vh; overflow: auto; padding: 12px; border: 1px solid #222a3a; border-radius: 8px;
    background: #0b0e15; font-size: 11px; white-space: pre-wrap; user-select: text;
  }
</style>
