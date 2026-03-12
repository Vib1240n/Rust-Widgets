use serde::{Deserialize, Serialize};
use sysinfo::System;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryInfo {
    /// Total RAM in bytes
    pub total: u64,
    /// Used RAM in bytes
    pub used: u64,
    /// Available RAM in bytes
    pub available: u64,
    /// Usage percentage (0-100)
    pub usage: f32,
    /// Total swap in bytes
    pub swap_total: u64,
    /// Used swap in bytes
    pub swap_used: u64,
}

impl MemoryInfo {
    pub fn from_system(sys: &System) -> Self {
        let total = sys.total_memory();
        let used = sys.used_memory();
        let available = sys.available_memory();
        let swap_total = sys.total_swap();
        let swap_used = sys.used_swap();

        let usage = if total > 0 {
            (used as f32 / total as f32) * 100.0
        } else {
            0.0
        };

        Self {
            total,
            used,
            available,
            usage,
            swap_total,
            swap_used,
        }
    }

    /// Format used memory as string (e.g., "12.4 GB")
    pub fn used_str(&self) -> String {
        Self::format_bytes(self.used)
    }

    /// Format total memory as string
    pub fn total_str(&self) -> String {
        Self::format_bytes(self.total)
    }

    /// Format available memory as string
    pub fn available_str(&self) -> String {
        Self::format_bytes(self.available)
    }

    /// Format usage as string (e.g., "78%")
    pub fn usage_str(&self) -> String {
        format!("{:.0}%", self.usage)
    }

    /// Format as "used / total" (e.g., "12.4 / 16.0 GB")
    pub fn summary_str(&self) -> String {
        format!(
            "{:.1} / {:.1} GB",
            self.used as f64 / 1_073_741_824.0,
            self.total as f64 / 1_073_741_824.0
        )
    }

    fn format_bytes(bytes: u64) -> String {
        const GB: u64 = 1_073_741_824;
        const MB: u64 = 1_048_576;

        if bytes >= GB {
            format!("{:.1} GB", bytes as f64 / GB as f64)
        } else {
            format!("{:.0} MB", bytes as f64 / MB as f64)
        }
    }
}
