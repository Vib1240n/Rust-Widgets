use serde::{Deserialize, Serialize};
use sysinfo::{Components, System};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThermalInfo {
    /// Component label (e.g., "CPU", "GPU", "nvme")
    pub label: String,
    /// Current temperature in Celsius
    pub temp: f32,
    /// Critical temperature threshold (if available)
    pub critical: Option<f32>,
}

impl ThermalInfo {
    pub fn from_system(_sys: &System) -> Vec<Self> {
        let components = Components::new_with_refreshed_list();
        
        components
            .iter()
            .map(|c| ThermalInfo {
                label: c.label().to_string(),
                temp: c.temperature(),
                critical: c.critical(),
            })
            .collect()
    }

    /// Get CPU temperature (first component with "cpu" or "core" in label)
    pub fn cpu(_sys: &System) -> Option<Self> {
        Self::from_system(_sys)
            .into_iter()
            .find(|t| {
                let label = t.label.to_lowercase();
                label.contains("cpu") || label.contains("core") || label.contains("package")
            })
    }

    /// Get GPU temperature
    pub fn gpu(_sys: &System) -> Option<Self> {
        Self::from_system(_sys)
            .into_iter()
            .find(|t| {
                let label = t.label.to_lowercase();
                label.contains("gpu") || label.contains("nvidia") || label.contains("amd")
            })
    }

    /// Format temperature
    pub fn temp_str(&self) -> String {
        format!("{:.0}°C", self.temp)
    }

    /// Check if temperature is critical
    pub fn is_critical(&self) -> bool {
        if let Some(critical) = self.critical {
            self.temp >= critical
        } else {
            self.temp >= 90.0 // Default critical threshold
        }
    }

    /// Check if temperature is warning level
    pub fn is_warning(&self) -> bool {
        if let Some(critical) = self.critical {
            self.temp >= critical * 0.85
        } else {
            self.temp >= 75.0
        }
    }
}
