use serde::{Deserialize, Serialize};
use sysinfo::{Networks, System};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInfo {
    /// Interface name (e.g., "wlo1", "eth0")
    pub interface: String,
    /// Total bytes received
    pub rx_bytes: u64,
    /// Total bytes transmitted
    pub tx_bytes: u64,
    /// Bytes received since last refresh
    pub rx_rate: u64,
    /// Bytes transmitted since last refresh
    pub tx_rate: u64,
}

impl NetworkInfo {
    pub fn from_system(_sys: &System) -> Vec<Self> {
        let networks = Networks::new_with_refreshed_list();
        
        networks
            .iter()
            .filter(|(name, _)| !name.starts_with("lo") && !name.starts_with("docker"))
            .map(|(name, data)| NetworkInfo {
                interface: name.to_string(),
                rx_bytes: data.total_received(),
                tx_bytes: data.total_transmitted(),
                rx_rate: data.received(),
                tx_rate: data.transmitted(),
            })
            .collect()
    }

    /// Get primary network interface (first non-loopback)
    pub fn primary(_sys: &System) -> Option<Self> {
        Self::from_system(_sys).into_iter().next()
    }

    /// Format receive rate
    pub fn rx_rate_str(&self) -> String {
        Self::format_rate(self.rx_rate)
    }

    /// Format transmit rate
    pub fn tx_rate_str(&self) -> String {
        Self::format_rate(self.tx_rate)
    }

    fn format_rate(bytes: u64) -> String {
        const KB: u64 = 1024;
        const MB: u64 = 1_048_576;
        const GB: u64 = 1_073_741_824;

        if bytes >= GB {
            format!("{:.1} GB/s", bytes as f64 / GB as f64)
        } else if bytes >= MB {
            format!("{:.1} MB/s", bytes as f64 / MB as f64)
        } else if bytes >= KB {
            format!("{:.1} KB/s", bytes as f64 / KB as f64)
        } else {
            format!("{} B/s", bytes)
        }
    }
}
