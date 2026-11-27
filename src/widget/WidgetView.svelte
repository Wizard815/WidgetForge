<script>
  // One widget instance: runs its data sources and renders its blocks or HTML.
  // Used on its own (Host) and for each widget inside a stack (StackView).
  import { onMount, setContext, untrack } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { api, userFile } from "../lib/api.js";
  import { bind, bindDeep, effectiveSettings, shiftToZone } from "../lib/bindings.js";
  import { parseFeed, parseIcs } from "../lib/parsers.js";
  import { sunAndMoon } from "../lib/astro.js";
  import Block from "./Block.svelte";
  import HtmlFrame from "./HtmlFrame.svelte";

  let { manifest, instance, inStack = false } = $props();

  let data = $state({});
  let errors = $state({});
  let tick = $state(0);
  let actionError = $state("");
  // Text typed into `input` blocks, readable in bindings as {{ input.<name> }}.
  let inputs = $state({});

  let settings = $derived(effectiveSettings(manifest, instance));
  let ctx = $derived({
    ...data,
    settings,
    instance: { id: instance.id },
    lookups: manifest.lookups ?? {},
    input: inputs,
    _tick: tick,
    _now: Date.now() + tick * 0,
    _errors: errors,
  });
  let firstError = $derived(actionError || Object.values(errors).find(Boolean) || "");

  const SKIP = Symbol("skip");
  const truthy = (v) => (Array.isArray(v) ? v.length > 0 : !!v);
  // The manifest and instance id are fixed for the life of this view (it is keyed by id).
  const sources = untrack(() => Object.entries(manifest.data ?? {}));

  /** Today's screen time from the hourly totals the core keeps. */
  function screenTimeToday(r) {
    if (!r.available) return r;
    const start = new Date();
    start.setHours(0, 0, 0, 0);
    const fromHour = Math.floor(start.getTime() / 3600000);
    const sum = (from, to) => {
      const apps = {};
      for (const [h, list] of Object.entries(r.hours ?? {})) {
        if (+h < from || +h >= to) continue;
        for (const [name, secs] of Object.entries(list)) apps[name] = (apps[name] ?? 0) + secs;
      }
      return apps;
    };
    const today = sum(fromHour, Infinity);
    const total = Object.values(today).reduce((a, b) => a + b, 0);
    const yesterday = Object.values(sum(fromHour - 24, fromHour)).reduce((a, b) => a + b, 0);
    const week = Object.values(sum(fromHour - 24 * 6, Infinity)).reduce((a, b) => a + b, 0);
    const apps = Object.entries(today)
      .map(([name, seconds]) => ({ name, seconds, share: total ? (seconds / total) * 100 : 0 }))
      .sort((a, b) => b.seconds - a.seconds);
    return { available: true, apps, total, yesterday, week_average: week / 7 };
  }

  /** Up/down checks for a list of "Name | address" lines. */
  async function runChecks(text) {
    const lines = String(text ?? "").split(/\n|,/).map((l) => l.trim()).filter(Boolean);
    const items = await Promise.all(
      lines.map(async (line) => {
        const [name, addr] = line.includes("|") ? line.split("|").map((x) => x.trim()) : [line.replace(/^\w+:\/\//, "").split(/[/:]/)[0], line];
        const started = performance.now();
        try {
          if (/^tcp:\/\//i.test(addr)) {
            const u = new URL(addr.replace(/^tcp:/i, "http:"));
            const r = await api.netLatency(instance.id, u.hostname, Number(u.port) || 80);
            return { name, address: addr, up: r.ok, ms: r.ms == null ? null : Math.round(r.ms), status: r.ok ? "open" : r.error };
          }
          const r = await api.netFetch(instance.id, { url: addr });
          return { name, address: addr, up: r.status > 0 && r.status < 500, ms: Math.round(performance.now() - started), status: r.status };
        } catch (e) {
          return { name, address: addr, up: false, ms: null, status: String(e) };
        }
      }),
    );
    const up = items.filter((i) => i.up).length;
    return { items, up, total: items.length, down: items.length - up, all_up: up === items.length };
  }

  /** Download (and optional upload) speed, kept as a history in storage. */
  async function speedTest(src) {
    const r = await api.speedTest(instance.id);
    if (!r.configured) return r;
    const history = (await api.storageGet(instance.id, "speed_history").catch(() => null)) ?? [];
    if (r.ok) {
      history.push({ at: Date.now(), down: r.down_mbps, up: r.up_mbps ?? null, ms: r.ping_ms ?? null });
      while (history.length > 96) history.shift();
      await api.storageSet(instance.id, "speed_history", history).catch(() => {});
    }
    return { ...r, history, last: history[history.length - 1] ?? null };
  }

  async function netRequest(src, c) {
    // Calendar links often use webcal://, which is plain https underneath.
    const url = String(bind(src.url, c) ?? "").trim().replace(/^webcals?:\/\//i, "https://");
    if (!url) return SKIP;
    const headers = {};
    for (const [k, v] of Object.entries(src.headers ?? {})) {
      const name = String(bind(k, c) ?? "").trim();
      if (name) headers[name] = String(bind(v, c) ?? "");
    }
    const body = src.body === undefined ? undefined
      : typeof src.body === "string" ? String(bind(src.body, c)) : JSON.stringify(bindDeep(src.body, c));
    const started = performance.now();
    let r;
    try {
      r = await api.netFetch(instance.id, { url, method: src.method, headers, body });
    } catch (e) {
      return { ok: false, status: 0, error: String(e), ms: Math.round(performance.now() - started) };
    }
    const out = { ok: r.ok, status: r.status, ms: Math.round(performance.now() - started), json: r.json, text: r.text, cookie: r.cookie };
    if (!r.ok) out.error = `HTTP ${r.status}`;
    try {
      if (r.ok && src.format === "rss") Object.assign(out, parseFeed(r.text ?? ""));
      if (r.ok && src.format === "ics") Object.assign(out, parseIcs(r.text ?? "", { days: Number(bind(src.days, c)) || 60 }));
    } catch (e) {
      out.ok = false;
      out.error = String(e.message ?? e);
    }
    return out;
  }

  async function read(src, arg) {
    // `when:` waits for another value (a sign-in, a looked-up city) or a setting.
    if (src.when !== undefined && !truthy(bind(src.when, { ...ctx, _now: Date.now(), arg }))) return SKIP;
    switch (src.source) {
      case "clock": {
        // `timezone:` (e.g. "Asia/Tokyo") makes a world clock; empty = this computer's time.
        const tz = String(bind(src.timezone, { ...ctx }) ?? "").trim();
        let now = Date.now();
        if (tz) {
          try {
            now = shiftToZone(now, tz);
          } catch {
            throw `unknown time zone "${tz}"`;
          }
        }
        return { now, real: Date.now(), timezone: tz || Intl.DateTimeFormat().resolvedOptions().timeZone, locale: navigator.language };
      }
      case "folder": {
        const key = src.setting ?? "folder";
        const r = await api.folderList(instance.id, key, src.kind);
        return { ...r, files: r.files.map((f) => ({ ...f, url: userFile(instance.id, key, f.name) })) };
      }
      case "logfile":
        return api.tailFile(instance.id, src.setting ?? "file", Number(bind(src.lines, { ...ctx })) || 50);
      case "minecraft":
        return api.minecraftStatus(instance.id, src.setting ?? "server");
      case "screentime":
        return screenTimeToday(await api.screenTime(instance.id));
      case "mail":
        return api.mailCheck(instance.id);
      case "speedtest":
        return speedTest(src);
      case "checks":
        return runChecks(bind(src.list, { ...ctx }));
      case "sun": {
        const c = { ...ctx };
        const lat = Number(bind(src.lat, c)), lon = Number(bind(src.lon, c));
        if (!isFinite(lat) || !isFinite(lon) || (bind(src.lat, c) ?? "") === "") return { configured: false };
        return { configured: true, ...sunAndMoon(new Date(), lat, lon) };
      }
      case "storage":
        return { value: await api.storageGet(instance.id, src.key) };
      case "latency": {
        const c = { ...ctx };
        const host = String(bind(src.host, c) ?? "").trim();
        if (!host) return SKIP;
        return api.netLatency(instance.id, host, Number(bind(src.port, c)) || 443);
      }
      case "audio":
        return api.audioStatus(instance.id);
      case "docker":
        return api.dockerList(instance.id);
      case "command":
        return api.runCommand(instance.id, src.setting);
      case "system":
        return api.systemStats(instance.id);
      case "media":
        return api.mediaStatus(instance.id);
      case "clipboard":
        return api.clipboardHistory(instance.id);
      case "discord":
        return api.discordApi(instance.id, src.endpoint);
      case "net": {
        const c = { ...ctx, _now: Date.now(), arg };
        // `for_each`: the same request once per item (a list of parcels, stock symbols, entities).
        if (src.for_each !== undefined) {
          const list = bind(src.for_each, c);
          const items = (Array.isArray(list) ? list : []).slice(0, Number(src.limit ?? 20));
          if (!items.length) return SKIP;
          const results = (await Promise.all(items.map((item) => netRequest(src, { ...c, item })))).map((r) => (r === SKIP ? { ok: false, error: "no address" } : r));
          const ok = results.filter((r) => r.ok).length;
          return { ok: ok > 0 || results.length === 0, error: ok ? undefined : results[0]?.error, items: results.map((r, i) => ({ ...r, item: items[i] })) };
        }
        return netRequest(src, c);
      }
      default:
        throw `source "${src.source}" is not available`;
    }
  }

  // ---- scheduling: each source on its own interval; restarted when settings change ----
  let generation = 0;
  let timers = [];
  const busy = new Set();

  async function run(name, src, gen, arg) {
    if (busy.has(name)) return;
    busy.add(name);
    try {
      const v = await read(src, arg);
      if (gen !== generation || v === SKIP) return;
      data[name] = v;
      errors[name] = v && v.ok === false ? v.error || "request failed" : undefined;
      tick++;
      // A source waiting on this one (`when:`) can run now.
      for (const [n, s] of sources) {
        if (n !== name && !s.manual && s.when !== undefined && data[n] === undefined) run(n, s, gen);
      }
    } catch (e) {
      if (gen === generation) errors[name] = String(e);
    } finally {
      busy.delete(name);
    }
  }

  function start() {
    stop();
    const gen = ++generation;
    data = {};
    errors = {};
    for (const [name, src] of sources) {
      // `manual: true` sources run only from a button (action: fetch.<name>).
      if (src.manual) continue;
      // The interval may come from settings, e.g. "{{ settings.minutes | mul(60000) }}".
      const every = Math.max(250, Number(bind(src.interval, ctx)) || (src.source === "net" ? 600000 : 1000));
      run(name, src, gen);
      timers.push(setInterval(() => run(name, src, gen), every));
    }
  }
  function stop() {
    timers.forEach(clearInterval);
    timers = [];
  }
  function refreshAll() {
    for (const [name, src] of sources) if (!src.manual) run(name, src, generation);
  }

  /** Runs one source now (a button's `fetch.<name>`), then refreshes the rest. */
  async function runSource(name, arg) {
    const src = manifest.data?.[name];
    if (!src) throw `no data source called "${name}"`;
    await run(name, src, generation, arg);
    if (errors[name]) throw errors[name];
    if (src.manual) setTimeout(refreshAll, Number(src.refresh_after ?? 400));
  }

  setContext("forge", {
    instanceId: untrack(() => instance.id),
    widgetId: untrack(() => manifest.id),
    refresh: refreshAll,
    runSource,
    setInput: (name, value) => (inputs[name] = value),
    report: (msg) => {
      actionError = msg;
      setTimeout(() => (actionError = ""), 5000);
    },
  });

  // Restart sources when the settings change (a new city, a new URL, ...).
  let settingsKey = $derived(JSON.stringify(settings));
  let started = false;
  $effect(() => {
    settingsKey;
    if (started) start();
  });

  onMount(() => {
    started = true;
    start();
    // Signing in or out of Discord in the Hub refreshes Discord widgets at once.
    const off = sources.some(([, s]) => s.source === "discord") ? listen("discord-changed", refreshAll) : null;
    return () => {
      stop();
      off?.then((f) => f());
    };
  });
</script>

<div
  class="view"
  class:in-stack={inStack}
  style="--accent: {manifest.style?.accent ?? '#7cc4ff'}; {inStack ? `min-height: ${manifest.window?.stack_height ?? 0}px` : ''}"
>
  {#if manifest.ui?.entry}
    <HtmlFrame {manifest} {instance} {settings} />
  {:else}
    {#each manifest.ui?.blocks ?? [] as block}
      <Block {block} {ctx} />
    {/each}
  {/if}
  {#if firstError && manifest.ui?.show_errors !== false}
    <div class="error" title={firstError}>⚠ {firstError}</div>
  {/if}
</div>

<style>
  .view { position: relative; display: flex; flex-direction: column; gap: 8px; flex: 1; min-height: 0; }
  .view.in-stack { flex: none; }
  .error {
    font-size: 11px;
    color: var(--wf-bad);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    opacity: 0.85;
  }
</style>
