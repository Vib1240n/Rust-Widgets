use serde::{Deserialize, Serialize};
use sysinfo::{Disks, System};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskInfo {
    /// Mount point (e.g., "/", "/home")
    pub mount: String,
    /// Device name (e.g., "/dev/nvme0n1p2")
    pub device: String,
    /// Filesystem type (e.g., "ext4", "btrfs")
    pub fs_type: String,
    /// Total space in bytes
    pub total: u64,
    /// Used space in bytes
    pub used: u64,
    /// Available space in bytes
    pub available: u64,
    /// Usage percentage (0-100)
    pub usage: f32,
}

impl DiskInfo {
    pub fn from_system(_sys: &System) -> Vec<Self> {
        let disks = Disks::new_with_refreshed_list();
        
        disks
            .iter()
            .map(|disk| {
                let total = disk.total_space();
                let available = disk.available_space();
                let used = total.saturating_sub(available);
                let usage = if total > 0 {
                    (used as f32 / total as f32) * 100.0
                } else {
                    0.0
                };

                DiskInfo {
                    mount: disk.mount_point().to_string_lossy().to_string(),
                    device: disk.name().to_string_lossy().to_string(),
                    fs_type: disk.file_system().to_string_lossy().to_string(),
                    total,
                    used,
                    available,
                    usage,
                }
            })
            .collect()
    }

    /// Get root disk only
    pub fn root(_sys: &System) -> Option<Self> {
        Self::from_system(_sys)
            .into_iter()
            .find(|d| d.mount == "/")
    }

    /// Format used space
    pub fn used_str(&self) -> String {
        Self::format_bytes(self.used)
    }

    /// Format total space
    pub fn total_str(&self) -> String {
        Self::format_bytes(self.total)
    }

    /// Format available space
    pub fn available_str(&self) -> String {
        Self::format_bytes(self.available)
    }

    /// Format usage percentage
    pub fn usage_str(&self) -> String {
        format!("{:.0}%", self.usage)
    }

    fn format_bytes(bytes: u64) -> String {
        const TB: u64 = 1_099_511_627_776;
        const GB: u64 = 1_073_741_824;
        const MB: u64 = 1_048_576;

        if bytes >= TB {
            format!("{:.1} TB", bytes as f64 / TB as f64)
        } else if bytes >= GB {
            format!("{:.1} GB", bytes as f64 / GB as f64)
        } else {
            format!("{:.0} MB", bytes as f64 / MB as f64)
        }
    }
}
