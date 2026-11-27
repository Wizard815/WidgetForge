// The look of every widget: an app-wide theme (Hub → Theme), the widget's own
// colours, and the user's overrides for one widget, turned into CSS variables.

// PLACEHOLDER wording: preset names.
export const PRESETS = {
  glass: { name: "Glass", background: "#101520", opacity: 0.78, text: "#eef2f8", border: true, shadow: false, radius: 14 },
  solid: { name: "Solid", background: "#1c2230", opacity: 1, text: "#eef2f8", border: false, shadow: false, radius: 12 },
  light: { name: "Light", background: "#f4f6fa", opacity: 0.94, text: "#1b2130", border: true, shadow: false, radius: 14 },
  clear: { name: "Clear", background: "#000000", opacity: 0, text: "#ffffff", border: false, shadow: true, radius: 0 },
  contrast: { name: "High contrast", background: "#000000", opacity: 1, text: "#ffffff", border: true, shadow: false, radius: 6, accent: "#ffd400", accent_mode: "theme" },
  paper: { name: "Paper", background: "#f3ead8", opacity: 1, text: "#3b3024", border: false, shadow: false, radius: 4, accent: "#b5562b", accent_mode: "theme", font: "serif" },
};

export const DEFAULT_THEME = { preset: "glass", ...PRESETS.glass, font: "system", scale: 1, accent_mode: "widget", accent: "#7cc4ff" };

export const FONTS = {
  system: 'system-ui, "Segoe UI", sans-serif',
  rounded: 'ui-rounded, "Nunito", "Varela Round", "SF Pro Rounded", system-ui, sans-serif',
  serif: 'Georgia, "Iowan Old Style", "Times New Roman", serif',
  mono: 'ui-monospace, "Cascadia Mono", "DejaVu Sans Mono", monospace',
  condensed: '"Roboto Condensed", "Arial Narrow", "DejaVu Sans Condensed", sans-serif',
};

/** The theme with every key filled in. */
export function fullTheme(t) {
  const preset = PRESETS[t?.preset] ?? {};
  const out = { ...DEFAULT_THEME, ...preset, ...(t ?? {}) };
  delete out.name;
  return out;
}

function rgb(hex) {
  const m = /^#?([0-9a-f]{6})$/i.exec(String(hex ?? ""));
  if (!m) return [0, 0, 0];
  const n = parseInt(m[1], 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

/** Perceived brightness 0..1. */
export function brightness(hex) {
  const [r, g, b] = rgb(hex);
  return (0.299 * r + 0.587 * g + 0.114 * b) / 255;
}

/**
 * What one widget should look like.
 * - frame: the user's choice for this widget ("theme", "glass", "solid", "clear", ...)
 *   or the widget's own (`style.theme: none` = no background, `solid`).
 * - accent: the user's for this widget, else the theme's (if the theme says so), else the widget's.
 */
export function look(theme, manifest, instance) {
  const t = fullTheme(theme);
  const own = instance?.style ?? {};
  let base = t;
  const frame = own.frame ?? "theme";
  if (frame !== "theme" && PRESETS[frame]) base = { ...t, ...PRESETS[frame] };
  else if (frame === "theme" && manifest?.style?.theme === "none") base = { ...t, opacity: 0, border: false, shadow: true };
  const opacity = typeof own.opacity === "number" ? own.opacity : base.opacity;
  const accent = own.accent || (t.accent_mode === "theme" ? t.accent : manifest?.style?.accent || t.accent);
  // What the text sits on: the background, or (see-through) the desktop, assumed dark.
  const light = opacity >= 0.45 && brightness(base.background) > 0.6;
  const text = opacity < 0.45 ? (own.text || "#ffffff") : base.text;
  return { ...base, opacity, accent, light, text, font: FONTS[t.font] ?? FONTS.system, scale: t.scale };
}

/** CSS custom properties for a widget's frame. */
export function lookVars(l) {
  const [r, g, b] = rgb(l.background);
  return [
    `--wf-bg: rgba(${r}, ${g}, ${b}, ${l.opacity})`,
    `--wf-text: ${l.text}`,
    `--wf-border: ${l.border ? `color-mix(in srgb, ${l.text} 14%, transparent)` : "transparent"}`,
    `--wf-radius: ${l.radius}px`,
    `--wf-font: ${l.font}`,
    `--wf-scale: ${l.scale}`,
    `--wf-shadow: ${l.shadow ? "0 1px 3px rgba(0,0,0,.85)" : "none"}`,
    `--accent: ${l.accent}`,
    `--wf-good: ${l.light ? "#16803f" : "#6cf0a0"}`,
    `--wf-bad: ${l.light ? "#c0392b" : "#ff8080"}`,
    `--wf-warn: ${l.light ? "#a0670a" : "#ffd166"}`,
    `--wf-muted: color-mix(in srgb, ${l.text} 60%, transparent)`,
    `--wf-on-accent: ${brightness(l.accent) > 0.55 ? "#111" : "#fff"}`,
    `color-scheme: ${l.light ? "light" : "dark"}`,
  ].join("; ");
}
