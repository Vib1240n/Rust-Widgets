use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::{debug, warn};

/// Global configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub general: GeneralConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    #[serde(default = "default_theme")]
    pub theme: String,

    #[serde(default = "default_animation_duration")]
    pub animation_duration: u64,
}

fn default_theme() -> String {
    "default".to_string()
}

fn default_animation_duration() -> u64 {
    200
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            animation_duration: default_animation_duration(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            general: GeneralConfig::default(),
        }
    }
}

impl Config {
    /// Load config from default location
    pub fn load() -> Self {
        let path = Self::config_path();
        Self::load_from(&path)
    }

    /// Load config from specific path
    pub fn load_from(path: &PathBuf) -> Self {
        if path.exists() {
            match std::fs::read_to_string(path) {
                Ok(content) => match toml::from_str(&content) {
                    Ok(config) => {
                        debug!("Loaded config from {:?}", path);
                        return config;
                    }
                    Err(e) => {
                        warn!("Failed to parse config: {}", e);
                    }
                },
                Err(e) => {
                    warn!("Failed to read config file: {}", e);
                }
            }
        }
        debug!("Using default config");
        Config::default()
    }

    /// Get config directory path
    pub fn config_dir() -> PathBuf {
        if let Some(proj_dirs) = ProjectDirs::from("com", "vib1240n", "rust-widgets") {
            proj_dirs.config_dir().to_path_buf()
        } else {
            PathBuf::from("~/.config/rust-widgets")
        }
    }

    /// Get config file path
    pub fn config_path() -> PathBuf {
        Self::config_dir().join("config.toml")
    }

    /// Get theme CSS path
    pub fn theme_path(&self) -> PathBuf {
        Self::config_dir().join(format!("themes/{}.css", self.general.theme))
    }

    /// Get user CSS override path
    pub fn user_css_path() -> PathBuf {
        Self::config_dir().join("style.css")
    }
}
