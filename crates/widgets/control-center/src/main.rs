mod animation;
mod config;
mod css;
mod media;
mod sliders;
mod stats;
mod toggles;

use config::Config;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow, Box, Orientation, Separator};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

const APP_ID: &str = "com.vib1240n.rust-widgets.control-center";
const BINARY_NAME: &str = "rw-control";

fn main() {
    // Enforce single instance - exit if already running
    if is_already_running() {
        eprintln!("rw-control is already running");
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
        .title("Control Center")
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
    container.add_css_class("control-center");
    container.set_width_request(config.appearance.width);

    // Quick toggles section
    if config.sections.toggles {
        let toggles_box = toggles::build(&config);
        container.append(&toggles_box);
        container.append(&Separator::new(Orientation::Horizontal));
    }

    // Sliders section
    if config.sections.sliders {
        let sliders_box = sliders::build(&config);
        container.append(&sliders_box);
        container.append(&Separator::new(Orientation::Horizontal));
    }

    // Media section
    if config.sections.media {
        let media_box = media::build();
        container.append(&media_box);
        container.append(&Separator::new(Orientation::Horizontal));
    }

    // Quick stats section
    if config.sections.stats {
        let stats_box = stats::build();
        container.append(&stats_box);
    }

    window.set_child(Some(&container));

    // Setup polling for dynamic content
    let config_clone = config.clone();
    glib::timeout_add_local(
        Duration::from_millis(config.behavior.poll_interval),
        move || {
            sliders::update(&config_clone);
            media::update();
            stats::update();
            glib::ControlFlow::Continue
        },
    );

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
        _ => {
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
            "down" => animation::Direction::Up,
            _ => animation::Direction::Down,
        };
        let w = window.clone();
        animation::slide_out(window, direction, anim_config.duration, move || w.close());
    } else {
        window.close();
    }
}
