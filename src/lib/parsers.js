// Turns fetched text into data widgets can bind to.
//   rss: RSS 2.0 or Atom  -> { title, items: [{ title, link, date, summary }] }
//   ics: iCalendar        -> { events: [{ summary, start, end, allDay, location, time }] }

import { formatDate } from "./bindings.js";

// ---------- RSS / Atom ----------

const text = (el, sel) => el.querySelector(sel)?.textContent?.trim() ?? "";
const stripTags = (s) => s.replace(/<[^>]*>/g, " ").replace(/\s+/g, " ").trim();

export function parseFeed(xml, DOMParserImpl = globalThis.DOMParser) {
  const doc = new DOMParserImpl().parseFromString(xml, "application/xml");
  if (doc.querySelector("parsererror")) throw new Error("not a valid RSS or Atom feed");
  const atom = doc.querySelector("feed");
  if (atom) {
    return {
      title: text(atom, ":scope > title"),
      items: [...atom.querySelectorAll("entry")].map((e) => ({
        title: text(e, "title"),
        link: e.querySelector("link[rel=alternate]")?.getAttribute("href")
          ?? e.querySelector("link")?.getAttribute("href") ?? "",
        date: Date.parse(text(e, "published") || text(e, "updated")) || null,
        summary: stripTags(text(e, "summary") || text(e, "content")).slice(0, 300),
      })),
    };
  }
  const channel = doc.querySelector("channel");
  if (!channel) throw new Error("not a valid RSS or Atom feed");
  return {
    title: text(channel, ":scope > title"),
    items: [...channel.querySelectorAll("item")].map((i) => ({
      title: text(i, "title"),
      link: text(i, "link"),
      date: Date.parse(text(i, "pubDate") || text(i, "date")) || null,
      summary: stripTags(text(i, "description")).slice(0, 300),
    })),
  };
}

// ---------- iCalendar ----------

/** Reads DTSTART-style values: 20261126, 20261126T150000, 20261126T150000Z. */
export function parseIcsDate(value, params = {}) {
  const m = value.match(/^(\d{4})(\d{2})(\d{2})(?:T(\d{2})(\d{2})(\d{2})(Z)?)?$/);
  if (!m) return null;
  const [, y, mo, d, h, mi, s, z] = m;
  if (h === undefined || params.VALUE === "DATE") {
    return { time: new Date(+y, +mo - 1, +d).getTime(), allDay: true };
  }
  // Times with a TZID are read as local time (good enough for most personal calendars).
  const t = z ? Date.UTC(+y, +mo - 1, +d, +h, +mi, +s) : new Date(+y, +mo - 1, +d, +h, +mi, +s).getTime();
  return { time: t, allDay: false };
}

function unfold(ics) {
  return ics.replace(/\r\n/g, "\n").replace(/\n[ \t]/g, "").split("\n");
}

function parseLine(line) {
  const colon = line.indexOf(":");
  if (colon < 0) return null;
  const [name, ...paramParts] = line.slice(0, colon).split(";");
  const params = {};
  for (const p of paramParts) {
    const [k, v] = p.split("=");
    params[k.toUpperCase()] = v;
  }
  return { name: name.toUpperCase(), params, value: line.slice(colon + 1) };
}

const unescape = (s) => s.replace(/\\n/gi, "\n").replace(/\\([,;\\])/g, "$1");
const DAYS = ["SU", "MO", "TU", "WE", "TH", "FR", "SA"];

/** Expands a simple RRULE (DAILY/WEEKLY/MONTHLY/YEARLY, INTERVAL, COUNT, UNTIL, weekly BYDAY). */
function expand(ev, rule, from, to, exdates) {
  const r = Object.fromEntries(rule.split(";").map((p) => p.split("=")));
  const interval = Math.max(1, Number(r.INTERVAL) || 1);
  const count = r.COUNT ? Number(r.COUNT) : Infinity;
  const until = r.UNTIL ? parseIcsDate(r.UNTIL)?.time ?? Infinity : Infinity;
  const duration = ev.end - ev.start;
  const byDay = r.BYDAY ? r.BYDAY.split(",").map((d) => DAYS.indexOf(d.slice(-2))).filter((d) => d >= 0) : null;
  const out = [];
  const base = new Date(ev.start);
  let n = 0;
  for (let step = 0; step < 2000 && n < count; step++) {
    const starts = [];
    const d = new Date(base);
    if (r.FREQ === "DAILY") d.setDate(base.getDate() + step * interval);
    else if (r.FREQ === "WEEKLY") d.setDate(base.getDate() + step * 7 * interval);
    else if (r.FREQ === "MONTHLY") d.setMonth(base.getMonth() + step * interval);
    else if (r.FREQ === "YEARLY") d.setFullYear(base.getFullYear() + step * interval);
    else return [ev];
    if (r.FREQ === "WEEKLY" && byDay) {
      const weekStart = new Date(d);
      weekStart.setDate(d.getDate() - d.getDay());
      for (const wd of byDay) {
        const x = new Date(weekStart);
        x.setDate(weekStart.getDate() + wd);
        if (x.getTime() >= ev.start) starts.push(x.getTime());
      }
    } else {
      starts.push(d.getTime());
    }
    for (const s of starts.sort((a, b) => a - b)) {
      if (s > until || n >= count) return out;
      n++;
      if (s > to) return out;
      if (s + duration >= from && !exdates.has(s)) out.push({ ...ev, start: s, end: s + duration });
    }
  }
  return out;
}

/** Upcoming events between now and `days` ahead, soonest first. */
export function parseIcs(ics, { now = Date.now(), days = 60, limit = 100 } = {}) {
  const from = new Date(now).setHours(0, 0, 0, 0);
  const to = now + days * 86400000;
  const events = [];
  let cur = null;
  for (const raw of unfold(ics)) {
    const line = parseLine(raw);
    if (!line) continue;
    if (line.name === "BEGIN" && line.value === "VEVENT") { cur = { exdates: new Set() }; continue; }
    if (line.name === "END" && line.value === "VEVENT") {
      if (cur?.start != null && cur.status !== "CANCELLED") {
        const ev = {
          summary: cur.summary ?? "",
          location: cur.location ?? "",
          start: cur.start,
          end: cur.end ?? cur.start + (cur.allDay ? 86400000 : 3600000),
          allDay: cur.allDay,
        };
        const list = cur.rrule ? expand(ev, cur.rrule, from, to, cur.exdates) : [ev];
        for (const e of list) if (e.end >= from && e.start <= to) events.push(e);
      }
      cur = null;
      continue;
    }
    if (!cur) continue;
    switch (line.name) {
      case "SUMMARY": cur.summary = unescape(line.value); break;
      case "LOCATION": cur.location = unescape(line.value); break;
      case "STATUS": cur.status = line.value.toUpperCase(); break;
      case "RRULE": cur.rrule = line.value; break;
      case "DTSTART": {
        const d = parseIcsDate(line.value, line.params);
        if (d) { cur.start = d.time; cur.allDay = d.allDay; }
        break;
      }
      case "DTEND": {
        const d = parseIcsDate(line.value, line.params);
        if (d) cur.end = d.time;
        break;
      }
      case "EXDATE":
        for (const v of line.value.split(",")) {
          const d = parseIcsDate(v, line.params);
          if (d) cur.exdates.add(d.time);
        }
        break;
    }
  }
  events.sort((a, b) => a.start - b.start);
  return {
    events: events.slice(0, limit).map((e) => ({
      ...e,
      time: e.allDay ? "" : formatDate(e.start, "HH:mm"),
      day: formatDate(e.start, "ddd D MMM"),
    })),
  };
}
