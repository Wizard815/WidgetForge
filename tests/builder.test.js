import { test } from "node:test";
import assert from "node:assert/strict";
import {
  newDraft, buildManifest, draftFromManifest, insertBlock, removeBlock, moveBlock, updateBlock,
  outline, valuePaths, problems, newSource, binding, BLOCK_TYPES,
} from "../src/lib/builder.js";
import { look, lookVars, fullTheme } from "../src/lib/theme.js";

test("a new draft makes a valid manifest with only the permissions it needs", () => {
  const m = buildManifest(newDraft());
  assert.equal(m.schema, 1);
  assert.deepEqual(m.permissions, { system: ["cpu", "memory"] });
  assert.equal(m.data.pc.source, "system");
  assert.equal(m.data.pc.interval, 2000);
  assert.equal(m.ui.blocks.length, 2);
  assert.deepEqual(problems(newDraft()), []);
});

test("web, key and command sources", () => {
  const d = newDraft();
  d.sources = [
    { ...newSource("net"), url: "http://192.168.1.5:8123/api/states", use_key: true },
    newSource("command"),
  ];
  const m = buildManifest(d);
  assert.deepEqual(m.permissions.network, ["192.168.1.5:8123"]);
  assert.equal(m.data.web.headers.Authorization, "Bearer [[secret:web_key]]");
  assert.deepEqual(m.permissions.run, ["setting:command"]);
  assert.equal(m.data.cmd.setting, "command");
  const cmd = m.settings.find((s) => s.type === "command");
  assert.equal(cmd.default, undefined, "a command is never shipped in the widget");
  assert.ok(m.settings.some((s) => s.key === "web_key" && s.type === "secret"));
});

test("storage blocks ask for storage", () => {
  const d = newDraft();
  d.blocks.push(BLOCK_TYPES.notes.make());
  assert.equal(buildManifest(d).permissions.storage, true);
});

test("tree editing", () => {
  let blocks = [{ type: "text", text: "a" }, { type: "row", children: [] }];
  let path;
  [blocks, path] = insertBlock(blocks, [1], { type: "stat" });
  assert.deepEqual(path, [1, 0]);
  [blocks, path] = insertBlock(blocks, [0], { type: "divider" });
  assert.deepEqual(path, [1]);
  assert.equal(blocks[1].type, "divider");
  [blocks, path] = moveBlock(blocks, [1], -1);
  assert.equal(blocks[0].type, "divider");
  blocks = updateBlock(blocks, [1], { text: "b", size: "" });
  assert.equal(blocks[1].text, "b");
  assert.equal("size" in blocks[1], false);
  blocks = removeBlock(blocks, [2, 0]);
  assert.equal(blocks[2].children.length, 0);
  assert.deepEqual(outline(blocks).map((o) => o.depth), [0, 0, 0]);
});

test("round trip of a built-in widget keeps what the builder cannot edit", () => {
  const m = {
    schema: 1, id: "forge.weather", name: "Weather", version: "1.0.0", permissions: { network: ["api.open-meteo.com"], storage: true },
    settings: [{ key: "city", type: "text", default: "London" }],
    data: { geo: { source: "net", url: "https://x/?q={{ settings.city }}", interval: 1000 } },
    lookups: { wmo: { "0": "Clear" } },
    ui: { blocks: [{ type: "text", text: "{{ geo.json.name }}" }] },
  };
  const d = draftFromManifest(m, true);
  assert.equal(d.id, "my.weather");
  assert.equal(d.sources[0].kind, "raw");
  const back = buildManifest(d);
  assert.deepEqual(back.data.geo, m.data.geo);
  assert.deepEqual(back.lookups, m.lookups);
  assert.equal(back.settings[0].key, "city");
});

test("problems are explained", () => {
  const d = newDraft();
  d.id = "forge.x";
  d.sources.push({ ...newSource("net", ["pc"]), url: "ftp://x" });
  const p = problems(d);
  assert.ok(p.some((x) => x.includes("forge.")));
  assert.ok(p.some((x) => x.includes("https://")));
});

test("value picker paths", () => {
  const paths = valuePaths({ pc: { cpu: { usage: 12.5 }, processes: [{ name: "a", cpu: 1 }] } });
  assert.ok(paths.some((p) => p.path === "pc.cpu.usage" && p.sample === "12.5"));
  assert.ok(paths.some((p) => p.path === "pc.processes" && p.kind === "list"));
  assert.ok(paths.some((p) => p.path === "pc.processes[0].name"));
  assert.equal(binding("pc.cpu.usage", "fixed(0)"), "{{ pc.cpu.usage | fixed(0) }}");
});

test("theme: app theme, widget colours and per-widget overrides", () => {
  const manifest = { style: { accent: "#ff0000" } };
  assert.equal(look({}, manifest, {}).accent, "#ff0000");
  assert.equal(look({ accent_mode: "theme", accent: "#00ff00" }, manifest, {}).accent, "#00ff00");
  assert.equal(look({}, manifest, { style: { accent: "#0000ff" } }).accent, "#0000ff");
  const light = look({ preset: "light" }, manifest, {});
  assert.equal(light.light, true);
  assert.match(lookVars(light), /--wf-good: #16803f/);
  const seeThrough = look({ preset: "light" }, manifest, { style: { opacity: 0 } });
  assert.equal(seeThrough.light, false);
  assert.equal(seeThrough.text, "#ffffff");
  assert.equal(fullTheme({ preset: "contrast" }).accent, "#ffd400");
  assert.equal(look({}, { style: { theme: "none" } }, {}).opacity, 0);
});
