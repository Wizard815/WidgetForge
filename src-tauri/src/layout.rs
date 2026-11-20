//! Placed widget instances and their positions (config/layout.json).

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::manifest::{Edge, Layer, Manifest, Mode};

fn yes() -> bool {
    true
}
fn one() -> f64 {
    1.0
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Instance {
    pub id: String,
    pub widget_id: String,
    /// Logical (DPI-independent) position of the window's top-left corner.
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    /// Name of the monitor the instance was last on.
    #[serde(default)]
    pub monitor: Option<String>,
    pub mode: Mode,
    pub layer: Layer,
    #[serde(default)]
    pub edge: Edge,
    #[serde(default = "yes")]
    pub start_with_app: bool,
    #[serde(default)]
    pub show_in_taskbar: bool,
    #[serde(default = "one")]
    pub opacity: f64,
    #[serde(default)]
    pub locked: bool,
    #[serde(default)]
    pub settings: Map<String, Value>,
}

impl Instance {
    pub fn new(id: String, m: &Manifest, x: f64, y: f64) -> Instance {
        Instance {
            id,
            widget_id: m.id.clone(),
            x,
            y,
            w: m.window.size[0] as f64,
            h: m.window.size[1] as f64,
            monitor: None,
            mode: m.window.mode,
            layer: m.window.layer,
            edge: m.window.edge,
            start_with_app: true,
            show_in_taskbar: false,
            opacity: 1.0,
            locked: false,
            settings: m.default_settings(),
        }
    }

    /// Setting values with manifest defaults filled in for keys the user never changed.
    pub fn effective_settings(&self, m: &Manifest) -> Map<String, Value> {
        let mut out = m.default_settings();
        for (k, v) in &self.settings {
            if out.contains_key(k) {
                out.insert(k.clone(), v.clone());
            }
        }
        out
    }
}

/// Fields the Hub may change on an instance. Missing fields are left as they are.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct InstancePatch {
    pub mode: Option<Mode>,
    pub layer: Option<Layer>,
    pub edge: Option<Edge>,
    pub start_with_app: Option<bool>,
    pub show_in_taskbar: Option<bool>,
    pub opacity: Option<f64>,
    pub locked: Option<bool>,
    pub settings: Option<Map<String, Value>>,
}

impl InstancePatch {
    pub fn apply(&self, i: &mut Instance) {
        if let Some(v) = self.mode {
            i.mode = v;
        }
        if let Some(v) = self.layer {
            i.layer = v;
        }
        if let Some(v) = self.edge {
            i.edge = v;
        }
        if let Some(v) = self.start_with_app {
            i.start_with_app = v;
        }
        if let Some(v) = self.show_in_taskbar {
            i.show_in_taskbar = v;
        }
        if let Some(v) = self.opacity {
            i.opacity = v.clamp(0.2, 1.0);
        }
        if let Some(v) = self.locked {
            i.locked = v;
        }
        if let Some(s) = &self.settings {
            for (k, v) in s {
                i.settings.insert(k.clone(), v.clone());
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Layout {
    pub version: u32,
    pub instances: Vec<Instance>,
}

impl Default for Layout {
    fn default() -> Self {
        Layout { version: 1, instances: Vec::new() }
    }
}

impl Layout {
    pub fn get(&self, id: &str) -> Option<&Instance> {
        self.instances.iter().find(|i| i.id == id)
    }
    pub fn get_mut(&mut self, id: &str) -> Option<&mut Instance> {
        self.instances.iter_mut().find(|i| i.id == id)
    }
    pub fn remove(&mut self, id: &str) -> Option<Instance> {
        let pos = self.instances.iter().position(|i| i.id == id)?;
        Some(self.instances.remove(pos))
    }
    pub fn count_of(&self, widget_id: &str) -> usize {
        self.instances.iter().filter(|i| i.widget_id == widget_id).count()
    }

    /// Where to put a new widget: cascade from the top-right of the work area so new
    /// widgets don't stack exactly on top of each other.
    pub fn next_position(&self, work_x: f64, work_y: f64, work_w: f64, w: f64) -> (f64, f64) {
        let n = self.instances.len() as f64;
        let margin = 24.0;
        let step = 28.0;
        let x = (work_x + work_w - w - margin - (n % 8.0) * step).max(work_x + margin);
        let y = work_y + margin + (n % 8.0) * step;
        (x, y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest;

    fn m() -> Manifest {
        let (m, _) = manifest::parse(
            "schema: 1\nid: forge.t\nname: T\nversion: 1.0.0\nwindow: { size: [300, 200], layer: top }\nsettings:\n  - { key: a, type: number, default: 5 }\n  - { key: b, type: bool, default: true }\nui:\n  blocks: [ { type: text, text: x } ]\n",
        );
        m.unwrap()
    }

    #[test]
    fn new_instance_uses_manifest_defaults() {
        let i = Instance::new("i1".into(), &m(), 10.0, 20.0);
        assert_eq!((i.w, i.h), (300.0, 200.0));
        assert_eq!(i.layer, Layer::Top);
        assert_eq!(i.settings["a"], 5);
        assert!(i.start_with_app);
    }

    #[test]
    fn patch_and_effective_settings() {
        let man = m();
        let mut i = Instance::new("i1".into(), &man, 0.0, 0.0);
        let patch: InstancePatch =
            serde_json::from_str(r#"{"layer":"normal","opacity":5,"settings":{"a":9,"zzz":1}}"#).unwrap();
        patch.apply(&mut i);
        assert_eq!(i.layer, Layer::Normal);
        assert_eq!(i.opacity, 1.0);
        let eff = i.effective_settings(&man);
        assert_eq!(eff["a"], 9);
        assert_eq!(eff["b"], true);
        assert!(!eff.contains_key("zzz"), "unknown keys are dropped");
    }

    #[test]
    fn layout_roundtrip_and_cascade() {
        let mut l = Layout::default();
        let (x0, y0) = l.next_position(0.0, 0.0, 1920.0, 300.0);
        l.instances.push(Instance::new("a".into(), &m(), x0, y0));
        let (x1, y1) = l.next_position(0.0, 0.0, 1920.0, 300.0);
        assert!(x1 < x0 && y1 > y0);
        let json = serde_json::to_string(&l).unwrap();
        let back: Layout = serde_json::from_str(&json).unwrap();
        assert_eq!(back, l);
        assert_eq!(back.count_of("forge.t"), 1);
    }
}
