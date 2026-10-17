use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

static CONFIG: OnceLock<Config> = OnceLock::new();

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub general: GeneralConfig,
    #[serde(default)]
    pub window: WindowConfig,
    #[serde(default)]
    pub paste: PasteConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            general: GeneralConfig::default(),
            window: WindowConfig::default(),
            paste: PasteConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    #[serde(default = "default_history_limit")]
    pub history_limit: usize,
    #[serde(default = "default_enable_tray")]
    pub enable_tray: bool,
    #[serde(default = "default_category")]
    pub default_category: String,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            history_limit: default_history_limit(),
            enable_tray: default_enable_tray(),
            default_category: default_category(),
        }
    }
}

fn default_history_limit() -> usize {
    200
}
fn default_enable_tray() -> bool {
    true
}
fn default_category() -> String {
    "all".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowConfig {
    #[serde(default = "default_width")]
    pub width: i32,
    #[serde(default = "default_height")]
    pub height: i32,
    #[serde(default = "default_margin_top")]
    pub margin_top: i32,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            width: default_width(),
            height: default_height(),
            margin_top: default_margin_top(),
        }
    }
}

fn default_width() -> i32 {
    540
}
fn default_height() -> i32 {
    580
}
fn default_margin_top() -> i32 {
    38
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PasteConfig {
    #[serde(default = "default_paste_delay_ms")]
    pub paste_delay_ms: u64,
    #[serde(default = "default_key_hold_delay_ms")]
    pub key_hold_delay_ms: u64,
    #[serde(default = "default_smart_terminal")]
    pub smart_terminal_paste: bool,
}

impl Default for PasteConfig {
    fn default() -> Self {
        Self {
            paste_delay_ms: default_paste_delay_ms(),
            key_hold_delay_ms: default_key_hold_delay_ms(),
            smart_terminal_paste: default_smart_terminal(),
        }
    }
}

fn default_paste_delay_ms() -> u64 {
    120
}
fn default_key_hold_delay_ms() -> u64 {
    15
}
fn default_smart_terminal() -> bool {
    true
}

impl Config {
    pub fn global() -> &'static Config {
        CONFIG.get_or_init(Self::load)
    }

    pub fn config_path() -> PathBuf {
        glib::user_config_dir().join("cosmic-clipboard").join("config.toml")
    }

    pub fn load() -> Self {
        let path = Self::config_path();

        if let Ok(content) = fs::read_to_string(&path) {
            match toml::from_str::<Config>(&content) {
                Ok(cfg) => return cfg,
                Err(err) => {
                    eprintln!("Warning: Failed to parse {}: {err}. Using defaults.", path.display());
                }
            }
        }

        let default_cfg = Config::default();

        // Write default config with explanatory comments if it doesn't exist
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        let toml_commented = r#"# Cosmic Clipboard Configuration
# Location: ~/.config/cosmic-clipboard/config.toml

[general]
# Maximum number of history entries to keep in view/memory (unpinned items are pruned)
# 0 = unlimited
history_limit = 200

# Enable system tray / top panel indicator (StatusNotifierItem)
enable_tray = true

# Default category filter on launch: "all", "favorites", "text", "code", "links", "images"
default_category = "all"

[window]
# Width and height of the clipboard popup
width = 540
height = 580

# Margin from the top panel on layer-shell compositors (COSMIC / Niri)
margin_top = 38

[paste]
# Delay in milliseconds before injecting paste keys after window hide
paste_delay_ms = 120

# Key hold duration in milliseconds for ydotool / wtype
key_hold_delay_ms = 15

# Automatically switch to Ctrl+Shift+V for terminal emulators
smart_terminal_paste = true
"#;
        let _ = fs::write(&path, toml_commented);

        default_cfg
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let cfg = Config::default();
        assert_eq!(cfg.general.history_limit, 200);
        assert!(cfg.general.enable_tray);
        assert_eq!(cfg.general.default_category, "all");
        assert_eq!(cfg.window.width, 540);
        assert_eq!(cfg.window.height, 580);
        assert_eq!(cfg.window.margin_top, 38);
        assert_eq!(cfg.paste.paste_delay_ms, 120);
        assert_eq!(cfg.paste.key_hold_delay_ms, 15);
        assert!(cfg.paste.smart_terminal_paste);
    }

    #[test]
    fn test_deserialize_partial_toml() {
        let toml_str = r#"
        [general]
        history_limit = 50
        default_category = "code"

        [window]
        width = 600
        "#;
        let cfg: Config = toml::from_str(toml_str).expect("parse failed");
        assert_eq!(cfg.general.history_limit, 50);
        assert_eq!(cfg.general.default_category, "code");
        assert!(cfg.general.enable_tray); // default
        assert_eq!(cfg.window.width, 600);
        assert_eq!(cfg.window.height, 580); // default
        assert_eq!(cfg.paste.paste_delay_ms, 120); // default
    }
}
