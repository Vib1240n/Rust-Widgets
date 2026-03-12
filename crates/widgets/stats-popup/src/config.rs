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
    pub temperatures: TemperaturesConfig,
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
    pub cpu: bool,
    #[serde(default = "default_true")]
    pub memory: bool,
    #[serde(default = "default_true")]
    pub disk: bool,
    #[serde(default = "default_true")]
    pub battery: bool,
    #[serde(default)]
    pub network: bool,
    #[serde(default = "default_true")]
    pub temperatures: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TemperaturesConfig {
    #[serde(default = "default_temp_labels")]
    pub show_labels: Vec<String>,
    #[serde(default = "default_max_display")]
    pub max_display: usize,
    #[serde(default = "default_warning_threshold")]
    pub warning_threshold: f32,
    #[serde(default = "default_critical_threshold")]
    pub critical_threshold: f32,
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
    280
}
fn default_poll_interval() -> u64 {
    2000
}
fn default_true() -> bool {
    true
}
fn default_temp_labels() -> Vec<String> {
    vec![
        "package".to_string(),
        "gpu".to_string(),
        "core 0".to_string(),
    ]
}
fn default_max_display() -> usize {
    4
}
fn default_warning_threshold() -> f32 {
    75.0
}
fn default_critical_threshold() -> f32 {
    90.0
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
            temperatures: TemperaturesConfig::default(),
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
            cpu: true,
            memory: true,
            disk: true,
            battery: true,
            network: false,
            temperatures: true,
        }
    }
}

impl Default for TemperaturesConfig {
    fn default() -> Self {
        Self {
            show_labels: default_temp_labels(),
            max_display: default_max_display(),
            warning_threshold: default_warning_threshold(),
            critical_threshold: default_critical_threshold(),
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
            .join("rw/stats-popup")
    }

    pub fn config_path() -> PathBuf {
        Self::config_dir().join("config.toml")
    }

    pub fn style_path() -> PathBuf {
        Self::config_dir().join("style.css")
    }
}
