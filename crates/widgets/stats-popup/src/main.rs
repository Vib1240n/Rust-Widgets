mod animation;
mod config;
mod css;

use config::Config;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow, Box, Label, Orientation, ProgressBar};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;
use widget_poll::Poller;

const APP_ID: &str = "com.vib1240n.rust-widgets.stats";
const BINARY_NAME: &str = "rw-stats";

fn main() {
    // Enforce single instance - exit if already running
    if is_already_running() {
        eprintln!("rw-stats is already running");
        std::process::exit(0);
    }

    tracing_subscriber::fmt().with_env_filter("info").init();

    let app = Application::builder().application_id(APP_ID).build();

    app.connect_activate(build_ui);
    app.run();
}

/// Check if another instance of this widget is already running
fn is_already_running() -> bool {
    let my_pid = std::process::id();
    
    if let Ok(entries) = std::fs::read_dir("/proc") {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            
            if let Ok(pid) = name_str.parse::<u32>() {
                // Skip our own process
                if pid == my_pid {
                    continue;
                }
                
                let comm_path = entry.path().join("comm");
                if let Ok(comm) = std::fs::read_to_string(&comm_path) {
                    if comm.trim() == BINARY_NAME {
                        return true;
                    }
                }
            }
        }
    }
    false
}

fn build_ui(app: &Application) {
    let config = Rc::new(Config::load());
    css::load();

    let window = ApplicationWindow::builder()
        .application(app)
        .title("System Stats")
        .build();

    // Layer shell setup
    window.init_layer_shell();
    window.set_layer(Layer::Overlay);
    window.set_namespace("rust-widgets");
    window.set_keyboard_mode(KeyboardMode::OnDemand);

    // Position from config
    apply_position(&window, &config);

    // Main container
    let container = Box::new(Orientation::Vertical, 0);
    container.add_css_class("stats-container");
    container.set_width_request(config.appearance.width);

    // Create labels
    let cpu_label = Label::new(Some("--"));
    cpu_label.add_css_class("stat-value");
    cpu_label.set_halign(gtk4::Align::End);
    cpu_label.set_hexpand(true);
    let cpu_bar = ProgressBar::new();

    let mem_label = Label::new(Some("--"));
    mem_label.add_css_class("stat-value");
    mem_label.set_halign(gtk4::Align::End);
    mem_label.set_hexpand(true);
    let mem_bar = ProgressBar::new();

    let disk_label = Label::new(Some("--"));
    disk_label.add_css_class("stat-value");
    disk_label.set_halign(gtk4::Align::End);
    disk_label.set_hexpand(true);
    let disk_bar = ProgressBar::new();

    let battery_label = Label::new(Some("--"));
    battery_label.add_css_class("stat-value");
    battery_label.set_halign(gtk4::Align::End);
    battery_label.set_hexpand(true);

    let temps_box = Box::new(Orientation::Vertical, 2);

    // CPU Section
    if config.sections.cpu {
        let cpu_title = Label::new(Some("CPU"));
        cpu_title.add_css_class("section-title");
        cpu_title.set_halign(gtk4::Align::Start);
        container.append(&cpu_title);

        let cpu_row = Box::new(Orientation::Horizontal, 8);
        cpu_row.add_css_class("stat-row");
        let cpu_name = Label::new(Some("Usage"));
        cpu_name.add_css_class("stat-label");
        cpu_row.append(&cpu_name);
        cpu_row.append(&cpu_label);
        container.append(&cpu_row);
        container.append(&cpu_bar);
    }

    // Memory Section
    if config.sections.memory {
        let mem_title = Label::new(Some("MEMORY"));
        mem_title.add_css_class("section-title");
        mem_title.set_halign(gtk4::Align::Start);
        container.append(&mem_title);

        let mem_row = Box::new(Orientation::Horizontal, 8);
        mem_row.add_css_class("stat-row");
        let mem_name = Label::new(Some("Used"));
        mem_name.add_css_class("stat-label");
        mem_row.append(&mem_name);
        mem_row.append(&mem_label);
        container.append(&mem_row);
        container.append(&mem_bar);
    }

    // Disk Section
    if config.sections.disk {
        let disk_title = Label::new(Some("DISK"));
        disk_title.add_css_class("section-title");
        disk_title.set_halign(gtk4::Align::Start);
        container.append(&disk_title);

        let disk_row = Box::new(Orientation::Horizontal, 8);
        disk_row.add_css_class("stat-row");
        let disk_name = Label::new(Some("Root"));
        disk_name.add_css_class("stat-label");
        disk_row.append(&disk_name);
        disk_row.append(&disk_label);
        container.append(&disk_row);
        container.append(&disk_bar);
    }

    // Battery Section
    if config.sections.battery {
        let bat_title = Label::new(Some("BATTERY"));
        bat_title.add_css_class("section-title");
        bat_title.set_halign(gtk4::Align::Start);
        container.append(&bat_title);

        let bat_row = Box::new(Orientation::Horizontal, 8);
        bat_row.add_css_class("stat-row");
        let bat_name = Label::new(Some("Charge"));
        bat_name.add_css_class("stat-label");
        bat_row.append(&bat_name);
        bat_row.append(&battery_label);
        container.append(&bat_row);
    }

    // Temps Section
    if config.sections.temperatures {
        let temps_title = Label::new(Some("TEMPERATURES"));
        temps_title.add_css_class("section-title");
        temps_title.set_halign(gtk4::Align::Start);
        container.append(&temps_title);
        container.append(&temps_box);
    }

    window.set_child(Some(&container));

    // Setup polling
    let poller = Rc::new(RefCell::new(Poller::new()));
    let config_clone = config.clone();

    let cpu_label_clone = cpu_label.clone();
    let cpu_bar_clone = cpu_bar.clone();
    let mem_label_clone = mem_label.clone();
    let mem_bar_clone = mem_bar.clone();
    let disk_label_clone = disk_label.clone();
    let disk_bar_clone = disk_bar.clone();
    let battery_label_clone = battery_label.clone();
    let temps_box_clone = temps_box.clone();
    let poller_clone = poller.clone();

    // Initial update
    update_stats(
        &poller_clone,
        &config_clone,
        &cpu_label_clone,
        &cpu_bar_clone,
        &mem_label_clone,
        &mem_bar_clone,
        &disk_label_clone,
        &disk_bar_clone,
        &battery_label_clone,
        &temps_box_clone,
    );

    // Update at configured interval
    let poll_interval = config.behavior.poll_interval;
    glib::timeout_add_local(Duration::from_millis(poll_interval), move || {
        update_stats(
            &poller_clone,
            &config_clone,
            &cpu_label_clone,
            &cpu_bar_clone,
            &mem_label_clone,
            &mem_bar_clone,
            &disk_label_clone,
            &disk_bar_clone,
            &battery_label_clone,
            &temps_box_clone,
        );
        glib::ControlFlow::Continue
    });

    // Track if closing
    let is_closing = Rc::new(RefCell::new(false));

    // Close on Escape
    if config.behavior.close_on_escape {
        let controller = gtk4::EventControllerKey::new();
        let window_clone = window.clone();
        let is_closing_clone = is_closing.clone();
        let anim_config = config.animation.clone();
        controller.connect_key_pressed(move |_, key, _, _| {
            if key == gtk4::gdk::Key::Escape {
                if !*is_closing_clone.borrow() {
                    *is_closing_clone.borrow_mut() = true;
                    close_with_animation(&window_clone, &anim_config);
                }
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
        window.add_controller(controller);
    }

    // Close on unfocus
    if config.behavior.close_on_unfocus {
        let focus_controller = gtk4::EventControllerFocus::new();
        let window_clone = window.clone();
        let is_closing_clone = is_closing.clone();
        let anim_config = config.animation.clone();
        focus_controller.connect_leave(move |_| {
            if !*is_closing_clone.borrow() {
                *is_closing_clone.borrow_mut() = true;
                close_with_animation(&window_clone, &anim_config);
            }
        });
        window.add_controller(focus_controller);
    }

    window.present();

    // Animate in
    if config.animation.enabled {
        let direction = match config.animation.direction.as_str() {
            "up" => animation::Direction::Up,
            _ => animation::Direction::Down,
        };
        animation::slide_in(
            &window,
            direction,
            config.animation.duration,
            config.position.margin_top,
        );
    }
}

fn apply_position(window: &gtk4::ApplicationWindow, config: &Config) {
    // Reset anchors
    window.set_anchor(Edge::Top, false);
    window.set_anchor(Edge::Bottom, false);
    window.set_anchor(Edge::Left, false);
    window.set_anchor(Edge::Right, false);

    match config.position.anchor.as_str() {
        "top-left" => {
            window.set_anchor(Edge::Top, true);
            window.set_anchor(Edge::Left, true);
        }
        "top-center" => {
            window.set_anchor(Edge::Top, true);
        }
        "top-right" => {
            window.set_anchor(Edge::Top, true);
            window.set_anchor(Edge::Right, true);
        }
        "bottom-left" => {
            window.set_anchor(Edge::Bottom, true);
            window.set_anchor(Edge::Left, true);
        }
        "bottom-center" => {
            window.set_anchor(Edge::Bottom, true);
        }
        "bottom-right" => {
            window.set_anchor(Edge::Bottom, true);
            window.set_anchor(Edge::Right, true);
        }
        "center-left" => {
            window.set_anchor(Edge::Left, true);
        }
        "center-right" => {
            window.set_anchor(Edge::Right, true);
        }
        _ => {
            // Default top-right
            window.set_anchor(Edge::Top, true);
            window.set_anchor(Edge::Right, true);
        }
    }

    window.set_margin(Edge::Top, config.position.margin_top);
    window.set_margin(Edge::Right, config.position.margin_right);
    window.set_margin(Edge::Bottom, config.position.margin_bottom);
    window.set_margin(Edge::Left, config.position.margin_left);
}

fn close_with_animation(window: &gtk4::ApplicationWindow, anim_config: &config::AnimationConfig) {
    if anim_config.enabled {
        let direction = match anim_config.direction.as_str() {
            "down" => animation::Direction::Up, // Reverse for close
            _ => animation::Direction::Down,
        };
        let w = window.clone();
        animation::slide_out(window, direction, anim_config.duration, move || w.close());
    } else {
        window.close();
    }
}

fn update_stats(
    poller: &Rc<RefCell<Poller>>,
    config: &Rc<Config>,
    cpu_label: &Label,
    cpu_bar: &ProgressBar,
    mem_label: &Label,
    mem_bar: &ProgressBar,
    disk_label: &Label,
    disk_bar: &ProgressBar,
    battery_label: &Label,
    temps_box: &Box,
) {
    let p = poller.borrow();

    // CPU
    if config.sections.cpu {
        let cpu = p.cpu();
        cpu_label.set_text(&format!("{} @ {}", cpu.usage_str(), cpu.frequency_str()));
        cpu_bar.set_fraction(cpu.usage as f64 / 100.0);
        update_bar_class(cpu_bar, cpu.usage, config);
    }

    // Memory
    if config.sections.memory {
        let mem = p.memory();
        mem_label.set_text(&mem.summary_str());
        mem_bar.set_fraction(mem.usage as f64 / 100.0);
        update_bar_class(mem_bar, mem.usage, config);
    }

    // Disk
    if config.sections.disk {
        let disks = p.disk();
        if let Some(root) = disks.iter().find(|d| d.mount == "/") {
            disk_label.set_text(&format!("{} / {}", root.used_str(), root.total_str()));
            disk_bar.set_fraction(root.usage as f64 / 100.0);
            update_bar_class(disk_bar, root.usage, config);
        }
    }

    // Battery
    if config.sections.battery {
        if let Some(bat) = p.battery() {
            let status = match bat.status {
                widget_poll::battery::BatteryStatus::Charging => " (charging)",
                widget_poll::battery::BatteryStatus::Discharging => "",
                widget_poll::battery::BatteryStatus::Full => " (full)",
                _ => "",
            };
            battery_label.set_text(&format!("{}{}", bat.percentage_str(), status));
        } else {
            battery_label.set_text("N/A");
        }
    }

    // Temps
    if config.sections.temperatures {
        while let Some(child) = temps_box.first_child() {
            temps_box.remove(&child);
        }

        let temps = p.thermal();
        let key_temps: Vec<_> = temps
            .iter()
            .filter(|t| {
                let label = t.label.to_lowercase();
                config
                    .temperatures
                    .show_labels
                    .iter()
                    .any(|l| label.contains(&l.to_lowercase()))
            })
            .take(config.temperatures.max_display)
            .collect();

        for temp in key_temps {
            let row = Box::new(Orientation::Horizontal, 8);

            let name = Label::new(Some(&temp.label));
            name.add_css_class("temp-label");
            name.set_halign(gtk4::Align::Start);
            name.set_hexpand(true);

            let value = Label::new(Some(&temp.temp_str()));
            value.add_css_class("temp-value");
            if temp.temp >= config.temperatures.critical_threshold {
                value.add_css_class("temp-critical");
            } else if temp.temp >= config.temperatures.warning_threshold {
                value.add_css_class("temp-warning");
            }

            row.append(&name);
            row.append(&value);
            temps_box.append(&row);
        }
    }
}

fn update_bar_class(bar: &ProgressBar, usage: f32, config: &Rc<Config>) {
    bar.remove_css_class("warning");
    bar.remove_css_class("critical");

    if usage >= config.temperatures.critical_threshold {
        bar.add_css_class("critical");
    } else if usage >= config.temperatures.warning_threshold {
        bar.add_css_class("warning");
    }
}
