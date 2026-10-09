use std::collections::{HashMap, HashSet};
use std::process::Command;

#[derive(Debug, Clone)]
pub struct WifiNetwork {
    pub ssid: String,
    pub signal: u8,
    pub secured: bool,
    pub active: bool,
    pub known: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConnType {
    Wifi,
    Ethernet,
    None,
}

#[derive(Debug, Clone)]
pub struct NetworkStatus {
    pub wifi_enabled: bool,
    pub conn_type: ConnType,
    pub name: String,
}

fn nmcli(args: &[&str]) -> Option<String> {
    let out = Command::new("nmcli").args(args).output().ok()?;
    if out.status.success() {
        Some(String::from_utf8_lossy(&out.stdout).to_string())
    } else {
        None
    }
}

/// Unescape nmcli terse output (`\:` -> `:`, `\\` -> `\`)
fn unescape(s: &str) -> String {
    s.replace("\\:", ":").replace("\\\\", "\\")
}

pub fn wifi_enabled() -> bool {
    nmcli(&["radio", "wifi"])
        .map(|s| s.trim() == "enabled")
        .unwrap_or(false)
}

pub fn set_wifi_enabled(on: bool) {
    let _ = Command::new("nmcli")
        .args(["radio", "wifi", if on { "on" } else { "off" }])
        .spawn();
}

/// Names of saved connection profiles (used to flag "known" networks)
pub fn known_connections() -> HashSet<String> {
    nmcli(&["-t", "-f", "NAME", "connection", "show"])
        .unwrap_or_default()
        .lines()
        .map(|l| unescape(l).trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

pub fn get_status() -> NetworkStatus {
    let wifi_on = wifi_enabled();

    // DEVICE:TYPE:STATE:CONNECTION
    let out = nmcli(&[
        "-t",
        "-f",
        "DEVICE,TYPE,STATE,CONNECTION",
        "device",
        "status",
    ])
    .unwrap_or_default();

    let mut eth: Option<String> = None; // device name
    let mut wifi: Option<String> = None; // connection (SSID)

    for line in out.lines() {
        let parts: Vec<&str> = line.splitn(4, ':').collect();
        if parts.len() < 4 {
            continue;
        }
        let device = parts[0];
        let typ = parts[1];
        let state = parts[2];
        let conn = unescape(parts[3]);

        if state != "connected" {
            continue;
        }
        match typ {
            "ethernet" => eth = Some(device.to_string()),
            "wifi" => wifi = Some(conn),
            _ => {}
        }
    }

    if let Some(dev) = eth {
        return NetworkStatus {
            wifi_enabled: wifi_on,
            conn_type: ConnType::Ethernet,
            name: dev,
        };
    }
    if let Some(conn) = wifi {
        return NetworkStatus {
            wifi_enabled: wifi_on,
            conn_type: ConnType::Wifi,
            name: conn,
        };
    }
    NetworkStatus {
        wifi_enabled: wifi_on,
        conn_type: ConnType::None,
        name: "Not connected".to_string(),
    }
}

/// Deduped, signal-sorted list of available networks (top `max`).
///
/// nmcli returns one row per BSSID, so a single SSID appears many times across
/// bands/APs. We collapse by SSID keeping the strongest signal.
pub fn list_networks(max: usize) -> Vec<WifiNetwork> {
    let out = nmcli(&[
        "-t",
        "-f",
        "IN-USE,SIGNAL,SECURITY,SSID",
        "device",
        "wifi",
        "list",
    ])
    .unwrap_or_default();

    let known = known_connections();
    let mut best: HashMap<String, WifiNetwork> = HashMap::new();

    for line in out.lines() {
        let parts: Vec<&str> = line.splitn(4, ':').collect();
        if parts.len() < 4 {
            continue;
        }
        let active = parts[0].trim() == "*";
        let signal = parts[1].trim().parse::<u8>().unwrap_or(0);
        let secured = !parts[2].trim().is_empty();
        let ssid = unescape(parts[3]).trim().to_string();

        if ssid.is_empty() || ssid == "--" {
            continue; // hidden / no SSID
        }

        let known_ssid = known.contains(&ssid);
        best.entry(ssid.clone())
            .and_modify(|e| {
                if signal > e.signal {
                    e.signal = signal;
                    e.secured = secured;
                }
                if active {
                    e.active = true;
                }
            })
            .or_insert(WifiNetwork {
                ssid,
                signal,
                secured,
                active,
                known: known_ssid,
            });
    }

    let mut v: Vec<WifiNetwork> = best.into_values().collect();
    // Connected first, then strongest signal
    v.sort_by(|a, b| b.active.cmp(&a.active).then(b.signal.cmp(&a.signal)));
    v.truncate(max);
    v
}

pub fn rescan() {
    let _ = Command::new("nmcli")
        .args(["device", "wifi", "rescan"])
        .output();
}

pub fn connect(ssid: &str, password: Option<&str>, known: bool) -> Result<(), String> {
    let result = if known {
        Command::new("nmcli")
            .args(["connection", "up", "id", ssid])
            .output()
    } else if let Some(pw) = password {
        Command::new("nmcli")
            .args(["device", "wifi", "connect", ssid, "password", pw])
            .output()
    } else {
        Command::new("nmcli")
            .args(["device", "wifi", "connect", ssid])
            .output()
    };

    match result {
        Ok(o) if o.status.success() => Ok(()),
        Ok(o) => Err(String::from_utf8_lossy(&o.stderr).trim().to_string()),
        Err(e) => Err(e.to_string()),
    }
}

pub fn disconnect(ssid: &str) -> Result<(), String> {
    match Command::new("nmcli")
        .args(["connection", "down", "id", ssid])
        .output()
    {
        Ok(o) if o.status.success() => Ok(()),
        Ok(o) => Err(String::from_utf8_lossy(&o.stderr).trim().to_string()),
        Err(e) => Err(e.to_string()),
    }
}

pub fn forget(ssid: &str) -> Result<(), String> {
    match Command::new("nmcli")
        .args(["connection", "delete", "id", ssid])
        .output()
    {
        Ok(o) if o.status.success() => Ok(()),
        Ok(o) => Err(String::from_utf8_lossy(&o.stderr).trim().to_string()),
        Err(e) => Err(e.to_string()),
    }
}

pub fn signal_icon(signal: u8) -> &'static str {
    match signal {
        80..=u8::MAX => "network-wireless-signal-excellent-symbolic",
        55..=79 => "network-wireless-signal-good-symbolic",
        30..=54 => "network-wireless-signal-ok-symbolic",
        5..=29 => "network-wireless-signal-weak-symbolic",
        _ => "network-wireless-signal-none-symbolic",
    }
}
