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
    pub popup: PopupConfig,
    #[serde(default)]
    pub animation: AnimationConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PositionConfig {
    /// Anchor for notification center panel
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
    /// Width of notification center panel
    #[serde(default = "default_panel_width")]
    pub panel_width: i32,
    /// Width of popup notifications
    #[serde(default = "default_popup_width")]
    pub popup_width: i32,
    /// Max height of notification center
    #[serde(default = "default_max_height")]
    pub max_height: i32,
    /// Max notifications to keep in history
    #[serde(default = "default_max_history")]
    pub max_history: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BehaviorConfig {
    #[serde(default = "default_true")]
    pub close_on_escape: bool,
    #[serde(default = "default_true")]
    pub close_on_unfocus: bool,
    /// Whether DND mode is enabled by default
    #[serde(default)]
    pub dnd_enabled: bool,
    /// Enable notification sounds
    #[serde(default = "default_true")]
    pub play_sounds: bool,
    /// Command for critical notifications
    #[serde(default)]
    pub critical_sound: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PopupConfig {
    /// Anchor for popup notifications (typically top-right)
    #[serde(default = "default_popup_anchor")]
    pub anchor: String,
    /// Margin from screen edge
    #[serde(default = "default_popup_margin")]
    pub margin: i32,
    /// Gap between stacked popups
    #[serde(default = "default_popup_gap")]
    pub gap: i32,
    /// Default timeout for low urgency (ms)
    #[serde(default = "default_timeout_low")]
    pub timeout_low: u64,
    /// Default timeout for normal urgency (ms)
    #[serde(default = "default_timeout_normal")]
    pub timeout_normal: u64,
    /// Default timeout for critical urgency (ms), 0 = no auto-dismiss
    #[serde(default = "default_timeout_critical")]
    pub timeout_critical: u64,
    /// Max popups to show at once
    #[serde(default = "default_max_popups")]
    pub max_visible: usize,
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
fn default_popup_anchor() -> String {
    "top-right".to_string()
}
fn default_margin_top() -> i32 {
    50
}
fn default_margin_right() -> i32 {
    10
}
fn default_panel_width() -> i32 {
    380
}
fn default_popup_width() -> i32 {
    360
}
fn default_max_height() -> i32 {
    600
}
fn default_max_history() -> usize {
    50
}
fn default_popup_margin() -> i32 {
    10
}
fn default_popup_gap() -> i32 {
    8
}
fn default_timeout_low() -> u64 {
    3000
}
fn default_timeout_normal() -> u64 {
    5000
}
fn default_timeout_critical() -> u64 {
    0
}
fn default_max_popups() -> usize {
    5
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
    200
}

impl Default for Config {
    fn default() -> Self {
        Self {
            position: PositionConfig::default(),
            appearance: AppearanceConfig::default(),
            behavior: BehaviorConfig::default(),
            popup: PopupConfig::default(),
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
            panel_width: default_panel_width(),
            popup_width: default_popup_width(),
            max_height: default_max_height(),
            max_history: default_max_history(),
        }
    }
}

impl Default for BehaviorConfig {
    fn default() -> Self {
        Self {
            close_on_escape: true,
            close_on_unfocus: true,
            dnd_enabled: false,
            play_sounds: true,
            critical_sound: None,
        }
    }
}

impl Default for PopupConfig {
    fn default() -> Self {
        Self {
            anchor: default_popup_anchor(),
            margin: default_popup_margin(),
            gap: default_popup_gap(),
            timeout_low: default_timeout_low(),
            timeout_normal: default_timeout_normal(),
            timeout_critical: default_timeout_critical(),
            max_visible: default_max_popups(),
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
            .join("rw/notification-center")
    }

    pub fn config_path() -> PathBuf {
        Self::config_dir().join("config.toml")
    }

    pub fn style_path() -> PathBuf {
        Self::config_dir().join("style.css")
    }
}
