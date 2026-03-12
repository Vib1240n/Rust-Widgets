pub mod config;
pub mod css;
pub mod position;
pub mod widget;

pub use config::Config;
pub use css::load_css;
pub use position::Position;
pub use widget::Widget;

use gtk4::Application;
use gtk4_layer_shell::{Edge, Layer, LayerShell};

/// Initialize a layer shell window with common settings
pub fn init_layer_window(window: &gtk4::ApplicationWindow, pos: &Position, namespace: &str) {
    window.init_layer_shell();
    window.set_layer(Layer::Overlay);
    window.set_namespace(namespace);
    window.set_keyboard_mode(gtk4_layer_shell::KeyboardMode::OnDemand);

    // Clear all anchors first
    window.set_anchor(Edge::Top, false);
    window.set_anchor(Edge::Bottom, false);
    window.set_anchor(Edge::Left, false);
    window.set_anchor(Edge::Right, false);

    // Apply position
    match pos {
        Position::TopLeft => {
            window.set_anchor(Edge::Top, true);
            window.set_anchor(Edge::Left, true);
        }
        Position::TopCenter => {
            window.set_anchor(Edge::Top, true);
        }
        Position::TopRight => {
            window.set_anchor(Edge::Top, true);
            window.set_anchor(Edge::Right, true);
        }
        Position::BottomLeft => {
            window.set_anchor(Edge::Bottom, true);
            window.set_anchor(Edge::Left, true);
        }
        Position::BottomCenter => {
            window.set_anchor(Edge::Bottom, true);
        }
        Position::BottomRight => {
            window.set_anchor(Edge::Bottom, true);
            window.set_anchor(Edge::Right, true);
        }
        Position::Center => {
            // No anchors = centered
        }
        Position::CenterLeft => {
            window.set_anchor(Edge::Left, true);
        }
        Position::CenterRight => {
            window.set_anchor(Edge::Right, true);
        }
    }
}

/// Create a basic GTK application for a widget
pub fn create_app(app_id: &str) -> Application {
    Application::builder().application_id(app_id).build()
}
