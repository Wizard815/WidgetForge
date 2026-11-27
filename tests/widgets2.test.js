import { test } from "node:test";
import assert from "node:assert/strict";
import { bind, bindDeep } from "../src/lib/bindings.js";
import { sunTimes, moonPhase } from "../src/lib/astro.js";

test("sunrise and sunset in London at midsummer", () => {
  const t = sunTimes(new Date(2026, 5, 21), 51.5074, -0.1278);
  const utc = (ms) => new Date(ms).getUTCHours() * 60 + new Date(ms).getUTCMinutes();
  // Published times: sunrise 03:43 UTC, sunset 20:21 UTC.
  assert.ok(Math.abs(utc(t.sunrise) - (3 * 60 + 43)) <= 4, `sunrise ${new Date(t.sunrise).toISOString()}`);
  assert.ok(Math.abs(utc(t.sunset) - (20 * 60 + 21)) <= 4, `sunset ${new Date(t.sunset).toISOString()}`);
  assert.ok(t.day_length > 16.5 * 3600 && t.day_length < 16.7 * 3600);
});

test("polar night has no sunrise", () => {
  const t = sunTimes(new Date(2026, 11, 21), 78.22, 15.65); // Svalbard
  assert.equal(t.sunrise, null);
  assert.equal(t.polar, "night");
});

test("moon phase near a known full moon", () => {
  const m = moonPhase(Date.UTC(2025, 0, 13, 22, 27)); // full moon, 13 Jan 2025
  assert.equal(m.name, "Full moon");
  assert.ok(m.illumination > 97);
  const n = moonPhase(Date.UTC(2025, 0, 29, 12, 36)); // new moon, 29 Jan 2025
  assert.equal(n.name, "New moon");
});

test("list filters", () => {
  const ctx = {
    rows: [{ id: "a", on: true, n: 2 }, { id: "b", on: false, n: 0 }, { id: "c", on: true, n: 5 }],
    text: "Plex | http://x\n\n  Pi | http://y  ",
    lights: { "1": { name: "Desk", state: { on: true } }, "2": { name: "Hall", state: { on: false } } },
    prices: { usd: 10, usd_24h_change: -1.25 },
  };
  assert.deepEqual(bind("{{ rows | pluck('id') | join('-') }}", ctx), "a-b-c");
  assert.equal(bind("{{ rows | where('on') | count }}", ctx), 2);
  assert.equal(bind("{{ rows | except('n', 0) | count }}", ctx), 2);
  assert.deepEqual(bind("{{ rows | pick('c, a') | pluck('id') }}", ctx), ["c", "a"]);
  assert.deepEqual(bind("{{ text | lines }}", ctx), ["Plex | http://x", "Pi | http://y"]);
  assert.equal(bind("{{ text | lines | first | split('|') | slice(-1) | first }}", ctx), "http://x");
  const e = bind("{{ lights | entries }}", ctx);
  assert.equal(e[0].key, "1");
  assert.equal(e[0].name, "Desk");
  assert.equal(bind("{{ prices | get('usd', '_24h_change') | signed(1) }}", ctx), "−1.3");
  assert.equal(bind("{{ prices | get('usd', '_24h_change') | trend }}", ctx), "bad");
  assert.equal(bind("{{ 'on' | among('on', 'off') }}", ctx), true);
  assert.equal(bind("{{ 3725 | hm }}", ctx), "1h 02m");
  assert.equal(bind("{{ 95 | gte(90) | choose('bad', 'ok') }}", ctx), "bad");
  assert.equal(bind("{{ x | time }}", { x: null }), "");
});

test("deep binding keeps types", () => {
  const body = bindDeep({ on: "{{ arg.on | not }}", id: "{{ arg.id }}", n: 5, list: ["{{ arg.id }}"] }, { arg: { on: true, id: "x" } });
  assert.deepEqual(body, { on: false, id: "x", n: 5, list: ["x"] });
});
