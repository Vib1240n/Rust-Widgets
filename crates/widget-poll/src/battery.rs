use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatteryInfo {
    /// Battery percentage (0-100)
    pub percentage: u8,
    /// Charging status
    pub status: BatteryStatus,
    /// Time to empty/full in minutes (if available)
    pub time_remaining: Option<u64>,
    /// Current power draw in watts (if available)
    pub power_draw: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BatteryStatus {
    Charging,
    Discharging,
    Full,
    NotCharging,
    Unknown,
}

impl BatteryInfo {
    /// Read battery info from /sys/class/power_supply
    pub fn read() -> Option<Self> {
        let bat_path = Path::new("/sys/class/power_supply/BAT0");
        
        if !bat_path.exists() {
            // Try BAT1
            let bat1_path = Path::new("/sys/class/power_supply/BAT1");
            if !bat1_path.exists() {
                return None;
            }
            return Self::read_from(bat1_path);
        }
        
        Self::read_from(bat_path)
    }

    fn read_from(path: &Path) -> Option<Self> {
        let capacity = fs::read_to_string(path.join("capacity"))
            .ok()?
            .trim()
            .parse::<u8>()
            .ok()?;

        let status_str = fs::read_to_string(path.join("status"))
            .ok()?
            .trim()
            .to_lowercase();

        let status = match status_str.as_str() {
            "charging" => BatteryStatus::Charging,
            "discharging" => BatteryStatus::Discharging,
            "full" => BatteryStatus::Full,
            "not charging" => BatteryStatus::NotCharging,
            _ => BatteryStatus::Unknown,
        };

        // Try to read power draw
        let power_draw = fs::read_to_string(path.join("power_now"))
            .ok()
            .and_then(|s| s.trim().parse::<u64>().ok())
            .map(|uw| uw as f32 / 1_000_000.0); // Convert microwatts to watts

        // Try to calculate time remaining
        let time_remaining = Self::calculate_time_remaining(path, &status);

        Some(BatteryInfo {
            percentage: capacity,
            status,
            time_remaining,
            power_draw,
        })
    }

    fn calculate_time_remaining(path: &Path, status: &BatteryStatus) -> Option<u64> {
        let energy_now = fs::read_to_string(path.join("energy_now"))
            .ok()?
            .trim()
            .parse::<u64>()
            .ok()?;

        let power_now = fs::read_to_string(path.join("power_now"))
            .ok()?
            .trim()
            .parse::<u64>()
            .ok()?;

        if power_now == 0 {
            return None;
        }

        match status {
            BatteryStatus::Discharging => {
                // Time to empty
                Some((energy_now * 60) / power_now)
            }
            BatteryStatus::Charging => {
                // Time to full
                let energy_full = fs::read_to_string(path.join("energy_full"))
                    .ok()?
                    .trim()
                    .parse::<u64>()
                    .ok()?;
                let remaining = energy_full.saturating_sub(energy_now);
                Some((remaining * 60) / power_now)
            }
            _ => None,
        }
    }

    /// Format percentage
    pub fn percentage_str(&self) -> String {
        format!("{}%", self.percentage)
    }

    /// Format time remaining (e.g., "2h 30m")
    pub fn time_str(&self) -> Option<String> {
        self.time_remaining.map(|mins| {
            let hours = mins / 60;
            let minutes = mins % 60;
            if hours > 0 {
                format!("{}h {}m", hours, minutes)
            } else {
                format!("{}m", minutes)
            }
        })
    }

    /// Format power draw (e.g., "12.5W")
    pub fn power_str(&self) -> Option<String> {
        self.power_draw.map(|w| format!("{:.1}W", w))
    }

    /// Get icon name based on status and percentage
    pub fn icon(&self) -> &'static str {
        match self.status {
            BatteryStatus::Charging => match self.percentage {
                0..=20 => "battery-level-20-charging-symbolic",
                21..=40 => "battery-level-40-charging-symbolic",
                41..=60 => "battery-level-60-charging-symbolic",
                61..=80 => "battery-level-80-charging-symbolic",
                _ => "battery-level-100-charging-symbolic",
            },
            _ => match self.percentage {
                0..=10 => "battery-level-10-symbolic",
                11..=20 => "battery-level-20-symbolic",
                21..=40 => "battery-level-40-symbolic",
                41..=60 => "battery-level-60-symbolic",
                61..=80 => "battery-level-80-symbolic",
                _ => "battery-level-100-symbolic",
            },
        }
    }
}
