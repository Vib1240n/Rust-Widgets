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

const APP_ID: &str = "com.vib1240n.rust-widgets.brightness-osd";
const SOCKET_PATH: &str = "/tmp/rw-brightness-osd.sock";

#[derive(Debug, Clone)]
struct BrightnessState {
    percent: f64, // 0.0 - 100.0
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

    // Created hidden; presented on first brightness change.
    let window = ApplicationWindow::builder()
        .application(app)
        .title("Brightness OSD")
        .build();

    window.init_layer_shell();
    window.set_layer(Layer::Overlay);
    window.set_namespace("rust-widgets");
    window.set_exclusive_zone(-1); // Don't reserve space

    apply_position(&window, &config);

    // Horizontal layout like macOS, shared OSD classes from the consolidated CSS.
    let container = gtk4::Box::new(Orientation::Horizontal, 12);
    container.add_css_class("osd-container");
    container.set_halign(gtk4::Align::Center);
    container.set_valign(gtk4::Align::Center);

    let icon = Image::from_icon_name("display-brightness-symbolic");
    icon.set_pixel_size(config.appearance.icon_size);
    icon.add_css_class("osd-icon");
    container.append(&icon);

    let scale = Scale::with_range(Orientation::Horizontal, 0.0, 100.0, 1.0);
    scale.set_draw_value(false);
    scale.set_hexpand(true);
    scale.set_sensitive(false); // Display only, not interactive
    scale.set_width_request(config.appearance.width - 80);
    scale.add_css_class("osd-scale");
    container.append(&scale);

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
    let hide_timeout: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
    let is_visible = Rc::new(RefCell::new(false));
    let timeout_ms = config.behavior.timeout;

    // Channel for receiving brightness updates from the async socket listener.
    let (tx, rx) = mpsc::unbounded_channel::<BrightnessState>();

    // Socket listener runs on its own tokio runtime in a background thread.
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async move {
            if let Err(e) = run_socket_listener(tx).await {
                tracing::error!("Socket listener error: {}", e);
            }
        });
    });

    // Drain updates on the GTK main thread.
    let icon_clone = icon_ref.clone();
    let scale_clone = scale_ref.clone();
    let label_clone = label_ref.clone();
    let window_clone = window_ref.clone();
    let hide_timeout_clone = hide_timeout.clone();
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

#[allow(clippy::too_many_arguments)]
fn update_osd(
    icon: &Rc<Image>,
    scale: &Rc<Scale>,
    label: &Rc<Label>,
    window: &Rc<ApplicationWindow>,
    hide_timeout: &Rc<RefCell<Option<glib::SourceId>>>,
    is_visible: &Rc<RefCell<bool>>,
    state: &BrightnessState,
    timeout_ms: u64,
) {
    let percent = state.percent.round().clamp(0.0, 100.0) as i32;

    icon.set_icon_name(Some(brightness_icon(percent)));
    scale.set_value(percent as f64);
    label.set_text(&format!("{}%", percent));

    // Show window if hidden
    if !*is_visible.borrow() {
        window.present();
        *is_visible.borrow_mut() = true;
    }

    // Cancel existing hide timeout. Safe because the fired timeout nulls itself.
    if let Some(id) = hide_timeout.borrow_mut().take() {
        id.remove();
    }

    // Schedule new hide timeout
    let window_c = window.clone();
    let is_visible_c = is_visible.clone();
    let hide_timeout_c = hide_timeout.clone();
    let source_id = glib::timeout_add_local_once(
        std::time::Duration::from_millis(timeout_ms),
        move || {
            window_c.set_visible(false);
            *is_visible_c.borrow_mut() = false;
            *hide_timeout_c.borrow_mut() = None;
        },
    );
    *hide_timeout.borrow_mut() = Some(source_id);
}

fn brightness_icon(percent: i32) -> &'static str {
    if percent <= 33 {
        "display-brightness-low-symbolic"
    } else if percent <= 66 {
        "display-brightness-medium-symbolic"
    } else {
        "display-brightness-high-symbolic"
    }
}

async fn run_socket_listener(
    tx: mpsc::UnboundedSender<BrightnessState>,
) -> Result<(), std::io::Error> {
    let listener = UnixListener::bind(SOCKET_PATH)?;
    tracing::info!("Brightness OSD listening on {}", SOCKET_PATH);

    loop {
        let (stream, _) = listener.accept().await?;
        let tx = tx.clone();

        tokio::spawn(async move {
            let reader = BufReader::new(stream);
            let mut lines = reader.lines();

            while let Ok(Some(line)) = lines.next_line().await {
                // Parse: "brightness:75"
                let parts: Vec<&str> = line.trim().split(':').collect();
                if parts.len() >= 2 && parts[0] == "brightness" {
                    if let Ok(pct) = parts[1].parse::<f64>() {
                        let _ = tx.send(BrightnessState { percent: pct });
                    }
                }
            }
        });
    }
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
