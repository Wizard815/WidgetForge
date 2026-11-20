//! `widget.yaml` parsing and validation (schema 1). See docs/DESIGN.md §5.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub const MANIFEST_FILE: &str = "widget.yaml";
pub const SCHEMA_VERSION: u32 = 1;

pub const SYSTEM_CATEGORIES: &[&str] = &["cpu", "memory", "disk", "network", "os", "battery", "gpu"];
pub const SOURCES: &[&str] = &["clock", "system"];
pub const SETTING_TYPES: &[&str] = &["text", "number", "bool", "choice", "color"];
pub const CATEGORIES: &[&str] = &["time", "productivity", "system", "media", "info", "fun", "other"];
pub const BLOCK_TYPES: &[&str] = &[
    "column", "row", "grid", "spacer", "text", "gauge", "bar", "graph", "list", "image", "clock",
    "button", "notes", "checklist",
];
const CONTAINER_BLOCKS: &[&str] = &["column", "row", "grid"];
const STORAGE_BLOCKS: &[&str] = &["notes", "checklist"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    #[default]
    Tile,
    Drawer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Layer {
    Top,
    Normal,
    #[default]
    Desktop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Edge {
    Left,
    #[default]
    Right,
    Top,
    Bottom,
}

fn default_size() -> [u32; 2] {
    [240, 160]
}
fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WindowSpec {
    #[serde(default)]
    pub mode: Mode,
    #[serde(default = "default_size")]
    pub size: [u32; 2],
    #[serde(default)]
    pub min_size: Option<[u32; 2]>,
    #[serde(default = "yes")]
    pub resizable: bool,
    #[serde(default)]
    pub layer: Layer,
    #[serde(default)]
    pub edge: Edge,
    #[serde(default)]
    pub multi_instance: bool,
}

impl Default for WindowSpec {
    fn default() -> Self {
        WindowSpec {
            mode: Mode::Tile,
            size: default_size(),
            min_size: None,
            resizable: true,
            layer: Layer::Desktop,
            edge: Edge::Right,
            multi_instance: false,
        }
    }
}

fn default_theme() -> String {
    "glass".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StyleSpec {
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default)]
    pub accent: Option<String>,
}

impl Default for StyleSpec {
    fn default() -> Self {
        StyleSpec { theme: default_theme(), accent: None }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Permissions {
    #[serde(default)]
    pub system: Vec<String>,
    #[serde(default)]
    pub network: Vec<String>,
    #[serde(default)]
    pub storage: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingDef {
    pub key: String,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub default: Value,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DataDef {
    pub source: String,
    #[serde(default)]
    pub interval: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ui {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocks: Option<Vec<Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entry: Option<String>,
}

fn default_category() -> String {
    "other".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub schema: u32,
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default = "default_category")]
    pub category: String,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub window: WindowSpec,
    #[serde(default)]
    pub style: StyleSpec,
    #[serde(default)]
    pub permissions: Permissions,
    #[serde(default)]
    pub settings: Vec<SettingDef>,
    #[serde(default)]
    pub data: BTreeMap<String, DataDef>,
    #[serde(default)]
    pub ui: Ui,
    /// Keys this version of WidgetForge does not know. Kept for forward compatibility.
    #[serde(flatten, skip_serializing)]
    pub extra: BTreeMap<String, Value>,
}

impl Manifest {
    /// Default values of all declared settings.
    pub fn default_settings(&self) -> serde_json::Map<String, Value> {
        self.settings.iter().map(|s| (s.key.clone(), s.default.clone())).collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    Error,
    Warning,
}

#[derive(Debug, Clone, Serialize)]
pub struct Issue {
    pub level: Level,
    pub path: String,
    pub message: String,
}

impl Issue {
    fn err(path: impl Into<String>, message: impl Into<String>) -> Self {
        Issue { level: Level::Error, path: path.into(), message: message.into() }
    }
    fn warn(path: impl Into<String>, message: impl Into<String>) -> Self {
        Issue { level: Level::Warning, path: path.into(), message: message.into() }
    }
}

impl std::fmt::Display for Issue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let lvl = match self.level {
            Level::Error => "error",
            Level::Warning => "warning",
        };
        if self.path.is_empty() {
            write!(f, "{lvl}: {}", self.message)
        } else {
            write!(f, "{lvl}: {}: {}", self.path, self.message)
        }
    }
}

pub fn has_errors(issues: &[Issue]) -> bool {
    issues.iter().any(|i| i.level == Level::Error)
}

/// Parses YAML text into a manifest and validates it. Returns the manifest (if it could be
/// parsed) and all issues found. Callers must treat any `Level::Error` as fatal.
pub fn parse(text: &str) -> (Option<Manifest>, Vec<Issue>) {
    let manifest: Manifest = match serde_yaml_ng::from_str(text) {
        Ok(m) => m,
        Err(e) => return (None, vec![Issue::err("", e.to_string())]),
    };
    let issues = validate(&manifest);
    (Some(manifest), issues)
}

/// Loads `widget.yaml` from a widget folder and checks referenced files exist.
pub fn load_dir(dir: &Path) -> (Option<Manifest>, Vec<Issue>) {
    let path = dir.join(MANIFEST_FILE);
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) => return (None, vec![Issue::err("", format!("cannot read {MANIFEST_FILE}: {e}"))]),
    };
    let (m, mut issues) = parse(&text);
    if let Some(m) = &m {
        if let Some(entry) = &m.ui.entry {
            if is_safe_relative(entry) && !dir.join(entry).is_file() {
                issues.push(Issue::err("ui.entry", format!("file '{entry}' not found")));
            }
        }
        if let Some(icon) = &m.icon {
            if is_safe_relative(icon) && !dir.join(icon).is_file() {
                issues.push(Issue::warn("icon", format!("file '{icon}' not found")));
            }
        }
    }
    (m, issues)
}

/// A relative path that stays inside the widget folder.
pub fn is_safe_relative(p: &str) -> bool {
    if p.is_empty() || p.starts_with('/') || p.starts_with('\\') || p.contains(':') {
        return false;
    }
    p.split(['/', '\\']).all(|seg| !seg.is_empty() && seg != "..")
}

pub fn is_valid_id(id: &str) -> bool {
    let parts: Vec<&str> = id.split('.').collect();
    id.len() <= 100
        && parts.len() >= 2
        && parts.iter().all(|p| {
            !p.is_empty()
                && !p.starts_with('-')
                && !p.ends_with('-')
                && p.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        })
}

fn is_valid_version(v: &str) -> bool {
    let core = v.split(['-', '+']).next().unwrap_or("");
    let nums: Vec<&str> = core.split('.').collect();
    nums.len() == 3 && nums.iter().all(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
}

fn is_valid_hostname(h: &str) -> bool {
    !h.is_empty()
        && h.len() <= 253
        && h.split('.').count() >= 2
        && h.split('.').all(|l| {
            !l.is_empty()
                && l.len() <= 63
                && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
                && !l.starts_with('-')
        })
}

fn is_hex_color(c: &str) -> bool {
    let h = c.strip_prefix('#').unwrap_or("");
    matches!(h.len(), 3 | 4 | 6 | 8) && h.chars().all(|c| c.is_ascii_hexdigit())
}

pub fn validate(m: &Manifest) -> Vec<Issue> {
    let mut out = Vec::new();

    if m.schema != SCHEMA_VERSION {
        out.push(Issue::err("schema", format!("unsupported schema {} (expected {SCHEMA_VERSION})", m.schema)));
    }
    if !is_valid_id(&m.id) {
        out.push(Issue::err(
            "id",
            "must be lowercase words separated by dots, at least two parts (e.g. 'me.weather')",
        ));
    }
    if m.name.trim().is_empty() || m.name.len() > 60 {
        out.push(Issue::err("name", "must be 1–60 characters"));
    }
    if !is_valid_version(&m.version) {
        out.push(Issue::err("version", "must look like 1.0.0"));
    }
    if !CATEGORIES.contains(&m.category.as_str()) {
        out.push(Issue::warn("category", format!("unknown category '{}', shown under 'other'", m.category)));
    }
    if let Some(icon) = &m.icon {
        if !is_safe_relative(icon) {
            out.push(Issue::err("icon", "must be a relative path inside the widget folder"));
        }
    }
    for key in m.extra.keys() {
        out.push(Issue::warn(key.clone(), "unknown key (ignored)"));
    }

    // window
    let [w, h] = m.window.size;
    if !(80..=2000).contains(&w) || !(40..=2000).contains(&h) {
        out.push(Issue::err("window.size", "width 80–2000, height 40–2000"));
    }
    if let Some([mw, mh]) = m.window.min_size {
        if mw > w || mh > h {
            out.push(Issue::err("window.min_size", "must not be larger than window.size"));
        }
    }

    // style
    if !["glass", "solid", "none"].contains(&m.style.theme.as_str()) {
        out.push(Issue::err("style.theme", "must be glass, solid or none"));
    }
    if let Some(a) = &m.style.accent {
        if !is_hex_color(a) {
            out.push(Issue::err("style.accent", "must be a hex colour like #6cb8ff"));
        }
    }

    // permissions
    for (i, s) in m.permissions.system.iter().enumerate() {
        if !SYSTEM_CATEGORIES.contains(&s.as_str()) {
            out.push(Issue::err(
                format!("permissions.system[{i}]"),
                format!("unknown '{s}', expected one of {}", SYSTEM_CATEGORIES.join(", ")),
            ));
        }
    }
    for (i, host) in m.permissions.network.iter().enumerate() {
        if !is_valid_hostname(host) {
            out.push(Issue::err(
                format!("permissions.network[{i}]"),
                "must be an exact hostname like api.example.com (no scheme, path or wildcard)",
            ));
        }
    }

    // settings
    let mut keys = BTreeSet::new();
    for (i, s) in m.settings.iter().enumerate() {
        let p = format!("settings[{i}]");
        if s.key.is_empty() || !s.key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            out.push(Issue::err(format!("{p}.key"), "letters, digits and _ only"));
        }
        if !keys.insert(s.key.clone()) {
            out.push(Issue::err(format!("{p}.key"), format!("duplicate key '{}'", s.key)));
        }
        if !SETTING_TYPES.contains(&s.kind.as_str()) {
            out.push(Issue::err(format!("{p}.type"), format!("expected one of {}", SETTING_TYPES.join(", "))));
        }
        if s.kind == "choice" && s.options.is_empty() {
            out.push(Issue::err(format!("{p}.options"), "a choice needs options"));
        }
    }

    // data sources
    for (name, d) in &m.data {
        let p = format!("data.{name}");
        if matches!(name.as_str(), "settings" | "instance" | "item" | "storage") {
            out.push(Issue::err(&p, "this name is reserved"));
        }
        if !SOURCES.contains(&d.source.as_str()) {
            out.push(Issue::err(format!("{p}.source"), format!("unknown source '{}'", d.source)));
        }
        if let Some(iv) = d.interval {
            if iv < 250 {
                out.push(Issue::err(format!("{p}.interval"), "minimum is 250 ms"));
            }
        }
        if d.source == "system" && m.permissions.system.is_empty() {
            out.push(Issue::err(&p, "the system source needs permissions.system"));
        }
    }

    // ui
    match (&m.ui.blocks, &m.ui.entry) {
        (None, None) => out.push(Issue::err("ui", "needs either ui.blocks or ui.entry")),
        (Some(_), Some(_)) => out.push(Issue::err("ui", "use ui.blocks or ui.entry, not both")),
        (None, Some(e)) => {
            if !is_safe_relative(e) || !(e.ends_with(".html") || e.ends_with(".htm")) {
                out.push(Issue::err("ui.entry", "must be a relative .html file inside the widget folder"));
            }
        }
        (Some(blocks), None) => {
            if blocks.is_empty() {
                out.push(Issue::err("ui.blocks", "is empty"));
            }
            let names: BTreeSet<&str> = m.data.keys().map(String::as_str).collect();
            for (i, b) in blocks.iter().enumerate() {
                validate_block(b, &format!("ui.blocks[{i}]"), 0, m, &names, &mut out);
            }
        }
    }

    out
}

fn validate_block(b: &Value, path: &str, depth: usize, m: &Manifest, names: &BTreeSet<&str>, out: &mut Vec<Issue>) {
    if depth > 6 {
        out.push(Issue::err(path, "blocks are nested too deeply (max 6)"));
        return;
    }
    let Some(obj) = b.as_object() else {
        out.push(Issue::err(path, "a block must be a mapping with a 'type'"));
        return;
    };
    let Some(kind) = obj.get("type").and_then(Value::as_str) else {
        out.push(Issue::err(path, "missing 'type'"));
        return;
    };
    if !BLOCK_TYPES.contains(&kind) {
        out.push(Issue::err(
            format!("{path}.type"),
            format!("unknown block '{kind}', expected one of {}", BLOCK_TYPES.join(", ")),
        ));
        return;
    }
    if STORAGE_BLOCKS.contains(&kind) && !m.permissions.storage {
        out.push(Issue::err(path, format!("'{kind}' needs permissions.storage: true")));
    }
    if kind == "button" {
        if let Some(url) = obj.get("open").and_then(Value::as_str) {
            if !url.contains("{{") && !(url.starts_with("https://") || url.starts_with("http://")) {
                out.push(Issue::err(format!("{path}.open"), "must be an http(s) URL"));
            }
        }
    }
    if CONTAINER_BLOCKS.contains(&kind) {
        match obj.get("children").and_then(Value::as_array) {
            Some(children) => {
                for (i, c) in children.iter().enumerate() {
                    validate_block(c, &format!("{path}.children[{i}]"), depth + 1, m, names, out);
                }
            }
            None => out.push(Issue::err(path, format!("'{kind}' needs a children list"))),
        }
    }
    // Template references to undeclared data sources are a common mistake: warn.
    for (k, v) in obj {
        if let Some(s) = v.as_str() {
            for root in template_roots(s) {
                let known = names.contains(root.as_str())
                    || matches!(root.as_str(), "settings" | "instance" | "item" | "storage");
                if !known {
                    out.push(Issue::warn(
                        format!("{path}.{k}"),
                        format!("'{root}' is not a declared data source"),
                    ));
                }
            }
        }
    }
}

/// First path segment of every `{{ ... }}` expression in a string.
pub fn template_roots(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = s;
    while let Some(start) = rest.find("{{") {
        let after = &rest[start + 2..];
        let Some(end) = after.find("}}") else { break };
        let expr = after[..end].trim();
        let head = expr.split('|').next().unwrap_or("").trim();
        let root: String = head.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
        let literal = matches!(head, "true" | "false" | "null") || head.starts_with(['"', '\'']);
        if !root.is_empty() && !literal && !head.starts_with(|c: char| c.is_ascii_digit()) {
            out.push(root);
        }
        rest = &after[end + 2..];
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = r#"
schema: 1
id: forge.test
name: Test
version: 1.0.0
window: { size: [200, 100], layer: top }
permissions: { system: [cpu], storage: true }
settings:
  - { key: units, type: choice, options: [a, b], default: a }
data:
  sys: { source: system, interval: 1000 }
ui:
  blocks:
    - type: column
      children:
        - { type: gauge, value: "{{ sys.cpu.usage }}", max: 100 }
        - { type: notes, key: n }
"#;

    #[test]
    fn parses_good_manifest() {
        let (m, issues) = parse(GOOD);
        let m = m.unwrap();
        assert!(!has_errors(&issues), "{issues:?}");
        assert_eq!(m.window.layer, Layer::Top);
        assert_eq!(m.window.mode, Mode::Tile);
        assert_eq!(m.default_settings()["units"], "a");
    }

    #[test]
    fn rejects_bad_fields() {
        let bad = GOOD
            .replace("forge.test", "Bad_ID")
            .replace("1.0.0", "1.0")
            .replace("[cpu]", "[cpu, fridge]")
            .replace("storage: true", "storage: false");
        let (_, issues) = parse(&bad);
        let paths: Vec<_> = issues.iter().filter(|i| i.level == Level::Error).map(|i| i.path.as_str()).collect();
        assert!(paths.contains(&"id"));
        assert!(paths.contains(&"version"));
        assert!(paths.contains(&"permissions.system[1]"));
        assert!(paths.iter().any(|p| p.ends_with("children[1]")), "{paths:?}");
    }

    #[test]
    fn unknown_block_and_undeclared_source() {
        let txt = GOOD.replace("type: gauge", "type: dial").replace("{{ sys.cpu", "{{ cpu.cpu");
        let (_, issues) = parse(&txt);
        assert!(issues.iter().any(|i| i.message.contains("unknown block 'dial'")));
        let txt = GOOD.replace("{{ sys.cpu", "{{ cpu.cpu");
        let (_, issues) = parse(&txt);
        assert!(issues.iter().any(|i| i.level == Level::Warning && i.message.contains("'cpu'")));
    }

    #[test]
    fn system_source_requires_permission() {
        let txt = GOOD.replace("system: [cpu], ", "");
        let (_, issues) = parse(&txt);
        assert!(issues.iter().any(|i| i.path == "data.sys" && i.level == Level::Error));
    }

    #[test]
    fn ids_paths_hosts() {
        assert!(is_valid_id("forge.clock"));
        assert!(is_valid_id("com.alice.my-weather2"));
        assert!(!is_valid_id("clock"));
        assert!(!is_valid_id("Forge.Clock"));
        assert!(!is_valid_id("a..b"));
        assert!(is_safe_relative("assets/icon.svg"));
        assert!(!is_safe_relative("../x"));
        assert!(!is_safe_relative("/etc/passwd"));
        assert!(!is_safe_relative("C:\\x"));
        assert!(is_valid_hostname("api.met.no"));
        assert!(!is_valid_hostname("https://api.met.no"));
        assert!(!is_valid_hostname("*.example.com"));
    }

    #[test]
    fn template_roots_extracts_heads() {
        assert_eq!(template_roots("{{ sys.cpu.usage | fixed(0) }}% and {{settings.x}}"), vec!["sys", "settings"]);
        assert!(template_roots("no templates").is_empty());
        assert!(template_roots("{{ true }} {{ 'x' | upper }} {{ 3 }}").is_empty());
    }

    #[test]
    fn yaml_syntax_error_is_reported() {
        let (m, issues) = parse("schema: [1");
        assert!(m.is_none());
        assert!(has_errors(&issues));
    }
}
