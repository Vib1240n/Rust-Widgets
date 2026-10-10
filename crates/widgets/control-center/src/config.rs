use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct Config {
    pub position: PositionConfig,
    pub appearance: AppearanceConfig,
    pub behavior: BehaviorConfig,
    pub sections: SectionsConfig,
    pub tiles: TilesConfig,
    pub commands: CommandsConfig,
    pub animation: AnimationConfig,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct PositionConfig {
    pub anchor: String,
    pub margin_top: i32,
    pub margin_right: i32,
    pub margin_bottom: i32,
    pub margin_left: i32,
}

impl Default for PositionConfig {
    fn default() -> Self {
        Self { anchor: "top-right".into(), margin_top: 10, margin_right: 10, margin_bottom: 0, margin_left: 0 }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct AppearanceConfig {
    pub width: i32,
}

impl Default for AppearanceConfig {
    fn default() -> Self {
        Self { width: 390 }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct BehaviorConfig {
    /// Volume + media refresh (ms) while open; connectivity/tiles every 3rd tick
    pub poll_interval: u64,
    pub close_on_escape: bool,
    pub close_on_unfocus: bool,
}

impl Default for BehaviorConfig {
    fn default() -> Self {
        Self { poll_interval: 1000, close_on_escape: true, close_on_unfocus: false }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct SectionsConfig {
    pub connectivity: bool,
    pub tiles: bool,
    pub levels: bool,
    pub media: bool,
    pub footer: bool,
}

impl Default for SectionsConfig {
    fn default() -> Self {
        Self { connectivity: true, tiles: true, levels: true, media: true, footer: true }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct TilesConfig {
    /// Quick tiles, two per row: airplane, caffeinate, vpn
    pub items: Vec<String>,
}

impl Default for TilesConfig {
    fn default() -> Self {
        Self { items: vec!["airplane".into(), "caffeinate".into()] }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct CommandsConfig {
    pub power: String,
    pub network: String,
    pub caffeinate: String,
    pub vpn_interface: String,
}

impl Default for CommandsConfig {
    fn default() -> Self {
        Self {
            power: "rw toggle power".into(),
            network: "rw toggle network".into(),
            caffeinate: "~/Development/bash_scripts/toggle-caffeinate.sh".into(),
            vpn_interface: "proton-us".into(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct AnimationConfig {
    pub enabled: bool,
    pub direction: String,
    pub duration: u64,
}

impl Default for AnimationConfig {
    fn default() -> Self {
        Self { enabled: true, direction: "down".into(), duration: 250 }
    }
}

impl Config {
    pub fn load() -> Self {
        let path = Self::config_path();
        match std::fs::read_to_string(&path) {
            Ok(s) => match toml::from_str(&s) {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!("config {}: {e}, using defaults", path.display());
                    Self::default()
                }
            },
            Err(_) => Self::default(),
        }
    }

    pub fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("~/.config"))
            .join("rw/control-center/config.toml")
    }
}
