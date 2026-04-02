use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub position: PositionConfig,
    #[serde(default)]
    pub appearance: AppearanceConfig,
    #[serde(default)]
    pub behavior: BehaviorConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PositionConfig {
    #[serde(default = "default_anchor")]
    pub anchor: String,
    #[serde(default = "default_margin_top")]
    pub margin_top: i32,
    #[serde(default)]
    pub margin_right: i32,
    #[serde(default)]
    pub margin_bottom: i32,
    #[serde(default)]
    pub margin_left: i32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AppearanceConfig {
    #[serde(default = "default_width")]
    pub width: i32,
    #[serde(default = "default_height")]
    pub height: i32,
    #[serde(default = "default_icon_size")]
    pub icon_size: i32,
    #[serde(default = "default_true")]
    pub show_percentage: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BehaviorConfig {
    #[serde(default = "default_timeout")]
    pub timeout: u64,
}

// Defaults
fn default_anchor() -> String { "top-center".to_string() }
fn default_margin_top() -> i32 { 50 }
fn default_width() -> i32 { 200 }
fn default_height() -> i32 { 48 }
fn default_icon_size() -> i32 { 24 }
fn default_timeout() -> u64 { 1500 }
fn default_true() -> bool { true }

impl Default for Config {
    fn default() -> Self {
        Self {
            position: PositionConfig::default(),
            appearance: AppearanceConfig::default(),
            behavior: BehaviorConfig::default(),
        }
    }
}

impl Default for PositionConfig {
    fn default() -> Self {
        Self {
            anchor: default_anchor(),
            margin_top: default_margin_top(),
            margin_right: 0,
            margin_bottom: 0,
            margin_left: 0,
        }
    }
}

impl Default for AppearanceConfig {
    fn default() -> Self {
        Self {
            width: default_width(),
            height: default_height(),
            icon_size: default_icon_size(),
            show_percentage: true,
        }
    }
}

impl Default for BehaviorConfig {
    fn default() -> Self {
        Self {
            timeout: default_timeout(),
        }
    }
}

impl Config {
    pub fn load() -> Self {
        let config_path = Self::config_path();
        
        if config_path.exists() {
            match std::fs::read_to_string(&config_path) {
                Ok(content) => match toml::from_str(&content) {
                    Ok(config) => {
                        tracing::info!("Loaded config from {:?}", config_path);
                        return config;
                    }
                    Err(e) => {
                        tracing::warn!("Failed to parse config: {}", e);
                    }
                },
                Err(e) => {
                    tracing::warn!("Failed to read config: {}", e);
                }
            }
        }
        
        tracing::info!("Using default config");
        Config::default()
    }
    
    pub fn config_dir() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("~/.config"))
            .join("rw/volume-osd")
    }
    
    pub fn config_path() -> PathBuf {
        Self::config_dir().join("config.toml")
    }
}
