mod config;
mod css;

use config::Config;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow, Image, Label, Orientation, Scale};
use gtk4_layer_shell::{Edge, Layer, LayerShell};
use std::cell::RefCell;
use std::rc::Rc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::UnixListener;
use tokio::sync::mpsc;

const APP_ID: &str = "com.vib1240n.rust-widgets.volume-osd";
const SOCKET_PATH: &str = "/tmp/rw-volume-osd.sock";

#[derive(Debug, Clone)]
struct VolumeState {
    volume: f64, // 0.0 - 1.0+
    muted: bool,
}

fn main() {
    tracing_subscriber::fmt().with_env_filter("info").init();

    // Clean up old socket
    let _ = std::fs::remove_file(SOCKET_PATH);

    let app = Application::builder().application_id(APP_ID).build();

    app.connect_activate(build_ui);
    app.run();
}

fn build_ui(app: &Application) {
    let config = Rc::new(Config::load());
    css::load();

    // Create hidden window initially
    let window = ApplicationWindow::builder()
        .application(app)
        .title("Volume OSD")
        .build();

    // Layer shell setup
    window.init_layer_shell();
    window.set_layer(Layer::Overlay);
    window.set_namespace("rust-widgets");
    window.set_exclusive_zone(-1); // Don't reserve space

    // Position
    apply_position(&window, &config);

    // Main container - horizontal layout like macOS
    let container = gtk4::Box::new(Orientation::Horizontal, 12);
    container.add_css_class("osd-container");
    container.set_halign(gtk4::Align::Center);
    container.set_valign(gtk4::Align::Center);

    // Volume icon
    let icon = Image::from_icon_name("audio-volume-high-symbolic");
    icon.set_pixel_size(config.appearance.icon_size);
    icon.add_css_class("osd-icon");
    container.append(&icon);

    // Volume bar
    let scale = Scale::with_range(Orientation::Horizontal, 0.0, 100.0, 1.0);
    scale.set_draw_value(false);
    scale.set_hexpand(true);
    scale.set_sensitive(false); // Display only, not interactive
    scale.set_width_request(config.appearance.width - 80);
    scale.add_css_class("osd-scale");
    container.append(&scale);

    // Percentage label
    let percent_label = Label::new(Some("100%"));
    percent_label.add_css_class("osd-percent");
    percent_label.set_width_request(45);
    if config.appearance.show_percentage {
        container.append(&percent_label);
    }

    window.set_child(Some(&container));

    // Shared state
    let icon_ref = Rc::new(icon);
    let scale_ref = Rc::new(scale);
    let label_ref = Rc::new(percent_label);
    let window_ref = Rc::new(window);
    // Store the timeout handle - use u32 ID instead of SourceId to avoid remove panic
    let hide_timeout_id: Rc<RefCell<Option<u32>>> = Rc::new(RefCell::new(None));
    let is_visible = Rc::new(RefCell::new(false));
    let timeout_ms = config.behavior.timeout;

    // Channel for receiving volume updates from async socket listener
    let (tx, rx) = mpsc::unbounded_channel::<VolumeState>();

    // Spawn async socket listener
    let tx_clone = tx.clone();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async move {
            if let Err(e) = run_socket_listener(tx_clone).await {
                tracing::error!("Socket listener error: {}", e);
            }
        });
    });

    // Handle incoming volume updates on GTK main thread
    let icon_clone = icon_ref.clone();
    let scale_clone = scale_ref.clone();
    let label_clone = label_ref.clone();
    let window_clone = window_ref.clone();
    let hide_timeout_clone = hide_timeout_id.clone();
    let is_visible_clone = is_visible.clone();

    glib::spawn_future_local(async move {
        let mut rx = rx;
        while let Some(state) = rx.recv().await {
            update_osd(
                &icon_clone,
                &scale_clone,
                &label_clone,
                &window_clone,
                &hide_timeout_clone,
                &is_visible_clone,
                &state,
                timeout_ms,
            );
        }
    });
}

fn apply_position(window: &ApplicationWindow, config: &Config) {
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
        "center" => {
            // No anchors = centered
        }
        _ => {
            window.set_anchor(Edge::Top, true);
        }
    }

    window.set_margin(Edge::Top, config.position.margin_top);
    window.set_margin(Edge::Right, config.position.margin_right);
    window.set_margin(Edge::Bottom, config.position.margin_bottom);
    window.set_margin(Edge::Left, config.position.margin_left);
}

fn update_osd(
    icon: &Rc<Image>,
    scale: &Rc<Scale>,
    label: &Rc<Label>,
    window: &Rc<ApplicationWindow>,
    hide_timeout_id: &Rc<RefCell<Option<u32>>>,
    is_visible: &Rc<RefCell<bool>>,
    state: &VolumeState,
    timeout_ms: u64,
) {
    // Update icon based on volume/mute state
    let icon_name = if state.muted {
        "audio-volume-muted-symbolic"
    } else if state.volume <= 0.0 {
        "audio-volume-muted-symbolic"
    } else if state.volume < 0.33 {
        "audio-volume-low-symbolic"
    } else if state.volume < 0.66 {
        "audio-volume-medium-symbolic"
    } else {
        "audio-volume-high-symbolic"
    };
    icon.set_icon_name(Some(icon_name));

    // Update muted class
    if state.muted {
        icon.add_css_class("muted");
        scale.add_css_class("muted");
    } else {
        icon.remove_css_class("muted");
        scale.remove_css_class("muted");
    }

    // Update scale (cap at 100 for display, but allow > 100%)
    let display_vol = (state.volume * 100.0).min(100.0);
    scale.set_value(display_vol);

    // Update percentage label
    let percent = (state.volume * 100.0).round() as i32;
    label.set_text(&format!("{}%", percent));

    // Show window if hidden
    if !*is_visible.borrow() {
        window.present();
        *is_visible.borrow_mut() = true;
    }

    // Cancel existing hide timeout using glib::source_remove (won't panic if invalid)
    if let Some(id) = hide_timeout_id.borrow_mut().take() {
        // Use the safe version that doesn't panic
        unsafe {
            glib::ffi::g_source_remove(id);
        }
    }

    // Schedule new hide timeout
    let window_clone = window.clone();
    let is_visible_clone = is_visible.clone();
    let hide_timeout_id_clone = hide_timeout_id.clone();

    let source_id =
        glib::timeout_add_local_once(std::time::Duration::from_millis(timeout_ms), move || {
            window_clone.set_visible(false);
            *is_visible_clone.borrow_mut() = false;
            // Clear the timeout ID since it fired
            *hide_timeout_id_clone.borrow_mut() = None;
        });

    // Store the raw ID
    *hide_timeout_id.borrow_mut() = Some(source_id.as_raw());
}

async fn run_socket_listener(tx: mpsc::UnboundedSender<VolumeState>) -> Result<(), std::io::Error> {
    let listener = UnixListener::bind(SOCKET_PATH)?;
    tracing::info!("Volume OSD listening on {}", SOCKET_PATH);

    loop {
        let (stream, _) = listener.accept().await?;
        let tx = tx.clone();

        tokio::spawn(async move {
            let reader = BufReader::new(stream);
            let mut lines = reader.lines();

            while let Ok(Some(line)) = lines.next_line().await {
                // Parse: "volume:0.75" or "volume:0.75:muted"
                let parts: Vec<&str> = line.trim().split(':').collect();
                if parts.len() >= 2 && parts[0] == "volume" {
                    if let Ok(vol) = parts[1].parse::<f64>() {
                        let muted = parts.get(2).map(|s| *s == "muted").unwrap_or(false);
                        let state = VolumeState { volume: vol, muted };
                        let _ = tx.send(state);
                    }
                }
            }
        });
    }
}
