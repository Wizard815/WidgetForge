import { describe, expect, it } from "vitest";
import { formatBytes, formatDate, formatDuration, lookup, render, truthy } from "./template";

const scope = {
  sys: { cpu: { usage: 42.345, cores: [{ usage: 10 }, { usage: 20 }] }, memory: { used: 8 * 1024 ** 3 }, uptime: 93784 },
  settings: { name: "Ada", show: false, city: "" },
  now: new Date(2026, 10, 20, 15, 4, 5).getTime(),
};

describe("templates", () => {
  it("looks up nested and indexed paths", () => {
    expect(lookup(scope, "sys.cpu.cores[1].usage")).toBe(20);
    expect(lookup(scope, "sys.nope.x")).toBeUndefined();
  });

  it("returns raw values for a single expression", () => {
    expect(render("{{ sys.cpu.usage }}", scope)).toBe(42.345);
    expect(render("  {{settings.show}} ", scope)).toBe(false);
  });

  it("interpolates and applies filters", () => {
    expect(render("CPU {{ sys.cpu.usage | fixed(0) }}%", scope)).toBe("CPU 42%");
    expect(render("{{ sys.cpu.usage | percent(1) }}", scope)).toBe("42.3%");
    expect(render("{{ sys.memory.used | bytes }}", scope)).toBe("8.0 GB");
    expect(render("{{ sys.uptime | duration }}", scope)).toBe("1d 2h");
    expect(render("{{ settings.city | default('Oslo') }}", scope)).toBe("Oslo");
    expect(render("{{ settings.name | upper }}!", scope)).toBe("ADA!");
    expect(render("{{ sys.cpu.cores | count }}", scope)).toBe(2);
    expect(render("{{ missing.path }}x", scope)).toBe("x");
    expect(render("{{ sys.cpu.usage | nosuchfilter }}", scope)).toBe(42.345);
  });

  it("formats dates", () => {
    expect(render("{{ now | time('HH:mm:ss') }}", scope)).toBe("15:04:05");
    expect(render("{{ now | time('h:mm A') }}", scope)).toBe("3:04 PM");
    expect(formatDate(scope.now, "YYYY-MM-DD [at] HH")).toBe("2026-11-20 at 15");
    expect(render("{{ now | date('D') }}", scope)).toBe("20");
  });

  it("formats numbers", () => {
    expect(formatBytes(512)).toBe("512 B");
    expect(formatBytes(1536)).toBe("1.5 KB");
    expect(formatDuration(59)).toBe("0m");
    expect(formatDuration(3700)).toBe("1h 1m");
  });

  it("truthiness for visible:", () => {
    expect(truthy("false")).toBe(false);
    expect(truthy("")).toBe(false);
    expect(truthy("yes")).toBe(true);
    expect(truthy(0)).toBe(false);
  });
});
