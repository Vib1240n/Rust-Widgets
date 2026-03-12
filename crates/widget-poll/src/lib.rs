pub mod cpu;
pub mod memory;
pub mod disk;
pub mod network;
pub mod battery;
pub mod thermal;

pub use cpu::CpuInfo;
pub use memory::MemoryInfo;
pub use disk::DiskInfo;
pub use network::NetworkInfo;
pub use battery::BatteryInfo;
pub use thermal::ThermalInfo;

use sysinfo::System;
use std::sync::{Arc, Mutex};

/// System poller that caches sysinfo::System
pub struct Poller {
    sys: Arc<Mutex<System>>,
}

impl Poller {
    pub fn new() -> Self {
        Self {
            sys: Arc::new(Mutex::new(System::new_all())),
        }
    }

    /// Refresh all system info
    pub fn refresh(&self) {
        let mut sys = self.sys.lock().unwrap();
        sys.refresh_all();
    }

    /// Refresh only CPU info
    pub fn refresh_cpu(&self) {
        let mut sys = self.sys.lock().unwrap();
        sys.refresh_cpu_all();
    }

    /// Refresh only memory info
    pub fn refresh_memory(&self) {
        let mut sys = self.sys.lock().unwrap();
        sys.refresh_memory();
    }

    /// Get CPU information
    pub fn cpu(&self) -> CpuInfo {
        self.refresh_cpu();
        let sys = self.sys.lock().unwrap();
        CpuInfo::from_system(&sys)
    }

    /// Get memory information
    pub fn memory(&self) -> MemoryInfo {
        self.refresh_memory();
        let sys = self.sys.lock().unwrap();
        MemoryInfo::from_system(&sys)
    }

    /// Get disk information
    pub fn disk(&self) -> Vec<DiskInfo> {
        let sys = self.sys.lock().unwrap();
        DiskInfo::from_system(&sys)
    }

    /// Get network information
    pub fn network(&self) -> Vec<NetworkInfo> {
        let sys = self.sys.lock().unwrap();
        NetworkInfo::from_system(&sys)
    }

    /// Get battery information
    pub fn battery(&self) -> Option<BatteryInfo> {
        BatteryInfo::read()
    }

    /// Get thermal information
    pub fn thermal(&self) -> Vec<ThermalInfo> {
        let sys = self.sys.lock().unwrap();
        ThermalInfo::from_system(&sys)
    }
}

impl Default for Poller {
    fn default() -> Self {
        Self::new()
    }
}
