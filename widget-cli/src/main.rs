use clap::{Parser, Subcommand};
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
    /// Reload configuration
    Reload,
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
        Commands::Reload => reload_config(),
        Commands::Config { widget } => show_config_path(widget),
    }
}

fn get_binary_name(widget: &str) -> Option<&'static str> {
    match widget {
        "stats" | "stats-popup" => Some("rw-stats"),
        "media" | "media-player" | "player" => Some("rw-media"),
        "control" | "control-center" | "cc" => Some("rw-control"),
        "volume" | "volume-osd" => Some("rw-volume"),
        "brightness" | "brightness-osd" => Some("rw-brightness"),
        "power" | "power-menu" => Some("rw-power"),
        "calendar" => Some("rw-calendar"),
        _ => None,
    }
}
fn is_widget_running(binary: &str) -> bool {
    get_widget_pid(binary).is_some()
}

fn show_widget(widget: &str) {
    let Some(binary) = get_binary_name(widget) else {
        eprintln!("Unknown widget: {}", widget);
        eprintln!("Run 'rw list' to see available widgets");
        std::process::exit(1);
    };

    if is_widget_running(binary) {
        println!("{} is already running", widget);
        return;
    }

    // Use setsid to fully detach the process
    match Command::new("setsid")
        .arg("-f")
        .arg(binary)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(_) => println!("Started {}", widget),
        Err(e) => {
            eprintln!("Failed to start {}: {}", widget, e);
            std::process::exit(1);
        }
    }
}

fn get_widget_pid(binary: &str) -> Option<u32> {
    // Find the PID of a running widget by checking /proc
    if let Ok(entries) = std::fs::read_dir("/proc") {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            
            // Only check numeric directories (PIDs)
            if let Ok(pid) = name_str.parse::<u32>() {
                let comm_path = entry.path().join("comm");
                if let Ok(comm) = std::fs::read_to_string(&comm_path) {
                    if comm.trim() == binary {
                        return Some(pid);
                    }
                }
            }
        }
    }
    None
}

fn hide_widget(widget: &str) {
    let Some(binary) = get_binary_name(widget) else {
        eprintln!("Unknown widget: {}", widget);
        std::process::exit(1);
    };

    match get_widget_pid(binary) {
        Some(pid) => {
            // Kill the process directly by PID
            match Command::new("kill").arg(pid.to_string()).status() {
                Ok(status) if status.success() => println!("Stopped {}", widget),
                Ok(_) => {
                    // Try SIGKILL if regular kill failed
                    let _ = Command::new("kill").arg("-9").arg(pid.to_string()).status();
                    println!("Stopped {}", widget);
                }
                Err(e) => {
                    eprintln!("Failed to stop {}: {}", widget, e);
                    std::process::exit(1);
                }
            }
        }
        None => {
            println!("{} is not running", widget);
        }
    }
}

fn toggle_widget(widget: &str) {
    let Some(binary) = get_binary_name(widget) else {
        eprintln!("Unknown widget: {}", widget);
        eprintln!("Run 'rw list' to see available widgets");
        std::process::exit(1);
    };

    if is_widget_running(binary) {
        hide_widget(widget);
    } else {
        show_widget(widget);
    }
}

fn list_widgets() {
    println!("Available widgets:");
    println!();

    let widgets = [
        (
            "stats",
            "rw-stats",
            "System stats popup (CPU, RAM, disk, battery, temps)",
        ),
        (
            "control",
            "rw-control",
            "Control center (toggles, sliders, media, stats)",
        ),
        ("volume", "rw-volume", "Volume OSD [not implemented]"),
        (
            "brightness",
            "rw-brightness",
            "Brightness OSD [not implemented]",
        ),
        ("power", "rw-power", "Power menu [not implemented]"),
        (
            "calendar",
            "rw-calendar",
            "Calendar popup [not implemented]",
        ),
        (
            "media",
            "rw-media",
            "Media player with album art and controls",
        ),
    ];

    for (name, binary, desc) in widgets {
        let status = if is_widget_running(binary) {
            "\x1b[32m[running]\x1b[0m"
        } else {
            ""
        };
        println!("  {:<12} - {} {}", name, desc, status);
    }
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

fn reload_config() {
    let widgets = [
        "rw-stats",
        "rw-control",
        "rw-volume",
        "rw-brightness",
        "rw-power",
        "rw-calendar",
    ];

    for binary in widgets {
        if is_widget_running(binary) {
            let _ = std::process::Command::new("pkill")
                .arg("-HUP")
                .arg("-x")
                .arg(binary)
                .status();
        }
    }

    println!("Sent reload signal to running widgets");
}

fn show_config_path(widget: Option<String>) {
    let base = dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("~/.config"))
        .join("rw");

    match widget {
        Some(w) => {
            let widget_dir = base.join(&w);
            println!("Config: {}/config.toml", widget_dir.display());
            println!("Style:  {}/style.css", widget_dir.display());
        }
        None => {
            println!("Config directory: {}", base.display());
            println!();
            println!("Widget configs:");
            println!("  {}/stats-popup/config.toml", base.display());
            println!("  {}/control-center/config.toml", base.display());
        }
    }
}
