use gtk4::gdk::Display;
use gtk4::CssProvider;
use std::path::Path;
use tracing::{debug, warn};

/// Load CSS from a file path
pub fn load_css(path: &Path) {
    let provider = CssProvider::new();

    if path.exists() {
        provider.load_from_path(path);
        debug!("Loaded CSS from {:?}", path);
    } else {
        warn!("CSS file not found: {:?}", path);
        return;
    }

    gtk4::style_context_add_provider_for_display(
        &Display::default().expect("Could not get default display"),
        &provider,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}

/// Load CSS from a string
pub fn load_css_string(css: &str) {
    let provider = CssProvider::new();
    provider.load_from_data(css);

    gtk4::style_context_add_provider_for_display(
        &Display::default().expect("Could not get default display"),
        &provider,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}

/// Load embedded default CSS plus optional user override
pub fn load_css_with_fallback(default_css: &str, user_path: Option<&Path>) {
    // Load default first
    load_css_string(default_css);

    // Then user overrides if exists
    if let Some(path) = user_path {
        if path.exists() {
            load_css(path);
        }
    }
}
