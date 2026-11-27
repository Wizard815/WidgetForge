import { test } from "node:test";
import assert from "node:assert/strict";
import { bind } from "../src/lib/bindings.js";
import { parseIcs } from "../src/lib/parsers.js";
import { calculate } from "../src/lib/calc.js";

test("index paths and expression arguments", () => {
  const ctx = { d: { max: [10, 20], code: [0, 61] }, row: "2026-11-27", index: 1, lookups: { wmo: { "61": "Light rain", default: "?" } } };
  assert.equal(bind("{{ d.max[index] }}", ctx), 20);
  assert.equal(bind("{{ d.code[index] | lookup('wmo') }}", ctx), "Light rain");
  assert.equal(bind("{{ d.code[0] | lookup('wmo') }}", ctx), "?");
  assert.equal(bind("{{ row | date('YYYY-MM-DD') }}", ctx), "2026-11-27"); // local day, not UTC
  assert.equal(bind("{{ x | eq(null) | choose('none', d.max[0] | fixed(1)) }}", { ...ctx, x: 5 }), "10.0");
});

test("sum, div, path, duration_clock, url", () => {
  const data = [{ cost: { amount: "41280.0" } }, { cost: { amount: "720.0" } }];
  assert.equal(bind("{{ data | sum('cost.amount') | div(100) | fixed(2) }}", { data }), "420.00");
  const nested = [{ results: [{ amount: { value: 0.5 } }, { amount: { value: 1 } }] }, { results: [] }];
  assert.equal(bind("{{ n | sum('results.amount.value') }}", { n: nested }), 1.5);
  assert.equal(bind("{{ j | path(p) }}", { j: { a: { b: [7, 8] } }, p: "a.b[1]" }), 8);
  assert.equal(bind("{{ 125 | duration_clock }}", {}), "2:05");
  assert.equal(bind("{{ c | url }}", { c: "São Paulo" }), "S%C3%A3o%20Paulo");
});

test("unix start of UTC day and month", () => {
  const now = Date.UTC(2026, 10, 26, 15, 30);
  assert.equal(bind("{{ t | unix('day') }}", { t: now }), Date.UTC(2026, 10, 26) / 1000);
  assert.equal(bind("{{ t | unix('month') }}", { t: now }), Date.UTC(2026, 10, 1) / 1000);
  assert.equal(bind("{{ t | utcdate('YYYY-MM') }}", { t: now }), "2026-11");
});

test("ICS: single, all-day, weekly repeat, exception, cancelled", () => {
  const ics = [
    "BEGIN:VCALENDAR",
    "BEGIN:VEVENT", "SUMMARY:Dentist", "DTSTART:20261127T150000", "DTEND:20261127T160000", "END:VEVENT",
    "BEGIN:VEVENT", "SUMMARY:Holiday", "DTSTART;VALUE=DATE:20261128", "END:VEVENT",
    "BEGIN:VEVENT", "SUMMARY:Standup", "DTSTART:20261123T090000", "DTEND:20261123T091500",
    "RRULE:FREQ=WEEKLY;BYDAY=MO,WE;COUNT=6", "EXDATE:20261202T090000", "END:VEVENT",
    "BEGIN:VEVENT", "SUMMARY:Gone", "STATUS:CANCELLED", "DTSTART:20261129T100000", "END:VEVENT",
    "BEGIN:VEVENT", "SUMMARY:Long", "DTSTART:20261126T080000", "DESCRIPTION:line one", " continued", "END:VEVENT",
    "END:VCALENDAR",
  ].join("\r\n");
  const now = new Date(2026, 10, 26, 12, 0).getTime();
  const { events } = parseIcs(ics, { now, days: 30 });
  const names = events.map((e) => `${e.summary}@${e.day} ${e.time}`);
  assert.deepEqual(names, [
    "Long@Thu 26 Nov 08:00",
    "Dentist@Fri 27 Nov 15:00",
    "Holiday@Sat 28 Nov ",
    "Standup@Mon 30 Nov 09:00",
    "Standup@Mon 7 Dec 09:00",
    "Standup@Wed 9 Dec 09:00",
  ]);
});

test("calculator", () => {
  assert.equal(calculate("1 + 2 × 3"), 7);
  assert.equal(calculate("(1 + 2) * 3"), 9);
  assert.equal(calculate("2 ^ 10"), 1024);
  assert.equal(calculate("-3 + 5"), 2);
  assert.equal(calculate("0.1 + 0.2"), 0.3);
  assert.equal(calculate("10 % 4"), 2);
  assert.throws(() => calculate("2 +"));
  assert.throws(() => calculate("alert(1)"));
  assert.throws(() => calculate("1 / 0"));
});

test("replace filter tidies Discord mentions", () => {
  const ctx = { c: "hi <@123>, see <#456> and <@!789>" };
  assert.equal(bind("{{ c | replace('<@!?&?\\d+>', '@…') | replace('<#\\d+>', '#…') }}", ctx), "hi @…, see #… and @…");
  assert.equal(bind("{{ c | replace('(', 'x') }}", ctx), ctx.c); // a bad pattern leaves text alone
});

test("sort filter", () => {
  const v = [{ n: "b", c: 2 }, { n: "a", c: 10 }, { n: "c", c: 1 }];
  assert.deepEqual(bind("{{ v | sort('c', 'desc') }}", { v }).map((x) => x.n), ["a", "b", "c"]);
  assert.deepEqual(bind("{{ v | sort('n') }}", { v }).map((x) => x.n), ["a", "b", "c"]);
  assert.deepEqual(
    bind("{{ s | eq('name') | choose(v | sort('n'), v | sort(s | eq('x') | choose('n', 'c'), 'desc')) }}", { v, s: "online" }).map((x) => x.n),
    ["a", "b", "c"],
  );
});

import { shiftToZone, nextOccurrence } from "../src/lib/bindings.js";

test("world clock shifts to another zone", () => {
  const t = Date.UTC(2026, 10, 26, 12, 0, 0); // 12:00 UTC
  const tokyo = new Date(shiftToZone(t, "Asia/Tokyo"));
  const ny = new Date(shiftToZone(t, "America/New_York"));
  assert.equal(`${tokyo.getHours()}:${tokyo.getMinutes()}`, "21:0"); // UTC+9
  assert.equal(`${ny.getHours()}:${ny.getMinutes()}`, "7:0"); // UTC-5 in November
  assert.throws(() => shiftToZone(t, "Mars/Olympus"));
  assert.equal(shiftToZone(t, ""), t);
});

test("countdown, days_until and yearly dates", () => {
  const now = new Date();
  const inThree = new Date(now.getFullYear(), now.getMonth(), now.getDate() + 3, 12);
  assert.equal(bind("{{ d | days_until }}", { d: inThree.toISOString() }), 3);
  assert.match(bind("{{ d | countdown }}", { d: inThree.toISOString() }), /^\d+d \d+h \d+m$/);
  const past = new Date(now.getFullYear() - 1, now.getMonth(), now.getDate() - 2);
  assert.equal(bind("{{ d | countdown }}", { d: past.toISOString() }), "");
  // A birthday two days ago comes round again next year.
  const next = new Date(nextOccurrence(past, now.getTime(), true));
  assert.equal(next.getFullYear(), now.getFullYear() + 1);
});

test("utcparse reads zone-less UTC timestamps", () => {
  assert.equal(bind("{{ t | utcparse }}", { t: "2026-11-29T16:30:00" }), Date.UTC(2026, 10, 29, 16, 30));
  assert.equal(bind("{{ t | utcparse }}", { t: "2026-11-29T16:30:00Z" }), Date.UTC(2026, 10, 29, 16, 30));
});
