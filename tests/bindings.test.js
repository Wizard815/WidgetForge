import { test } from "node:test";
import assert from "node:assert/strict";
import { bind, isVisible, effectiveSettings, formatDate } from "../src/lib/bindings.js";

const ctx = {
  sys: { cpu: { usage: 42.567, count: 8 }, memory: { used: 3 * 1024 ** 3, total: 16 * 1024 ** 3 }, uptime: 93784 },
  settings: { show: true, label: "", title: "Hi" },
  list: [{ a: 1 }, { a: 2 }],
};

test("a single binding keeps its type", () => {
  assert.equal(bind("{{ sys.cpu.usage }}", ctx), 42.567);
  assert.equal(bind("{{ settings.show }}", ctx), true);
});
test("mixed text becomes a string", () => {
  assert.equal(bind("CPU {{ sys.cpu.usage | fixed(0) }}%", ctx), "CPU 43%");
});
test("indexing", () => assert.equal(bind("{{ list[1].a }}", ctx), 2));
test("bytes, rate, duration", () => {
  assert.equal(bind("{{ sys.memory.used | bytes }} / {{ sys.memory.total | bytes(0) }}", ctx), "3.0 GB / 16 GB");
  assert.equal(bind("{{ 2048 | rate }}", ctx), "2.0 KB/s");
  assert.equal(bind("{{ sys.uptime | duration }}", ctx), "1d 2h");
});
test("not, default, count, upper", () => {
  assert.equal(bind("{{ settings.show | not }}", ctx), false);
  assert.equal(bind("{{ settings.label | default('none') }}", ctx), "none");
  assert.equal(bind("{{ list | count }}", ctx), 2);
  assert.equal(bind("{{ settings.title | upper }}", ctx), "HI");
});
test("visible", () => {
  assert.equal(isVisible({}, ctx), true);
  assert.equal(isVisible({ visible: "{{ settings.label }}" }, ctx), false);
  assert.equal(isVisible({ visible: "{{ settings.title }}" }, ctx), true);
});
test("dates", () => {
  const d = new Date(2026, 10, 26, 15, 4, 9);
  assert.equal(formatDate(d, "YYYY-MM-DD HH:mm:ss"), "2026-11-26 15:04:09");
  assert.equal(formatDate(d, "h:mm A"), "3:04 PM");
  assert.equal(formatDate(d, "[Day] D"), "Day 26");
});
test("settings defaults", () => {
  const m = { settings: [{ key: "a", default: 1 }, { key: "b", default: true }] };
  assert.deepEqual(effectiveSettings(m, { settings: { a: 5 } }), { a: 5, b: true });
});
