use crate::config::Config;
use gtk4::CssProvider;

const STYLE: &str = include_str!("style.css");

pub fn load() {
    let provider = CssProvider::new();
    provider.load_from_data(STYLE);
    gtk4::style_context_add_provider_for_display(
        &gtk4::gdk::Display::default().expect("Could not get default display"),
        &provider,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    // Load user override if exists
    let user_css = Config::style_path();
    if user_css.exists() {
        let user_provider = CssProvider::new();
        user_provider.load_from_path(&user_css);
        gtk4::style_context_add_provider_for_display(
            &gtk4::gdk::Display::default().expect("Could not get default display"),
            &user_provider,
            gtk4::STYLE_PROVIDER_PRIORITY_USER,
        );
        tracing::info!("Loaded user CSS from {:?}", user_css);
    }
}
