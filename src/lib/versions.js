// Comparing widget versions ("1.10.0" is newer than "1.9.2").

/** -1, 0 or 1. Pre-release parts ("1.0.0-beta") sort before the release. */
export function compareVersions(a, b) {
  const split = (v) => {
    const [main, pre = ""] = String(v ?? "0").split(/[-+]/, 2);
    return { nums: main.split(".").map((n) => parseInt(n, 10) || 0), pre };
  };
  const x = split(a), y = split(b);
  for (let i = 0; i < Math.max(x.nums.length, y.nums.length); i++) {
    const d = (x.nums[i] ?? 0) - (y.nums[i] ?? 0);
    if (d) return Math.sign(d);
  }
  if (x.pre === y.pre) return 0;
  if (!x.pre) return 1;
  if (!y.pre) return -1;
  return x.pre < y.pre ? -1 : 1;
}

/** Plain words for a manifest's permissions (short, for chips). */
export function permissionChips(p = {}) {
  const out = [];
  for (const s of p.system ?? []) out.push(`reads ${s}`);
  for (const h of p.network ?? []) out.push(h.startsWith("setting:") ? "web address you type" : `reaches ${h}`);
  if (p.run) out.push("runs a command you type");
  if (p.folders) out.push("reads a folder you choose");
  if (p.files) out.push("reads a file you choose");
  const flags = {
    storage: "saves notes", clipboard: "clipboard", media: "media control", audio: "volume", docker: "Docker",
    latency: "tests connections", discord: "Discord sign-in", wake: "wakes computers", game: "game servers",
    mail: "mailbox", activity: "app usage", notify: "notifications", launch: "opens files",
  };
  for (const [k, v] of Object.entries(flags)) if (p[k] === true) out.push(v);
  return out;
}
