//! Themes for every rust-widgets binary.
//!
//! CSS is built in this order into ONE provider (swapped on reload):
//!   1. palette.css   colour names   (user copy if present, else built-in)
//!   2. glass.css     design system  (built-in, read-only)
//!   3. component     the widget's own CSS (compiled in)
//!   4. themes/<name>/style.css        user extras for that theme
//!   5. ~/.config/rustapp-theme/style.css   global user overrides
//!   6. ~/.config/rw/style.css              rw-only user overrides
//!
//! Theme name: ~/.config/rw/theme (rw only) > ~/.config/rustapp-theme/config
//! (`theme = name`, shared with launch-gui / cliphist-gui) > "glass".
//! Built-ins live in the binary and can't be edited; `rw theme <name>`
//! copies one to ~/.config/{rw|rustapp-theme}/themes/<name>/ for editing.
//! Changes to any of these files apply live (file monitors, no polling).

use gtk4::gio;
use gtk4::prelude::*;
use std::cell::RefCell;
use std::path::PathBuf;
use std::time::Duration;

pub const APP_DIR: &str = "rw";
pub const DEFAULT_THEME: &str = "glass";
const GLOBAL_DIR: &str = "rustapp-theme";

pub struct Builtin {
    pub name: &'static str,
    pub palette: &'static str,
    pub glass: &'static str,
}

/// Compiled-in themes (Rust-Widgets/themes/<name>/)
pub const BUILTIN: &[Builtin] = &[Builtin {
    name: "glass",
    palette: include_str!("../../../themes/glass/palette.css"),
    glass: include_str!("../../../themes/glass/glass.css"),
}];

const STYLE_TEMPLATE: &str = "/* Extra rules for this theme, loaded after the built-in design system.\n * Colours: use the names from palette.css (@bg @surface @fg @border\n * @shadow @accent @accent_fg @red @yellow @green), e.g.\n *\n *   .widget-container { border-radius: 20px; }\n */\n";

fn config_home() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".config"))
}

pub fn global_dir() -> PathBuf {
    config_home().join(GLOBAL_DIR)
}

pub fn app_dir() -> PathBuf {
    config_home().join(APP_DIR)
}

fn read(p: PathBuf) -> Option<String> {
    std::fs::read_to_string(p).ok()
}

fn app_choice() -> Option<String> {
    read(app_dir().join("theme"))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn global_choice() -> Option<String> {
    let cfg = read(global_dir().join("config"))?;
    cfg.lines().find_map(|l| {
        let (k, v) = l.split_once('=')?;
        (k.trim() == "theme").then(|| v.trim().trim_matches('"').to_string())
    })
    .filter(|s| !s.is_empty())
}

pub fn current_name() -> String {
    app_choice()
        .or_else(global_choice)
        .unwrap_or_else(|| DEFAULT_THEME.to_string())
}

/// The user's editable copy of `name`: rw's first, then the global one
fn user_theme_dir(name: &str) -> Option<PathBuf> {
    [app_dir(), global_dir()]
        .into_iter()
        .map(|d| d.join("themes").join(name))
        .find(|d| d.is_dir())
}

pub fn build_css(component: &str) -> String {
    let name = current_name();
    let builtin = BUILTIN.iter().find(|b| b.name == name).unwrap_or(&BUILTIN[0]);
    let user = user_theme_dir(&name);

    let palette = user
        .as_ref()
        .and_then(|d| read(d.join("palette.css")))
        .unwrap_or_else(|| builtin.palette.to_string());

    let mut css = String::with_capacity(palette.len() + builtin.glass.len() + component.len() + 4096);
    css.push_str(&palette);
    css.push('\n');
    css.push_str(builtin.glass);
    css.push('\n');
    css.push_str(component);
    for extra in [
        user.map(|d| d.join("style.css")),
        Some(global_dir().join("style.css")),
        Some(app_dir().join("style.css")),
    ]
    .into_iter()
    .flatten()
    {
        if let Some(s) = read(extra) {
            css.push('\n');
            css.push_str(&s);
        }
    }
    css
}

// ---------------------------------------------------------------------------
// Applying + live reload
// ---------------------------------------------------------------------------

thread_local! {
    static PROVIDER: RefCell<Option<gtk4::CssProvider>> = const { RefCell::new(None) };
    static MONITORS: RefCell<Vec<gio::FileMonitor>> = const { RefCell::new(Vec::new()) };
    static PENDING: RefCell<Option<glib::SourceId>> = const { RefCell::new(None) };
}

use gtk4::glib;

/// Build and install the stylesheet (replacing the previous one).
pub fn apply(component: &str) {
    let Some(display) = gtk4::gdk::Display::default() else { return };
    let provider = gtk4::CssProvider::new();
    provider.connect_parsing_error(|_, section, err| {
        tracing::warn!("theme css: {} at {}", err, section.to_str());
    });
    provider.load_from_string(&build_css(component));
    PROVIDER.with(|p| {
        if let Some(old) = p.borrow_mut().take() {
            gtk4::style_context_remove_provider_for_display(&display, &old);
        }
        gtk4::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
        *p.borrow_mut() = Some(provider);
    });
    tracing::info!("theme '{}' applied", current_name());
}

/// Apply now and re-apply whenever a theme/config/style file changes.
pub fn load_and_watch(component: &'static str) {
    apply(component);
    arm(component);
}

/// Watch the folders that can change the result. Directory monitors (inotify)
/// cost nothing while idle; re-armed after each change since the theme
/// folder may have switched.
fn arm(component: &'static str) {
    let _ = std::fs::create_dir_all(global_dir());
    let _ = std::fs::create_dir_all(app_dir());
    let name = current_name();
    let dirs = [
        global_dir(),
        app_dir(),
        global_dir().join("themes").join(&name),
        app_dir().join("themes").join(&name),
    ];
    let mut monitors = Vec::new();
    for d in dirs.iter().filter(|d| d.is_dir()) {
        let file = gio::File::for_path(d);
        if let Ok(m) = file.monitor_directory(gio::FileMonitorFlags::NONE, None::<&gio::Cancellable>) {
            m.connect_changed(move |_, _, _, _| schedule_reload(component));
            monitors.push(m);
        }
    }
    MONITORS.with(|m| *m.borrow_mut() = monitors); // old monitors dropped = cancelled
}

/// Editors write files in bursts; reload once 150 ms after the last event.
fn schedule_reload(component: &'static str) {
    PENDING.with(|p| {
        if let Some(id) = p.borrow_mut().take() {
            id.remove();
        }
    });
    let id = glib::timeout_add_local_once(Duration::from_millis(150), move || {
        PENDING.with(|p| p.borrow_mut().take());
        apply(component);
        arm(component);
    });
    PENDING.with(|p| *p.borrow_mut() = Some(id));
}

// ---------------------------------------------------------------------------
// CLI: rw theme <name> [--global] [--reset]
// ---------------------------------------------------------------------------

/// Select `name` for rw (or globally) and make sure an editable copy exists.
pub fn set_theme(name: &str, global: bool, reset: bool) -> Result<String, String> {
    let builtin = BUILTIN.iter().find(|b| b.name == name);
    let scope = if global { global_dir() } else { app_dir() };
    let dest = scope.join("themes").join(name);

    match builtin {
        Some(b) => {
            if reset || !dest.join("palette.css").exists() {
                std::fs::create_dir_all(&dest).map_err(|e| e.to_string())?;
                std::fs::write(dest.join("palette.css"), b.palette).map_err(|e| e.to_string())?;
            }
            if reset || !dest.join("style.css").exists() {
                std::fs::write(dest.join("style.css"), STYLE_TEMPLATE).map_err(|e| e.to_string())?;
            }
        }
        None if user_theme_dir(name).is_none() => {
            return Err(format!(
                "unknown theme '{name}': not built in, and no {}/themes/{name}/palette.css",
                scope.display()
            ));
        }
        None => {}
    }

    if global {
        write_global_choice(name)?;
        // follow the global choice from now on
        let _ = std::fs::remove_file(app_dir().join("theme"));
    } else {
        std::fs::create_dir_all(app_dir()).map_err(|e| e.to_string())?;
        std::fs::write(app_dir().join("theme"), format!("{name}\n")).map_err(|e| e.to_string())?;
    }
    Ok(format!(
        "theme '{name}' selected {}; edit {}",
        if global { "globally" } else { "for rw" },
        dest.display()
    ))
}

fn write_global_choice(name: &str) -> Result<(), String> {
    let path = global_dir().join("config");
    std::fs::create_dir_all(global_dir()).map_err(|e| e.to_string())?;
    let existing = read(path.clone()).unwrap_or_default();
    let mut found = false;
    let mut out: Vec<String> = existing
        .lines()
        .map(|l| match l.split_once('=') {
            Some((k, _)) if k.trim() == "theme" => {
                found = true;
                format!("theme = {name}")
            }
            _ => l.to_string(),
        })
        .collect();
    if !found {
        if out.is_empty() {
            out.push("# Shared theme for rust apps (rw widgets, launch-gui, cliphist-gui)".into());
        }
        out.push(format!("theme = {name}"));
    }
    std::fs::write(&path, out.join("\n") + "\n").map_err(|e| e.to_string())
}

/// Built-ins plus user themes (name, where)
pub fn list() -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> =
        BUILTIN.iter().map(|b| (b.name.to_string(), "built-in".to_string())).collect();
    for (dir, label) in [(app_dir(), "rw"), (global_dir(), "global")] {
        if let Ok(rd) = std::fs::read_dir(dir.join("themes")) {
            for e in rd.flatten().filter(|e| e.path().is_dir()) {
                v.push((e.file_name().to_string_lossy().into_owned(), format!("user ({label})")));
            }
        }
    }
    v
}
