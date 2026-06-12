use clap::{Parser, Subcommand};
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::process::{Command, Stdio};

#[derive(Parser)]
#[command(name = "rw")]
#[command(about = "Rust Widgets - Desktop widget framework", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Show a widget
    Show {
        /// Widget name
        widget: String,
    },
    /// Hide a widget (kill it)
    Hide {
        /// Widget name
        widget: String,
    },
    /// Toggle widget visibility
    Toggle {
        /// Widget name
        widget: String,
    },
    /// List available widgets
    List,
    /// Show system stats (debug)
    Stats,
    /// Reload configuration (optionally for a specific widget)
    Reload {
        /// Widget name (optional - reloads all if not specified)
        widget: Option<String>,
    },
    /// Restart a widget (kill and start)
    Restart {
        /// Widget name
        widget: String,
    },
    /// Show config paths
    Config {
        /// Widget name (optional)
        widget: Option<String>,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Show { widget } => show_widget(&widget),
        Commands::Hide { widget } => hide_widget(&widget),
        Commands::Toggle { widget } => toggle_widget(&widget),
        Commands::List => list_widgets(),
        Commands::Stats => print_stats(),
        Commands::Reload { widget } => reload_config(widget),
        Commands::Restart { widget } => restart_widget(&widget),
        Commands::Config { widget } => show_config_path(widget),
    }
}

/// Widget metadata
struct WidgetInfo {
    name: &'static str,
    binary: &'static str,
    description: &'static str,
    is_daemon: bool,
    socket_path: Option<&'static str>,
    /// Whether this widget supports SIGHUP reload
    supports_sighup: bool,
}

const WIDGETS: &[WidgetInfo] = &[
    WidgetInfo {
        name: "stats",
        binary: "rw-stats",
        description: "System stats popup (CPU, RAM, disk, battery, temps)",
        is_daemon: false,
        socket_path: None,
        supports_sighup: true,
    },
    WidgetInfo {
        name: "control",
        binary: "rw-control",
        description: "Control center (toggles, sliders, media, stats)",
        is_daemon: false,
        socket_path: None,
        supports_sighup: true,
    },
    WidgetInfo {
        name: "volume",
        binary: "rw-volume",
        description: "Volume control with app mixer",
        is_daemon: false,
        socket_path: None,
        supports_sighup: true,
    },
    WidgetInfo {
        name: "volume-osd",
        binary: "rw-volume-osd",
        description: "Volume OSD (daemon)",
        is_daemon: true,
        socket_path: Some("/tmp/rw-volume-osd.sock"),
        supports_sighup: false, // Needs restart for config changes
    },
    WidgetInfo {
        name: "notifications",
        binary: "rw-notifications",
        description: "Notification center (daemon, replaces swaync)",
        is_daemon: true,
        socket_path: Some("/tmp/rw-notifications.sock"),
        supports_sighup: false, // Needs restart for config changes
    },
    WidgetInfo {
        name: "media",
        binary: "rw-media",
        description: "Media player with album art and controls",
        is_daemon: false,
        socket_path: None,
        supports_sighup: true,
    },
    WidgetInfo {
        name: "brightness",
        binary: "rw-brightness",
        description: "Brightness OSD [not implemented]",
        is_daemon: true,
        socket_path: Some("/tmp/rw-brightness.sock"),
        supports_sighup: false,
    },
    WidgetInfo {
        name: "power",
        binary: "rw-power",
        description: "Power menu [not implemented]",
        is_daemon: false,
        socket_path: None,
        supports_sighup: true,
    },
    WidgetInfo {
        name: "calendar",
        binary: "rw-calendar",
        description: "Calendar popup [not implemented]",
        is_daemon: false,
        socket_path: None,
        supports_sighup: true,
    },
];

fn get_widget_info(widget: &str) -> Option<&'static WidgetInfo> {
    // Check primary name
    if let Some(info) = WIDGETS.iter().find(|w| w.name == widget) {
        return Some(info);
    }

    // Check aliases
    match widget {
        "stats-popup" => WIDGETS.iter().find(|w| w.name == "stats"),
        "control-center" | "cc" => WIDGETS.iter().find(|w| w.name == "control"),
        "volume-control" => WIDGETS.iter().find(|w| w.name == "volume"),
        "osd" => WIDGETS.iter().find(|w| w.name == "volume-osd"),
        "notification-center" | "nc" => WIDGETS.iter().find(|w| w.name == "notifications"),
        "media-player" | "player" => WIDGETS.iter().find(|w| w.name == "media"),
        "brightness-osd" => WIDGETS.iter().find(|w| w.name == "brightness"),
        "power-menu" => WIDGETS.iter().find(|w| w.name == "power"),
        _ => None,
    }
}

fn get_widget_pid(binary: &str) -> Option<u32> {
    // Linux truncates /proc/*/comm to 15 characters
    let truncated_name = if binary.len() > 15 {
        &binary[..15]
    } else {
        binary
    };

    if let Ok(entries) = std::fs::read_dir("/proc") {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();

            if let Ok(pid) = name_str.parse::<u32>() {
                let comm_path = entry.path().join("comm");
                if let Ok(comm) = std::fs::read_to_string(&comm_path) {
                    let comm_trimmed = comm.trim();
                    if comm_trimmed == binary || comm_trimmed == truncated_name {
                        return Some(pid);
                    }
                }
            }
        }
    }
    None
}

fn is_widget_running(binary: &str) -> bool {
    get_widget_pid(binary).is_some()
}

fn send_daemon_command(socket_path: &str, command: &str) -> Result<(), String> {
    let mut stream = UnixStream::connect(socket_path)
        .map_err(|e| format!("Failed to connect to daemon: {}", e))?;

    stream
        .write_all(command.as_bytes())
        .map_err(|e| format!("Failed to send command: {}", e))?;

    Ok(())
}

fn start_widget(info: &WidgetInfo) {
    match Command::new("setsid")
        .arg("-f")
        .arg(info.binary)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(_) => println!("Started {}", info.name),
        Err(e) => {
            eprintln!("Failed to start {}: {}", info.name, e);
            std::process::exit(1);
        }
    }
}

fn kill_widget(info: &WidgetInfo) -> bool {
    if let Some(pid) = get_widget_pid(info.binary) {
        match Command::new("kill").arg(pid.to_string()).status() {
            Ok(status) if status.success() => {
                // Wait a moment for process to terminate
                std::thread::sleep(std::time::Duration::from_millis(100));
                return true;
            }
            Ok(_) => {
                // Try SIGKILL
                let _ = Command::new("kill").arg("-9").arg(pid.to_string()).status();
                std::thread::sleep(std::time::Duration::from_millis(100));
                return true;
            }
            Err(_) => return false,
        }
    }
    false
}

fn show_widget(widget: &str) {
    let Some(info) = get_widget_info(widget) else {
        eprintln!("Unknown widget: {}", widget);
        eprintln!("Run 'rw list' to see available widgets");
        std::process::exit(1);
    };

    if info.is_daemon {
        if is_widget_running(info.binary) {
            if let Some(socket) = info.socket_path {
                match send_daemon_command(socket, "show\n") {
                    Ok(_) => println!("Showing {}", info.name),
                    Err(e) => eprintln!("{}", e),
                }
            }
        } else {
            start_widget(info);
            if let Some(socket) = info.socket_path {
                for _ in 0..10 {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    if std::path::Path::new(socket).exists() {
                        break;
                    }
                }
                match send_daemon_command(socket, "show\n") {
                    Ok(_) => println!("Showing {}", info.name),
                    Err(e) => eprintln!("Daemon started but show failed: {}", e),
                }
            }
        }
        return;
    }

    if is_widget_running(info.binary) {
        println!("{} is already running", info.name);
        return;
    }

    start_widget(info);
}

fn hide_widget(widget: &str) {
    let Some(info) = get_widget_info(widget) else {
        eprintln!("Unknown widget: {}", widget);
        std::process::exit(1);
    };

    if info.is_daemon {
        if is_widget_running(info.binary) {
            if let Some(socket) = info.socket_path {
                match send_daemon_command(socket, "hide\n") {
                    Ok(_) => println!("Hiding {}", info.name),
                    Err(e) => eprintln!("{}", e),
                }
            }
        } else {
            println!("{} daemon is not running", info.name);
        }
        return;
    }

    if kill_widget(info) {
        println!("Stopped {}", info.name);
    } else {
        println!("{} is not running", info.name);
    }
}

fn toggle_widget(widget: &str) {
    let Some(info) = get_widget_info(widget) else {
        eprintln!("Unknown widget: {}", widget);
        eprintln!("Run 'rw list' to see available widgets");
        std::process::exit(1);
    };

    if info.is_daemon {
        if is_widget_running(info.binary) {
            if let Some(socket) = info.socket_path {
                match send_daemon_command(socket, "toggle\n") {
                    Ok(_) => println!("Toggled {}", info.name),
                    Err(e) => eprintln!("{}", e),
                }
            }
        } else {
            // Start daemon, wait for socket, then send toggle
            start_widget(info);
            if let Some(socket) = info.socket_path {
                // Wait for daemon to create socket
                for _ in 0..10 {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    if std::path::Path::new(socket).exists() {
                        break;
                    }
                }
                match send_daemon_command(socket, "toggle\n") {
                    Ok(_) => println!("Toggled {}", info.name),
                    Err(e) => eprintln!("Daemon started but toggle failed: {}", e),
                }
            }
        }
        return;
    }

    if is_widget_running(info.binary) {
        hide_widget(widget);
    } else {
        show_widget(widget);
    }
}

fn restart_widget(widget: &str) {
    let Some(info) = get_widget_info(widget) else {
        eprintln!("Unknown widget: {}", widget);
        eprintln!("Run 'rw list' to see available widgets");
        std::process::exit(1);
    };

    let was_running = is_widget_running(info.binary);

    if was_running {
        println!("Stopping {}...", info.name);
        kill_widget(info);
        // Give it a moment to fully terminate
        std::thread::sleep(std::time::Duration::from_millis(200));
    }

    println!("Starting {}...", info.name);
    start_widget(info);

    if was_running {
        println!("Restarted {}", info.name);
    }
}

fn reload_config(widget: Option<String>) {
    match widget {
        Some(w) => reload_single_widget(&w),
        None => reload_all_widgets(),
    }
}

fn reload_single_widget(widget: &str) {
    let Some(info) = get_widget_info(widget) else {
        eprintln!("Unknown widget: {}", widget);
        eprintln!("Run 'rw list' to see available widgets");
        std::process::exit(1);
    };

    if !is_widget_running(info.binary) {
        println!("{} is not running", info.name);
        return;
    }

    if info.is_daemon || !info.supports_sighup {
        // Daemon widgets need restart for config reload
        println!(
            "Restarting {} (daemon widgets require restart for config changes)...",
            info.name
        );
        restart_widget(widget);
    } else {
        // Non-daemon widgets can use SIGHUP
        if let Some(pid) = get_widget_pid(info.binary) {
            match Command::new("kill")
                .arg("-HUP")
                .arg(pid.to_string())
                .status()
            {
                Ok(status) if status.success() => {
                    println!("Reloaded {} (sent SIGHUP)", info.name);
                }
                _ => {
                    eprintln!(
                        "Failed to send reload signal to {}, restarting instead...",
                        info.name
                    );
                    restart_widget(widget);
                }
            }
        }
    }
}

fn reload_all_widgets() {
    println!("Reloading all widgets...\n");

    let mut reloaded = 0;
    let mut restarted = 0;

    for info in WIDGETS {
        if !is_widget_running(info.binary) {
            continue;
        }

        if info.is_daemon || !info.supports_sighup {
            // Restart daemon widgets
            println!("  Restarting {}...", info.name);
            kill_widget(info);
            std::thread::sleep(std::time::Duration::from_millis(200));
            start_widget(info);
            restarted += 1;
        } else {
            // Send SIGHUP to non-daemon widgets
            if let Some(pid) = get_widget_pid(info.binary) {
                let _ = Command::new("kill")
                    .arg("-HUP")
                    .arg(pid.to_string())
                    .status();
                println!("  Reloaded {} (SIGHUP)", info.name);
                reloaded += 1;
            }
        }
    }

    println!();
    if reloaded > 0 || restarted > 0 {
        println!("Done: {} reloaded, {} restarted", reloaded, restarted);
    } else {
        println!("No widgets were running");
    }
}

fn list_widgets() {
    println!("Available widgets:");
    println!();

    for info in WIDGETS {
        let status = if is_widget_running(info.binary) {
            "\x1b[32m[running]\x1b[0m"
        } else {
            ""
        };
        let daemon_tag = if info.is_daemon { " (daemon)" } else { "" };
        println!(
            "  {:<14} - {}{} {}",
            info.name, info.description, daemon_tag, status
        );
    }

    println!();
    println!("Aliases:");
    println!("  cc, control-center -> control");
    println!("  nc, notification-center -> notifications");
    println!("  osd -> volume-osd");
    println!("  player, media-player -> media");
}

fn print_stats() {
    use widget_poll::Poller;

    let poller = Poller::new();

    println!("=== System Stats ===\n");

    let cpu = poller.cpu();
    println!("CPU: {} ({})", cpu.name, cpu.core_count);
    println!("  Usage: {}", cpu.usage_str());
    println!("  Frequency: {}", cpu.frequency_str());
    println!();

    let mem = poller.memory();
    println!("Memory:");
    println!(
        "  Used: {} / {} ({})",
        mem.used_str(),
        mem.total_str(),
        mem.usage_str()
    );
    println!();

    let disks = poller.disk();
    println!("Disks:");
    for disk in disks {
        println!(
            "  {} - {} / {} ({})",
            disk.mount,
            disk.used_str(),
            disk.total_str(),
            disk.usage_str()
        );
    }
    println!();

    if let Some(bat) = poller.battery() {
        println!("Battery: {} {:?}", bat.percentage_str(), bat.status);
        println!();
    }

    let temps = poller.thermal();
    if !temps.is_empty() {
        println!("Temperatures:");
        for t in temps.iter().take(8) {
            println!("  {}: {}", t.label, t.temp_str());
        }
    }
}

fn show_config_path(widget: Option<String>) {
    let base = dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("~/.config"))
        .join("rw");

    match widget {
        Some(w) => {
            let config_name = match w.as_str() {
                "stats" | "stats-popup" => "stats-popup",
                "control" | "control-center" | "cc" => "control-center",
                "volume" | "volume-control" => "volume-control",
                "volume-osd" | "osd" => "volume-osd",
                "notifications" | "notification-center" | "nc" => "notification-center",
                "media" | "media-player" | "player" => "media-player",
                _ => &w,
            };
            let widget_dir = base.join(config_name);
            println!("Config: {}/config.toml", widget_dir.display());
            println!("Style:  {}/style.css (global)", base.display());
        }
        None => {
            println!("Config directory: {}", base.display());
            println!("Global style:     {}/style.css", base.display());
            println!();
            println!("Widget configs:");
            for info in WIDGETS {
                let config_name = match info.name {
                    "stats" => "stats-popup",
                    "control" => "control-center",
                    "volume" => "volume-control",
                    "media" => "media-player",
                    _ => info.name,
                };
                println!("  {}/{}/config.toml", base.display(), config_name);
            }
        }
    }
}
