//! App-wide settings (config/settings.json).

use serde::{Deserialize, Serialize};

fn yes() -> bool {
    true
}
fn system() -> String {
    "system".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppSettings {
    /// Start WidgetForge when the user logs in (mirrors the OS login item).
    #[serde(default)]
    pub launch_on_startup: bool,
    /// Show the Hub when started normally (startup launches never show it).
    #[serde(default = "yes")]
    pub show_hub_on_launch: bool,
    /// "system", "dark" or "light".
    #[serde(default = "system")]
    pub theme: String,
    /// Temporarily hide every widget (tray toggle). Not persisted across restarts.
    #[serde(skip)]
    pub widgets_hidden: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        AppSettings { launch_on_startup: false, show_hub_on_launch: true, theme: system(), widgets_hidden: false }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SettingsPatch {
    pub launch_on_startup: Option<bool>,
    pub show_hub_on_launch: Option<bool>,
    pub theme: Option<String>,
}

impl SettingsPatch {
    pub fn apply(&self, s: &mut AppSettings) {
        if let Some(v) = self.launch_on_startup {
            s.launch_on_startup = v;
        }
        if let Some(v) = self.show_hub_on_launch {
            s.show_hub_on_launch = v;
        }
        if let Some(v) = &self.theme {
            if ["system", "dark", "light"].contains(&v.as_str()) {
                s.theme = v.clone();
            }
        }
    }
}
