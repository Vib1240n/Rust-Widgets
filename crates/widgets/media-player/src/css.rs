//! CSS loading for media-player widget

use gtk4::CssProvider;
use std::path::PathBuf;

/// Default CSS compiled into the binary
const DEFAULT_STYLE: &str = include_str!("../../style.css");

const WIDGET_NAME: &str = "media-player";

/// Load CSS for the widget
pub fn load() {
    let display = gtk4::gdk::Display::default().expect("Could not get default display");

    // 1. Load default CSS (lowest priority)
    let default_provider = CssProvider::new();
    default_provider.load_from_data(DEFAULT_STYLE);
    gtk4::style_context_add_provider_for_display(
        &display,
        &default_provider,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    // 2. Check for user CSS override (~/.config/rw/style.css)
    let user_css_path = user_style_path();
    if user_css_path.exists() {
        let user_provider = CssProvider::new();
        user_provider.load_from_path(&user_css_path);
        gtk4::style_context_add_provider_for_display(
            &display,
            &user_provider,
            gtk4::STYLE_PROVIDER_PRIORITY_USER,
        );
        tracing::info!("Loaded user CSS from {:?}", user_css_path);
    }

    // 3. Check for deprecated per-widget CSS and warn
    let deprecated_path = deprecated_widget_style_path();
    if deprecated_path.exists() {
        tracing::warn!(
            "DEPRECATED: Per-widget style.css found at {:?}",
            deprecated_path
        );
        tracing::warn!(
            "Please migrate your custom styles to ~/.config/rw/style.css"
        );
        tracing::warn!(
            "Per-widget style.css files will be ignored in future versions"
        );
    }
}

fn user_style_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("~/.config"))
        .join("rw")
        .join("style.css")
}

fn deprecated_widget_style_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("~/.config"))
        .join("rw")
        .join(WIDGET_NAME)
        .join("style.css")
}
