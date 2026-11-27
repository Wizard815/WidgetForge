<script>
  // Countdown, stopwatch, or Pomodoro (focus / break cycles).
  // State is saved to the widget's storage when it has storage permission,
  // so a running timer survives closing the widget.
  import { getContext, onMount } from "svelte";
  import { api } from "../../lib/api.js";

  let { mode = "countdown", minutes = 5, work = 25, rest = 5, longRest = 15, labels } = $props();
  const forge = getContext("forge");

  // running: started at `since` with `banked` ms already counted.
  let running = $state(false);
  let since = $state(0);
  let banked = $state(0);
  let phase = $state("work"); // pomodoro: work | rest
  let rounds = $state(0);
  let now = $state(Date.now());
  let done = $state(false);

  let elapsed = $derived(banked + (running ? now - since : 0));
  let target = $derived(
    mode === "stopwatch" ? 0
    : mode === "pomodoro" ? (phase === "work" ? work : rounds % 4 === 0 && rounds > 0 ? longRest : rest) * 60000
    : minutes * 60000,
  );
  let shown = $derived(mode === "stopwatch" ? elapsed : Math.max(0, target - elapsed));
  let progress = $derived(target ? Math.min(1, elapsed / target) : 0);

  function fmt(ms) {
    const total = Math.ceil(ms / 1000);
    const h = Math.floor(total / 3600);
    const m = Math.floor((total % 3600) / 60);
    const s = total % 60;
    const pad = (n) => String(n).padStart(2, "0");
    return h ? `${h}:${pad(m)}:${pad(s)}` : `${pad(m)}:${pad(s)}`;
  }

  const save = () =>
    api.storageSet(forge.instanceId, "timer", { mode, running, since, banked, phase, rounds }).catch(() => {});

  function start() {
    if (running) return;
    if (done) reset();
    since = Date.now();
    running = true;
    save();
  }
  function pause() {
    if (!running) return;
    banked += Date.now() - since;
    running = false;
    save();
  }
  function reset() {
    running = false;
    banked = 0;
    done = false;
    save();
  }
  function skip() {
    nextPhase();
  }
  function nextPhase() {
    if (phase === "work") rounds += 1;
    phase = phase === "work" ? "rest" : "work";
    banked = 0;
    since = Date.now();
    save();
  }

  onMount(() => {
    api.storageGet(forge.instanceId, "timer").then((s) => {
      if (s && s.mode === mode) ({ running, since, banked, phase, rounds } = s);
    }).catch(() => {});
    const t = setInterval(() => {
      now = Date.now();
      if (running && target && elapsed >= target) {
        if (mode === "pomodoro") {
          nextPhase();
          flash();
        } else {
          banked = target;
          running = false;
          done = true;
          save();
          flash();
        }
      }
    }, 250);
    return () => clearInterval(t);
  });

  let flashing = $state(false);
  function flash() {
    flashing = true;
    setTimeout(() => (flashing = false), 4000);
  }
</script>

<div class="timer" class:flashing class:done>
  {#if mode === "pomodoro"}
    <div class="phase">{phase === "work" ? labels.work : labels.rest} · {rounds}</div>
  {/if}
  <div class="time">{fmt(shown)}</div>
  {#if target}
    <div class="track"><div class="fill" style="width: {progress * 100}%"></div></div>
  {/if}
  <div class="buttons">
    {#if running}
      <button onclick={pause} title="Pause">❚❚</button>
    {:else}
      <button onclick={start} title="Start">▶</button>
    {/if}
    <button onclick={reset} title="Reset">↺</button>
    {#if mode === "pomodoro"}<button onclick={skip} title="Skip">⏭</button>{/if}
  </div>
</div>

<style>
  .timer { display: flex; flex-direction: column; align-items: center; gap: 8px; padding: 4px 0; border-radius: 10px; }
  .phase { font-size: 12px; opacity: 0.7; text-transform: uppercase; letter-spacing: 0.08em; }
  .time { font-size: 40px; font-weight: 700; font-variant-numeric: tabular-nums; line-height: 1; }
  .track { width: 100%; height: 5px; border-radius: 3px; background: color-mix(in srgb, currentColor 12%, transparent); overflow: hidden; }
  .fill { height: 100%; background: var(--accent); transition: width 0.25s linear; }
  .buttons { display: flex; gap: 6px; }
  button {
    width: 40px; height: 32px; border-radius: 8px; cursor: pointer;
    border: 1px solid color-mix(in srgb, currentColor 16%, transparent); background: color-mix(in srgb, currentColor 8%, transparent); color: inherit;
  }
  button:hover { border-color: var(--accent); }
  .done .time { color: var(--accent); }
  .flashing { animation: flash 0.5s ease-in-out 8 alternate; }
  @keyframes flash { from { background: transparent; } to { background: color-mix(in srgb, var(--accent) 35%, transparent); } }
</style>
