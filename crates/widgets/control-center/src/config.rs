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
    #[serde(default)]
    pub sections: SectionsConfig,
    #[serde(default)]
    pub toggles: TogglesConfig,
    #[serde(default)]
    pub sliders: SlidersConfig,
    #[serde(default)]
    pub animation: AnimationConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PositionConfig {
    #[serde(default = "default_anchor")]
    pub anchor: String,
    #[serde(default = "default_margin_top")]
    pub margin_top: i32,
    #[serde(default = "default_margin_right")]
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
}

#[derive(Debug, Clone, Deserialize)]
pub struct BehaviorConfig {
    #[serde(default = "default_poll_interval")]
    pub poll_interval: u64,
    #[serde(default = "default_true")]
    pub close_on_escape: bool,
    #[serde(default = "default_true")]
    pub close_on_unfocus: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SectionsConfig {
    #[serde(default = "default_true")]
    pub toggles: bool,
    #[serde(default = "default_true")]
    pub sliders: bool,
    #[serde(default = "default_true")]
    pub media: bool,
    #[serde(default = "default_true")]
    pub stats: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TogglesConfig {
    #[serde(default = "default_true")]
    pub wifi: bool,
    #[serde(default = "default_true")]
    pub bluetooth: bool,
    #[serde(default = "default_true")]
    pub dnd: bool,
    #[serde(default = "default_true")]
    pub caffeinate: bool,
    #[serde(default = "default_true")]
    pub night_light: bool,
    #[serde(default)]
    pub vpn: bool,
    #[serde(default)]
    pub airplane: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SlidersConfig {
    #[serde(default = "default_true")]
    pub volume: bool,
    #[serde(default = "default_true")]
    pub brightness: bool,
    #[serde(default = "default_true")]
    pub volume_output_selector: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AnimationConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_animation_type")]
    pub r#type: String,
    #[serde(default = "default_direction")]
    pub direction: String,
    #[serde(default = "default_duration")]
    pub duration: u64,
}

// Defaults
fn default_anchor() -> String {
    "top-right".to_string()
}
fn default_margin_top() -> i32 {
    50
}
fn default_margin_right() -> i32 {
    10
}
fn default_width() -> i32 {
    360
}
fn default_poll_interval() -> u64 {
    1000
}
fn default_true() -> bool {
    true
}
fn default_animation_type() -> String {
    "slide".to_string()
}
fn default_direction() -> String {
    "down".to_string()
}
fn default_duration() -> u64 {
    250
}

impl Default for Config {
    fn default() -> Self {
        Self {
            position: PositionConfig::default(),
            appearance: AppearanceConfig::default(),
            behavior: BehaviorConfig::default(),
            sections: SectionsConfig::default(),
            toggles: TogglesConfig::default(),
            sliders: SlidersConfig::default(),
            animation: AnimationConfig::default(),
        }
    }
}

impl Default for PositionConfig {
    fn default() -> Self {
        Self {
            anchor: default_anchor(),
            margin_top: default_margin_top(),
            margin_right: default_margin_right(),
            margin_bottom: 0,
            margin_left: 0,
        }
    }
}

impl Default for AppearanceConfig {
    fn default() -> Self {
        Self {
            width: default_width(),
        }
    }
}

impl Default for BehaviorConfig {
    fn default() -> Self {
        Self {
            poll_interval: default_poll_interval(),
            close_on_escape: true,
            close_on_unfocus: true,
        }
    }
}

impl Default for SectionsConfig {
    fn default() -> Self {
        Self {
            toggles: true,
            sliders: true,
            media: true,
            stats: true,
        }
    }
}

impl Default for TogglesConfig {
    fn default() -> Self {
        Self {
            wifi: true,
            bluetooth: true,
            dnd: true,
            caffeinate: true,
            night_light: true,
            vpn: false,
            airplane: false,
        }
    }
}

impl Default for SlidersConfig {
    fn default() -> Self {
        Self {
            volume: true,
            brightness: true,
            volume_output_selector: true,
        }
    }
}

impl Default for AnimationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            r#type: default_animation_type(),
            direction: default_direction(),
            duration: default_duration(),
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
            .join("rw/control-center")
    }

    pub fn config_path() -> PathBuf {
        Self::config_dir().join("config.toml")
    }

    pub fn style_path() -> PathBuf {
        Self::config_dir().join("style.css")
    }
}
