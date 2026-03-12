use serde::{Deserialize, Serialize};
use sysinfo::System;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuInfo {
    /// Overall CPU usage percentage (0-100)
    pub usage: f32,
    /// Per-core usage percentages
    pub cores: Vec<f32>,
    /// Number of physical cores
    pub core_count: usize,
    /// CPU frequency in MHz (average)
    pub frequency: u64,
    /// CPU brand/model name
    pub name: String,
}

impl CpuInfo {
    pub fn from_system(sys: &System) -> Self {
        let cpus = sys.cpus();
        let core_count = cpus.len();

        let usage = if core_count > 0 {
            cpus.iter().map(|c| c.cpu_usage()).sum::<f32>() / core_count as f32
        } else {
            0.0
        };

        let cores: Vec<f32> = cpus.iter().map(|c| c.cpu_usage()).collect();

        let frequency = if core_count > 0 {
            cpus.iter().map(|c| c.frequency()).sum::<u64>() / core_count as u64
        } else {
            0
        };

        let name = cpus
            .first()
            .map(|c| c.brand().to_string())
            .unwrap_or_default();

        Self {
            usage,
            cores,
            core_count,
            frequency,
            name,
        }
    }

    /// Format usage as string (e.g., "45%")
    pub fn usage_str(&self) -> String {
        format!("{:.0}%", self.usage)
    }

    /// Format frequency as string (e.g., "3.2 GHz")
    pub fn frequency_str(&self) -> String {
        if self.frequency >= 1000 {
            format!("{:.1} GHz", self.frequency as f64 / 1000.0)
        } else {
            format!("{} MHz", self.frequency)
        }
    }
}
