use std::process::Command;

#[derive(Debug, Clone)]
pub struct AudioDevice {
    pub id: u32,
    pub name: String,        // Internal name (e.g., "alsa_output.usb-...")
    pub description: String, // Human-readable (e.g., "Starship HD Audio Analog Stereo")
    pub volume: f32,
    pub muted: bool,
    pub is_default: bool,
}

#[derive(Debug, Clone)]
pub struct AudioStream {
    pub id: u32,
    pub name: String,
    pub app_name: String,
    pub icon_name: String,
    pub volume: f32,
    pub muted: bool,
}

// Get master output (sink) volume
pub fn get_sink_volume() -> f32 {
    Command::new("wpctl")
        .args(["get-volume", "@DEFAULT_AUDIO_SINK@"])
        .output()
        .ok()
        .and_then(|o| {
            let s = String::from_utf8_lossy(&o.stdout);
            // Format: "Volume: 0.75" or "Volume: 0.75 [MUTED]"
            s.split_whitespace()
                .nth(1)
                .and_then(|v| v.parse::<f32>().ok())
        })
        .unwrap_or(0.5)
}

// Check if sink is muted
pub fn is_sink_muted() -> bool {
    Command::new("wpctl")
        .args(["get-volume", "@DEFAULT_AUDIO_SINK@"])
        .output()
        .ok()
        .map(|o| {
            let s = String::from_utf8_lossy(&o.stdout);
            s.contains("[MUTED]")
        })
        .unwrap_or(false)
}

// Set master output volume (0.0 - 1.0+)
pub fn set_sink_volume(vol: f32) {
    let _ = Command::new("wpctl")
        .args(["set-volume", "@DEFAULT_AUDIO_SINK@", &format!("{:.2}", vol)])
        .spawn();
}

// Toggle sink mute
pub fn toggle_sink_mute() {
    let _ = Command::new("wpctl")
        .args(["set-mute", "@DEFAULT_AUDIO_SINK@", "toggle"])
        .spawn();
}

// Get master input (source) volume
pub fn get_source_volume() -> f32 {
    Command::new("wpctl")
        .args(["get-volume", "@DEFAULT_AUDIO_SOURCE@"])
        .output()
        .ok()
        .and_then(|o| {
            let s = String::from_utf8_lossy(&o.stdout);
            s.split_whitespace()
                .nth(1)
                .and_then(|v| v.parse::<f32>().ok())
        })
        .unwrap_or(0.5)
}

// Check if source is muted
pub fn is_source_muted() -> bool {
    Command::new("wpctl")
        .args(["get-volume", "@DEFAULT_AUDIO_SOURCE@"])
        .output()
        .ok()
        .map(|o| {
            let s = String::from_utf8_lossy(&o.stdout);
            s.contains("[MUTED]")
        })
        .unwrap_or(false)
}

// Set master input volume
pub fn set_source_volume(vol: f32) {
    let _ = Command::new("wpctl")
        .args([
            "set-volume",
            "@DEFAULT_AUDIO_SOURCE@",
            &format!("{:.2}", vol),
        ])
        .spawn();
}

// Toggle source mute
pub fn toggle_source_mute() {
    let _ = Command::new("wpctl")
        .args(["set-mute", "@DEFAULT_AUDIO_SOURCE@", "toggle"])
        .spawn();
}

// Get all output devices (sinks) using pactl - more reliable than wpctl
pub fn get_sinks() -> Vec<AudioDevice> {
    get_devices_pactl("sinks", "sink")
}

// Get all input devices (sources) using pactl
pub fn get_sources() -> Vec<AudioDevice> {
    get_devices_pactl("sources", "source")
}

fn get_devices_pactl(list_type: &str, default_type: &str) -> Vec<AudioDevice> {
    // Get default device name first
    let default_cmd = format!("get-default-{}", default_type);
    let default_name = Command::new("pactl")
        .args([&default_cmd])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();

    // List all devices
    let output = Command::new("pactl")
        .args(["list", list_type])
        .output()
        .ok();

    let mut devices = Vec::new();

    if let Some(output) = output {
        let s = String::from_utf8_lossy(&output.stdout);
        let mut current_device: Option<AudioDevice> = None;

        for line in s.lines() {
            let line_trimmed = line.trim();

            // Start of a new sink/source block
            // Format: "Sink #47" or "Source #52"
            if (line_trimmed.starts_with("Sink #") || line_trimmed.starts_with("Source #")) {
                // Save previous device if exists
                if let Some(device) = current_device.take() {
                    // Filter out monitor devices for sinks
                    if list_type == "sinks" && !device.name.contains(".monitor") {
                        devices.push(device);
                    } else if list_type == "sources" {
                        devices.push(device);
                    }
                }

                let id_str = line_trimmed.split('#').nth(1).unwrap_or("0");
                let id: u32 = id_str.parse().unwrap_or(0);

                current_device = Some(AudioDevice {
                    id,
                    name: String::new(),
                    description: String::new(),
                    volume: 1.0,
                    muted: false,
                    is_default: false,
                });
            } else if let Some(ref mut device) = current_device {
                if line_trimmed.starts_with("Name:") {
                    device.name = line_trimmed
                        .strip_prefix("Name:")
                        .unwrap_or("")
                        .trim()
                        .to_string();
                    device.is_default = device.name == default_name;
                } else if line_trimmed.starts_with("Description:") {
                    device.description = line_trimmed
                        .strip_prefix("Description:")
                        .unwrap_or("")
                        .trim()
                        .to_string();
                } else if line_trimmed.starts_with("Mute:") {
                    device.muted = line_trimmed.contains("yes");
                } else if line_trimmed.starts_with("Volume:") {
                    // Parse "Volume: front-left: 65536 / 100% / 0.00 dB, ..."
                    if let Some(pct_start) = line_trimmed.find("/ ") {
                        let rest = &line_trimmed[pct_start + 2..];
                        if let Some(pct_end) = rest.find('%') {
                            if let Ok(pct) = rest[..pct_end].trim().parse::<u32>() {
                                device.volume = pct as f32 / 100.0;
                            }
                        }
                    }
                }
            }
        }

        // Don't forget the last device
        if let Some(device) = current_device {
            if list_type == "sinks" && !device.name.contains(".monitor") {
                devices.push(device);
            } else if list_type == "sources" {
                devices.push(device);
            }
        }
    }

    devices
}

// Set default output device by name (pactl uses names, not IDs)
pub fn set_default_sink(id: u32) {
    // First we need to get the name from the ID
    if let Some(device) = get_sinks().iter().find(|d| d.id == id) {
        let _ = Command::new("pactl")
            .args(["set-default-sink", &device.name])
            .status(); // Use status() to wait for completion

        // Move all current streams to new sink
        move_streams_to_sink(&device.name);
    }
}

// Set default input device by name
pub fn set_default_source(id: u32) {
    if let Some(device) = get_sources().iter().find(|d| d.id == id) {
        let _ = Command::new("pactl")
            .args(["set-default-source", &device.name])
            .status();
    }
}

// Move all playing streams to the new sink (like your bash script does)
fn move_streams_to_sink(sink_name: &str) {
    let output = Command::new("pactl")
        .args(["list", "short", "sink-inputs"])
        .output()
        .ok();

    if let Some(output) = output {
        let s = String::from_utf8_lossy(&output.stdout);
        for line in s.lines() {
            if let Some(id_str) = line.split_whitespace().next() {
                if let Ok(_id) = id_str.parse::<u32>() {
                    let _ = Command::new("pactl")
                        .args(["move-sink-input", id_str, sink_name])
                        .status();
                }
            }
        }
    }
}

// Get all audio streams (apps playing audio)
pub fn get_streams() -> Vec<AudioStream> {
    get_streams_pactl()
}

fn get_streams_pactl() -> Vec<AudioStream> {
    let output = Command::new("pactl")
        .args(["list", "sink-inputs"])
        .output()
        .ok();

    let mut streams = Vec::new();

    if let Some(output) = output {
        let s = String::from_utf8_lossy(&output.stdout);
        let mut current_stream: Option<AudioStream> = None;

        for line in s.lines() {
            let line = line.trim();

            if line.starts_with("Sink Input #") {
                // Save previous stream if exists
                if let Some(stream) = current_stream.take() {
                    if !stream.app_name.is_empty() {
                        streams.push(stream);
                    }
                }

                let id: u32 = line
                    .strip_prefix("Sink Input #")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);

                current_stream = Some(AudioStream {
                    id,
                    name: String::new(),
                    app_name: String::new(),
                    icon_name: "audio-x-generic-symbolic".to_string(),
                    volume: 1.0,
                    muted: false,
                });
            } else if let Some(ref mut stream) = current_stream {
                if line.starts_with("application.name = ") {
                    stream.app_name = line
                        .strip_prefix("application.name = ")
                        .unwrap_or("")
                        .trim_matches('"')
                        .to_string();
                } else if line.starts_with("media.name = ") {
                    stream.name = line
                        .strip_prefix("media.name = ")
                        .unwrap_or("")
                        .trim_matches('"')
                        .to_string();
                } else if line.starts_with("application.icon_name = ") {
                    stream.icon_name = line
                        .strip_prefix("application.icon_name = ")
                        .unwrap_or("")
                        .trim_matches('"')
                        .to_string();
                } else if line.starts_with("Volume:") {
                    // Parse "Volume: front-left: 65536 / 100% / 0.00 dB, ..."
                    if let Some(pct_start) = line.find("/ ") {
                        let rest = &line[pct_start + 2..];
                        if let Some(pct_end) = rest.find('%') {
                            if let Ok(pct) = rest[..pct_end].trim().parse::<u32>() {
                                stream.volume = pct as f32 / 100.0;
                            }
                        }
                    }
                } else if line.starts_with("Mute:") {
                    stream.muted = line.contains("yes");
                }
            }
        }

        // Don't forget the last stream
        if let Some(stream) = current_stream {
            if !stream.app_name.is_empty() {
                streams.push(stream);
            }
        }
    }

    streams
}

// Set stream volume
pub fn set_stream_volume(id: u32, vol: f32) {
    let pct = (vol * 100.0) as u32;
    let _ = Command::new("pactl")
        .args([
            "set-sink-input-volume",
            &id.to_string(),
            &format!("{}%", pct),
        ])
        .spawn();
}

// Toggle stream mute
pub fn toggle_stream_mute(id: u32) {
    let _ = Command::new("pactl")
        .args(["set-sink-input-mute", &id.to_string(), "toggle"])
        .spawn();
}

// Get appropriate icon for app
pub fn get_app_icon(app_name: &str) -> &'static str {
    let lower = app_name.to_lowercase();

    if lower.contains("spotify") {
        "emblem-music-symbolic"
    } else if lower.contains("firefox") {
        "firefox-symbolic"
    } else if lower.contains("chrom") {
        "web-browser-symbolic"
    } else if lower.contains("discord") || lower.contains("vesktop") {
        "user-available-symbolic"
    } else if lower.contains("steam") || lower.contains("game") {
        "applications-games-symbolic"
    } else if lower.contains("mpv") || lower.contains("vlc") {
        "video-x-generic-symbolic"
    } else {
        "audio-x-generic-symbolic"
    }
}
