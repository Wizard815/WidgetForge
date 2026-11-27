<script>
  // Unit and currency converter. Units are worked out here; currency uses the
  // `rates` given ({ base, date, rates: { USD: 1.05, ... } }).
  let { rates = null, base = "EUR" } = $props();

  // Each unit: [name, factor to the category's base unit]. Temperature is special.
  const UNITS = {
    Length: [["m", 1], ["km", 1000], ["cm", 0.01], ["mm", 0.001], ["mi", 1609.344], ["yd", 0.9144], ["ft", 0.3048], ["in", 0.0254], ["nmi", 1852]],
    Weight: [["kg", 1], ["g", 0.001], ["mg", 1e-6], ["t", 1000], ["lb", 0.45359237], ["oz", 0.028349523125], ["st", 6.35029318]],
    Temperature: [["°C"], ["°F"], ["K"]],
    Volume: [["L", 1], ["mL", 0.001], ["m³", 1000], ["US gal", 3.785411784], ["UK gal", 4.54609], ["US cup", 0.2365882365], ["US fl oz", 0.0295735295625], ["tbsp", 0.01478676478125], ["tsp", 0.00492892159375]],
    Speed: [["km/h", 1], ["m/s", 3.6], ["mph", 1.609344], ["knot", 1.852]],
    Area: [["m²", 1], ["km²", 1e6], ["ha", 1e4], ["acre", 4046.8564224], ["ft²", 0.09290304], ["mi²", 2589988.110336]],
    Data: [["B", 1], ["KB", 1e3], ["MB", 1e6], ["GB", 1e9], ["TB", 1e12], ["KiB", 1024], ["MiB", 1048576], ["GiB", 1073741824], ["TiB", 1099511627776], ["bit", 0.125]],
    Time: [["s", 1], ["min", 60], ["h", 3600], ["day", 86400], ["week", 604800], ["year", 31557600]],
  };

  let category = $state("Length");
  let from = $state("km");
  let to = $state("mi");
  let amount = $state("1");

  let currencies = $derived(rates?.rates ? [rates.base ?? base, ...Object.keys(rates.rates)].sort() : []);
  let categories = $derived(currencies.length ? ["Currency", ...Object.keys(UNITS)] : Object.keys(UNITS));
  let units = $derived(category === "Currency" ? currencies : UNITS[category].map((u) => u[0]));

  function pickCategory(c) {
    category = c;
    const list = c === "Currency" ? currencies : UNITS[c].map((u) => u[0]);
    from = c === "Currency" ? (list.includes("USD") ? "USD" : list[0]) : list[0];
    to = c === "Currency" ? (list.includes("EUR") ? "EUR" : list[1]) : list[1] ?? list[0];
  }

  function temp(v, a, b) {
    const c = a === "°C" ? v : a === "°F" ? ((v - 32) * 5) / 9 : v - 273.15;
    return b === "°C" ? c : b === "°F" ? (c * 9) / 5 + 32 : c + 273.15;
  }
  const rate = (code) => (code === (rates?.base ?? base) ? 1 : rates?.rates?.[code]);

  let result = $derived.by(() => {
    const v = Number(String(amount).replace(",", "."));
    if (!isFinite(v) || amount === "") return "";
    let out;
    if (category === "Temperature") out = temp(v, from, to);
    else if (category === "Currency") {
      const a = rate(from), b = rate(to);
      if (!a || !b) return "";
      out = (v / a) * b;
    } else {
      const f = UNITS[category].find((u) => u[0] === from)?.[1];
      const t = UNITS[category].find((u) => u[0] === to)?.[1];
      if (!f || !t) return "";
      out = (v * f) / t;
    }
    const digits = Math.abs(out) >= 100 ? 2 : Math.abs(out) >= 1 ? 4 : 6;
    return out.toLocaleString(undefined, { maximumFractionDigits: digits });
  });
  function swap() {
    [from, to] = [to, from];
  }
</script>

<div class="conv">
  <select value={category} onchange={(e) => pickCategory(e.currentTarget.value)}>
    {#each categories as c}<option>{c}</option>{/each}
  </select>
  <div class="line">
    <input bind:value={amount} inputmode="decimal" />
    <select bind:value={from}>{#each units as u}<option>{u}</option>{/each}</select>
  </div>
  <button class="swap" onclick={swap} title="Swap">⇅</button>
  <div class="line">
    <div class="result">{result || "–"}</div>
    <select bind:value={to}>{#each units as u}<option>{u}</option>{/each}</select>
  </div>
  {#if category === "Currency" && rates?.date}<div class="note">Rates of {rates.date}, from Frankfurter</div>{/if}
</div>

<style>
  .conv { display: flex; flex-direction: column; gap: 6px; font-size: 13px; }
  .line { display: flex; gap: 6px; }
  input, select, .result {
    font: inherit; color: inherit; padding: 6px 8px; border-radius: 7px; min-width: 0;
    border: 1px solid color-mix(in srgb, currentColor 16%, transparent); background: color-mix(in srgb, currentColor 7%, transparent);
  }
  input, .result { flex: 1; }
  .line select { width: 92px; flex: none; }
  .result { font-weight: 700; font-size: 16px; font-variant-numeric: tabular-nums; overflow: hidden; text-overflow: ellipsis; }
  .swap { align-self: center; border: 0; background: none; color: var(--accent); cursor: pointer; font-size: 15px; padding: 0; }
  .note { font-size: 10px; opacity: 0.5; text-align: right; }
</style>
