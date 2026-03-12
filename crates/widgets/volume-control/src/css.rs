use gtk4::gdk::Display;
use gtk4::CssProvider;
use std::path::PathBuf;

const EMBEDDED_CSS: &str = include_str!("style.css");

pub fn load() {
    let provider = CssProvider::new();

    // Try external CSS first (same path as config)
    let external_path = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("rw/volume-control")
        .join("style.css");

    if external_path.exists() {
        provider.load_from_path(&external_path);
        tracing::info!("Loaded external CSS from {:?}", external_path);
    } else {
        provider.load_from_data(EMBEDDED_CSS);
        tracing::debug!("Using embedded CSS");
    }

    gtk4::style_context_add_provider_for_display(
        &Display::default().expect("Could not get default display"),
        &provider,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}
